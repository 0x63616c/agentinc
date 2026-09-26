//! Durable actions: product effects that must survive restarts and retry
//! safely. A command commits a `durable_actions` row with its receipt; this
//! runner starts a Temporal workflow named `action-<id>` for each new row, and
//! the workflow applies the row's idempotent effect and records the outcome.
use crate::{home::Home, tickets::Actor};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection, PgPool, postgres::PgListener};
use std::{sync::Arc, time::Duration};
use turnkeel::{Occurrence, RecurringAction, Runtime, RuntimeConfig};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionKind {
    Home,
    CalendarImport,
}
impl ActionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::CalendarImport => "calendar_import",
        }
    }
    /// How long an effect stays wanted. A light switched a minute ago must not
    /// flip when the control center comes back; an import is always wanted.
    fn expires_after(self) -> Option<i64> {
        match self {
            Self::Home => Some(30),
            Self::CalendarImport => None,
        }
    }
}

#[derive(Clone, Debug, FromRow)]
pub(crate) struct ActionRow {
    pub id: String,
    pub kind: String,
    pub input: Value,
    pub state: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}
impl ActionRow {
    pub fn is_open(&self) -> bool {
        matches!(self.state.as_str(), "queued" | "running")
    }
    pub fn view(&self) -> ActionView {
        let summary = match self.kind.as_str() {
            "home" => serde_json::from_value::<crate::home::HomeCommand>(self.input.clone())
                .map_or_else(|_| "Smart Home change".into(), |c| c.summary()),
            _ => {
                let count = self.input["events"].as_array().map_or(0, Vec::len);
                format!(
                    "Import {count} {}",
                    if count == 1 { "event" } else { "events" }
                )
            }
        };
        ActionView {
            id: self.id.clone(),
            summary,
            state: self.state.clone(),
            error: self.error.clone(),
            created_at: self.created_at,
            finished_at: self.finished_at,
        }
    }
}
/// One durable action as people see it.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq)]
pub struct ActionView {
    pub id: String,
    pub summary: String,
    /// queued, running, completed or failed.
    pub state: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

/// Record an action inside the caller's command transaction and wake the runner.
pub(crate) async fn enqueue(
    tx: &mut PgConnection,
    actor: &Actor,
    kind: ActionKind,
    input: &Value,
    digest: Option<&str>,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO durable_actions(id,workspace_id,actor_id,kind,input,digest) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(&id)
        .bind(&actor.workspace)
        .bind(&actor.id)
        .bind(kind.as_str())
        .bind(input)
        .bind(digest)
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_notify('agentinc_actions','')")
        .execute(&mut *tx)
        .await?;
    Ok(id)
}

pub(crate) async fn recent(
    pool: &PgPool,
    workspace: &str,
    kind: ActionKind,
    limit: i64,
) -> Result<Vec<ActionRow>, sqlx::Error> {
    sqlx::query_as("SELECT id,kind,input,state,error,created_at,finished_at FROM durable_actions WHERE workspace_id=$1 AND kind=$2 ORDER BY seq DESC LIMIT $3")
        .bind(workspace)
        .bind(kind.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await
}

/// Everything an action's effect may touch.
#[derive(Clone)]
pub struct Effects {
    pub pool: PgPool,
    pub home: Home,
}

/// Run one action attempt. `Err` asks Temporal to retry; an expired or
/// finished action returns `Ok` so the workflow ends.
pub(crate) async fn attempt(effects: &Effects, id: &str) -> anyhow::Result<()> {
    let pool = &effects.pool;
    let row: Option<(String, String, Value)> = sqlx::query_as(
        "UPDATE durable_actions SET state='running' WHERE id=$1 AND state IN ('queued','running') RETURNING workspace_id,kind,input",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some((workspace, kind, input)) = row else {
        return Ok(()); // Already finished; a retried workflow has nothing to do.
    };
    let kind = match kind.as_str() {
        "home" => ActionKind::Home,
        _ => ActionKind::CalendarImport,
    };
    let result = match kind {
        ActionKind::Home => crate::home::apply(pool, &effects.home, &workspace, &input).await,
        ActionKind::CalendarImport => crate::calendar::apply_import(pool, id).await,
    };
    let error = match result {
        Ok(()) => None,
        Err(error) => Some(error.to_string()),
    };
    let expired: bool = sqlx::query_scalar(
        "SELECT extract(epoch FROM now())::bigint-created_at >= $2 FROM durable_actions WHERE id=$1",
    )
    .bind(id)
    .bind(kind.expires_after().unwrap_or(i64::MAX))
    .fetch_one(pool)
    .await?;
    let state = match (&error, expired) {
        (None, _) => "completed",
        (Some(_), true) => "failed",
        (Some(_), false) => "running",
    };
    sqlx::query("UPDATE durable_actions SET state=$2,error=$3,finished_at=CASE WHEN $2 IN ('completed','failed') THEN extract(epoch FROM now())::bigint END WHERE id=$1 AND state='running'")
        .bind(id)
        .bind(state)
        .bind(&error)
        .execute(pool)
        .await?;
    sqlx::query("SELECT pg_notify('agentinc_actions','')")
        .execute(pool)
        .await?;
    match (error, state) {
        (Some(error), "running") => Err(anyhow::anyhow!(error)),
        _ => Ok(()),
    }
}

struct Executor(Effects);
impl RecurringAction for Executor {
    fn execute(&self, occurrence: Occurrence) -> BoxFuture<'static, Result<(), turnkeel::Error>> {
        let effects = self.0.clone();
        Box::pin(async move {
            let id = occurrence.input["id"]
                .as_str()
                .ok_or_else(|| turnkeel::Error::Other(anyhow::anyhow!("missing action ID")))?
                .to_owned();
            attempt(&effects, &id).await.map_err(turnkeel::Error::Other)
        })
    }
}

/// Starts a workflow for every undispatched action. One daemon owns this.
pub struct Runner {
    pool: PgPool,
    runtime: Runtime,
    listener: PgListener,
    owner: PgConnection,
}
impl Runner {
    pub async fn start(effects: Effects, mut config: RuntimeConfig) -> anyhow::Result<Self> {
        let pool = effects.pool.clone();
        let mut owner = pool.acquire().await?.detach();
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(7358710505)")
            .fetch_one(&mut owner)
            .await?;
        anyhow::ensure!(locked, "another daemon owns durable actions");
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen("agentinc_actions").await?;
        config.worker_group.push_str("-actions");
        let runtime = Runtime::recurring(config, Arc::new(Executor(effects))).await?;
        Ok(Self {
            pool,
            runtime,
            listener,
            owner,
        })
    }
    pub async fn run_until(
        mut self,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> anyhow::Result<()> {
        tokio::pin!(shutdown);
        loop {
            sqlx::query("SELECT 1").execute(&mut self.owner).await?;
            self.dispatch().await?;
            tokio::select! {
                _ = &mut shutdown => { self.runtime.shutdown().await?; return Ok(()); }
                result = self.listener.recv() => { result?; }
                _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            }
        }
    }
    pub(crate) async fn dispatch(&self) -> anyhow::Result<()> {
        let pending: Vec<String> =
            sqlx::query_scalar("SELECT id FROM durable_actions WHERE NOT dispatched ORDER BY seq")
                .fetch_all(&self.pool)
                .await?;
        for id in pending {
            self.runtime
                .run_occurrence(Occurrence {
                    id: format!("action-{id}"),
                    input: json!({ "id": id }),
                })
                .await?;
            sqlx::query("UPDATE durable_actions SET dispatched=true WHERE id=$1")
                .bind(&id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}
