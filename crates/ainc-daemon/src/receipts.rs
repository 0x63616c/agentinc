//! Command receipts. Every mutating command carries a client-chosen operation
//! ID; the committed receipt is the acknowledgement. Repeating an operation
//! with the same request replays its result, repeating it with a different
//! request is a conflict. One table, one lock-key rule, one hash rule.
use std::fmt;

use futures::future::BoxFuture;
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{Postgres, Transaction};

use crate::api::CommandError;

/// A client-chosen UUID naming one attempt at a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(uuid::Uuid);

/// The operation ID was not a UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidOperationId;

impl OperationId {
    pub fn parse(text: &str) -> Result<Self, InvalidOperationId> {
        uuid::Uuid::parse_str(text)
            .map(Self)
            .map_err(|_| InvalidOperationId)
    }
}
impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Serialize for OperationId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
impl From<InvalidOperationId> for CommandError {
    fn from(_: InvalidOperationId) -> Self {
        CommandError::Invalid("Use a UUID operation ID.".into())
    }
}

/// The command family an operation ID is unique within.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Workspace commands: one ID space.
    Workspace,
    /// Todo commands: one ID space per workspace.
    Todo { workspace: String },
    /// Calendar commands: one ID space per workspace.
    Calendar { workspace: String },
}
impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scope::Workspace => f.write_str("workspace"),
            Scope::Todo { workspace } => write!(f, "todo/{workspace}"),
            Scope::Calendar { workspace } => write!(f, "calendar/{workspace}"),
        }
    }
}

/// A committed command and its result.
#[derive(Clone, Debug)]
pub(crate) struct Receipt<R> {
    pub result: R,
}

/// Run `apply` once per (scope, operation). A repeat with the same request
/// replays the stored result; a repeat with a different request conflicts.
/// Concurrent repeats serialize on one advisory lock so the second sees the
/// first's committed receipt. The receipt commits with the caller's transaction.
pub(crate) async fn execute<'t, R, F>(
    tx: &mut Transaction<'t, Postgres>,
    scope: Scope,
    operation_id: OperationId,
    request: &impl Serialize,
    apply: F,
) -> Result<Receipt<R>, CommandError>
where
    R: Serialize + DeserializeOwned,
    F: for<'c> FnOnce(&'c mut Transaction<'t, Postgres>) -> BoxFuture<'c, Result<R, CommandError>>,
{
    let request = serde_json::to_value(request)
        .map_err(|_| CommandError::Invalid("Invalid command.".into()))?;
    let scope = scope.to_string();
    let operation = operation_id.to_string();
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        format!("receipt/{scope}/{operation}")
    )
    .execute(&mut **tx)
    .await?;
    let prior = sqlx::query!(
        r#"SELECT request_hash = receipt_hash($3) AS "same_request!", result FROM receipts WHERE scope=$1 AND operation_id=$2::text::uuid"#,
        scope,
        operation,
        request
    )
    .fetch_optional(&mut **tx)
    .await?;
    let result = match prior {
        Some(prior) if prior.same_request => serde_json::from_value(prior.result).map_err(|error| {
            tracing::error!(%error, scope, operation, "stored receipt result does not match its command");
            CommandError::Internal
        })?,
        Some(_) => {
            return Err(CommandError::Conflict(
                "This operation ID was already used for a different command.".into(),
            ));
        }
        None => {
            let result = apply(tx).await?;
            sqlx::query!(
                "INSERT INTO receipts(scope,operation_id,request_hash,result) VALUES ($1,$2::text::uuid,receipt_hash($3),$4)",
                scope,
                operation,
                request,
                serde_json::to_value(&result).expect("serializable result")
            )
            .execute(&mut **tx)
            .await?;
            result
        }
    };
    Ok(Receipt { result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_ids_parse_uuids_only() {
        let id = OperationId::parse("2f1c5a1e-6b4e-4b0e-9d3a-0f3b0d7c1a2b").unwrap();
        assert_eq!(id.to_string(), "2f1c5a1e-6b4e-4b0e-9d3a-0f3b0d7c1a2b");
        assert_eq!(
            serde_json::to_value(id).unwrap(),
            serde_json::json!("2f1c5a1e-6b4e-4b0e-9d3a-0f3b0d7c1a2b")
        );
        assert_eq!(OperationId::parse("not-a-uuid"), Err(InvalidOperationId));
        assert_eq!(OperationId::parse(""), Err(InvalidOperationId));
    }

    #[test]
    fn scopes_name_what_the_receipt_is_unique_within() {
        assert_eq!(Scope::Workspace.to_string(), "workspace");
        assert_eq!(
            Scope::Todo {
                workspace: "local".into()
            }
            .to_string(),
            "todo/local"
        );
        assert_eq!(
            Scope::Calendar {
                workspace: "local".into()
            }
            .to_string(),
            "calendar/local"
        );
    }

    #[sqlx::test]
    async fn replays_same_request_conflicts_on_changed_request_and_runs_once(pool: sqlx::PgPool) {
        let op = OperationId::parse("2f1c5a1e-6b4e-4b0e-9d3a-0f3b0d7c1a2b").unwrap();
        let scope = Scope::Todo {
            workspace: "local".into(),
        };
        let request = serde_json::json!({"kind":"add","title":"Water the plants"});
        let mut tx = pool.begin().await.unwrap();
        let first = execute(&mut tx, scope.clone(), op, &request, |tx| {
            Box::pin(async move {
                sqlx::query("SELECT 1").execute(&mut **tx).await?;
                Ok("rule-1".to_string())
            })
        })
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(first.result, "rule-1");

        // Replay: apply must not run again, the stored result comes back.
        let mut tx = pool.begin().await.unwrap();
        let replay: Receipt<String> = execute(&mut tx, scope.clone(), op, &request, |_| {
            Box::pin(async { panic!("replayed command must not run") })
        })
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(replay.result, "rule-1");

        // Changed request under the same ID conflicts.
        let mut tx = pool.begin().await.unwrap();
        let changed = serde_json::json!({"kind":"save","name":"weekly"});
        let conflict = execute::<String, _>(&mut tx, scope.clone(), op, &changed, |_| {
            Box::pin(async { panic!("conflicting command must not run") })
        })
        .await
        .unwrap_err();
        assert_eq!(conflict.status(), axum::http::StatusCode::CONFLICT);

        // Another scope is another ID space.
        let mut tx = pool.begin().await.unwrap();
        let other = execute(
            &mut tx,
            Scope::Todo {
                workspace: "other".into(),
            },
            op,
            &request,
            |_| Box::pin(async { Ok("rule-2".to_string()) }),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(other.result, "rule-2");

        // Concurrent repeats serialize on the lock and both see one result.
        let (a, b) = tokio::join!(
            async {
                let mut tx = pool.begin().await.unwrap();
                let r = execute(&mut tx, Scope::Workspace, op, &request, |_| {
                    Box::pin(async { Ok(Some(7_i64)) })
                })
                .await
                .unwrap();
                tx.commit().await.unwrap();
                r.result
            },
            async {
                let mut tx = pool.begin().await.unwrap();
                let r = execute(&mut tx, Scope::Workspace, op, &request, |_| {
                    Box::pin(async { Ok(Some(7_i64)) })
                })
                .await
                .unwrap();
                tx.commit().await.unwrap();
                r.result
            }
        );
        assert_eq!(a, Some(7));
        assert_eq!(b, Some(7));
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM receipts WHERE scope='workspace'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1);
    }
}
