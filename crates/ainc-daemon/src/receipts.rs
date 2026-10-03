//! Command receipts. Every mutating command carries a client-chosen operation
//! ID; the committed receipt is the acknowledgement. Repeating an operation
//! with the same request replays its result, repeating it with a different
//! request is a conflict. One table, one lock-key rule, one hash rule.
use std::fmt;

use axum::http::StatusCode;
use futures::future::BoxFuture;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};

use crate::product::ApiError;

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
    /// Derive a stable operation ID from a tool's idempotency key, so a
    /// retried tool call replays the receipt of its first attempt.
    pub fn from_idempotency_key(key: &str) -> Self {
        let digest = Sha256::digest(key.as_bytes());
        Self(uuid::Uuid::from_bytes(
            digest[..16].try_into().expect("SHA-256 has 32 bytes"),
        ))
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
impl From<InvalidOperationId> for ApiError {
    fn from(_: InvalidOperationId) -> Self {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_operation",
            "Use a UUID operation ID.",
        )
    }
}

/// The command family an operation ID is unique within.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Product commands: one ID space across workspaces; the workspace is part
    /// of the request, so reuse from another workspace conflicts.
    Product,
    /// Ticket commands: one ID space per actor per workspace.
    Ticket { workspace: String, actor: String },
    /// Automation commands: one ID space per workspace.
    Automation { workspace: String },
    /// Workspace commands: one ID space.
    Workspace,
}
impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scope::Product => f.write_str("product"),
            Scope::Ticket { workspace, actor } => write!(f, "ticket/{workspace}/{actor}"),
            Scope::Automation { workspace } => write!(f, "automation/{workspace}"),
            Scope::Workspace => f.write_str("workspace"),
        }
    }
}

/// A committed command and its result.
#[derive(Clone, Debug)]
pub(crate) struct Receipt<R> {
    pub operation_id: OperationId,
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
) -> Result<Receipt<R>, ApiError>
where
    R: Serialize + DeserializeOwned,
    F: for<'c> FnOnce(&'c mut Transaction<'t, Postgres>) -> BoxFuture<'c, Result<R, ApiError>>,
{
    let request = serde_json::to_value(request)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid", "Invalid command."))?;
    let scope = scope.to_string();
    let operation = operation_id.to_string();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("receipt/{scope}/{operation}"))
        .execute(&mut **tx)
        .await?;
    let prior: Option<(bool, serde_json::Value)> = sqlx::query_as(
        "SELECT request_hash = receipt_hash($3), result FROM receipts WHERE scope=$1 AND operation_id=$2::uuid",
    )
    .bind(&scope)
    .bind(&operation)
    .bind(&request)
    .fetch_optional(&mut **tx)
    .await?;
    let result = match prior {
        Some((true, result)) => serde_json::from_value(result).map_err(|error| {
            tracing::error!(%error, scope, operation, "stored receipt result does not match its command");
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Data is unavailable. Try again.",
            )
        })?,
        Some((false, _)) => return Err(ApiError::conflict()),
        None => {
            let result = apply(tx).await?;
            sqlx::query(
                "INSERT INTO receipts(scope,operation_id,request_hash,result) VALUES ($1,$2::uuid,receipt_hash($3),$4)",
            )
            .bind(&scope)
            .bind(&operation)
            .bind(&request)
            .bind(serde_json::to_value(&result).expect("serializable result"))
            .execute(&mut **tx)
            .await?;
            result
        }
    };
    Ok(Receipt {
        operation_id,
        result,
    })
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
    fn idempotency_keys_derive_stable_distinct_ids() {
        let a = OperationId::from_idempotency_key("run-1/tool-call-7");
        assert_eq!(a, OperationId::from_idempotency_key("run-1/tool-call-7"));
        assert_ne!(a, OperationId::from_idempotency_key("run-1/tool-call-8"));
        // The first 16 bytes of SHA-256, as before the derivation was shared.
        assert_eq!(
            a.to_string(),
            uuid::Uuid::from_bytes(
                Sha256::digest(b"run-1/tool-call-7")[..16]
                    .try_into()
                    .unwrap()
            )
            .to_string()
        );
        assert_eq!(OperationId::parse(&a.to_string()), Ok(a));
    }

    #[test]
    fn scopes_name_what_the_receipt_is_unique_within() {
        assert_eq!(Scope::Product.to_string(), "product");
        assert_eq!(
            Scope::Ticket {
                workspace: "local".into(),
                actor: "owner".into()
            }
            .to_string(),
            "ticket/local/owner"
        );
        assert_eq!(
            Scope::Automation {
                workspace: "local".into()
            }
            .to_string(),
            "automation/local"
        );
        assert_eq!(Scope::Workspace.to_string(), "workspace");
    }

    #[sqlx::test]
    async fn replays_same_request_conflicts_on_changed_request_and_runs_once(pool: sqlx::PgPool) {
        let op = OperationId::parse("2f1c5a1e-6b4e-4b0e-9d3a-0f3b0d7c1a2b").unwrap();
        let scope = Scope::Automation {
            workspace: "local".into(),
        };
        let request = serde_json::json!({"kind":"save","name":"nightly"});
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
        assert_eq!(first.operation_id, op);

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
        assert_eq!(conflict.status(), StatusCode::CONFLICT);

        // Another scope is another ID space.
        let mut tx = pool.begin().await.unwrap();
        let other = execute(
            &mut tx,
            Scope::Automation {
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
