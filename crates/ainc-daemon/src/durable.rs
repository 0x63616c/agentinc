//! Durable actions: product effects that must survive restarts and retry
//! safely. A command commits a `durable_actions` row with its receipt; this
//! runner starts a durable task named `<kind>-<id>` for each new row, and the
//! task applies the row's idempotent effect and records the outcome.
use crate::{home::Home, tickets::Actor};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection, PgPool, postgres::PgListener};
use std::{sync::Arc, time::Duration};
use turnkeel::{Runtime, RuntimeConfig, Task, TaskError, TaskHandler};
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
    /// `None` for a kind this daemon does not know, such as one written by a
    /// newer daemon before a rollback.
    fn parse(kind: &str) -> Option<Self> {
        match kind {
            "home" => Some(Self::Home),
            "calendar_import" => Some(Self::CalendarImport),
            _ => None,
        }
    }
    /// The task ID prefix, so run history names what each action is.
    fn task_prefix(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::CalendarImport => "calendar-import",
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

/// Where an action is in its life.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ActionState {
    Queued,
    Running,
    Completed,
    Failed,
    /// A newer action made this one moot before it applied.
    Superseded,
}

#[derive(Clone, Debug, FromRow)]
pub(crate) struct ActionRow {
    pub id: String,
    pub input: Value,
    pub summary: String,
    pub state: ActionState,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}
impl ActionRow {
    pub fn is_open(&self) -> bool {
        matches!(self.state, ActionState::Queued | ActionState::Running)
    }
    pub fn view(&self) -> ActionView {
        ActionView {
            id: self.id.clone(),
            summary: self.summary.clone(),
            state: self.state,
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
    pub state: ActionState,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

const COLUMNS: &str = "id,input,summary,state,error,created_at,finished_at";

/// Record an action inside the caller's command transaction and wake the runner.
pub(crate) async fn enqueue(
    tx: &mut PgConnection,
    actor: &Actor,
    kind: ActionKind,
    input: &Value,
    summary: &str,
    digest: Option<&str>,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO durable_actions(id,workspace_id,actor_id,kind,input,summary,digest) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(&id)
        .bind(&actor.workspace)
        .bind(&actor.id)
        .bind(kind.as_str())
        .bind(input)
        .bind(summary)
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
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM durable_actions WHERE workspace_id=$1 AND kind=$2 ORDER BY seq DESC LIMIT $3"))
        .bind(workspace)
        .bind(kind.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await
}
pub(crate) async fn latest_for(
    pool: &PgPool,
    actor: &Actor,
    kind: ActionKind,
) -> Result<Option<ActionRow>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM durable_actions WHERE workspace_id=$1 AND actor_id=$2 AND kind=$3 ORDER BY seq DESC LIMIT 1"))
        .bind(&actor.workspace)
        .bind(&actor.id)
        .bind(kind.as_str())
        .fetch_optional(pool)
        .await
}

/// What applying an action's effect found.
pub(crate) enum Applied {
    Done,
    /// A newer action already covers this one.
    Superseded,
}

/// Everything an action's effect may touch.
#[derive(Clone)]
pub struct Effects {
    pub pool: PgPool,
    pub home: Home,
}

async fn finish(
    pool: &PgPool,
    id: &str,
    state: ActionState,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let finished = matches!(
        state,
        ActionState::Completed | ActionState::Failed | ActionState::Superseded
    );
    sqlx::query("UPDATE durable_actions SET state=$2,error=$3,finished_at=CASE WHEN $4 THEN extract(epoch FROM now())::bigint END WHERE id=$1")
        .bind(id)
        .bind(state)
        .bind(error)
        .bind(finished)
        .execute(pool)
        .await?;
    sqlx::query("SELECT pg_notify('agentinc_actions','')")
        .execute(pool)
        .await?;
    Ok(())
}

/// Run one attempt. A `Retry` error asks the runtime to try again; a finished,
/// expired or unknown action ends without touching the outside world.
pub(crate) async fn attempt(effects: &Effects, id: &str) -> Result<(), TaskError> {
    let pool = &effects.pool;
    let retry = |error: sqlx::Error| TaskError::Retry(error.to_string());
    let row: Option<(String, String, Value, i64, i64)> = sqlx::query_as(
        "UPDATE durable_actions SET state='running' WHERE id=$1 AND state IN ('queued','running')
         RETURNING workspace_id,kind,input,seq,extract(epoch FROM now())::bigint-created_at",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(retry)?;
    let Some((workspace, kind, input, seq, age)) = row else {
        return Ok(()); // Already finished; a repeated attempt has nothing to do.
    };
    let Some(kind) = ActionKind::parse(&kind) else {
        let message = format!("This daemon cannot run a {kind} action.");
        finish(pool, id, ActionState::Failed, Some(&message))
            .await
            .map_err(retry)?;
        return Err(TaskError::Permanent(message));
    };
    // Expiry is decided before the effect, so a stale switch never reaches the lights.
    if kind.expires_after().is_some_and(|limit| age >= limit) {
        let error: Option<String> =
            sqlx::query_scalar("SELECT error FROM durable_actions WHERE id=$1")
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(retry)?;
        let message = error.unwrap_or_else(|| "Expired before it could be applied.".into());
        finish(pool, id, ActionState::Failed, Some(&message))
            .await
            .map_err(retry)?;
        return Ok(());
    }
    let result = match kind {
        ActionKind::Home => crate::home::apply(pool, &effects.home, &workspace, seq, &input).await,
        ActionKind::CalendarImport => crate::calendar::apply_import(pool, id).await,
    };
    match result {
        Ok(Applied::Done) => finish(pool, id, ActionState::Completed, None).await,
        Ok(Applied::Superseded) => finish(pool, id, ActionState::Superseded, None).await,
        Err(error) => {
            sqlx::query("UPDATE durable_actions SET error=$2 WHERE id=$1")
                .bind(id)
                .bind(error.to_string())
                .execute(pool)
                .await
                .map_err(retry)?;
            return Err(TaskError::Retry(error.to_string()));
        }
    }
    .map_err(retry)
}

struct Executor(Effects);
impl TaskHandler for Executor {
    fn run(&self, task: Task) -> BoxFuture<'static, Result<(), TaskError>> {
        let effects = self.0.clone();
        Box::pin(async move {
            let id = task.input["id"]
                .as_str()
                .ok_or_else(|| TaskError::Permanent("missing action ID".into()))?
                .to_owned();
            attempt(&effects, &id).await
        })
    }
}

/// Starts a durable task for every undispatched action. One daemon owns this.
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
        let runtime = Runtime::tasks(config, Arc::new(Executor(effects))).await?;
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
    /// Start each new action. A start that fails stays undispatched with its
    /// reason and is tried again on the next pass; the daemon keeps serving.
    pub(crate) async fn dispatch(&self) -> anyhow::Result<()> {
        let pending: Vec<(String, String)> =
            sqlx::query_as("SELECT id,kind FROM durable_actions WHERE NOT dispatched ORDER BY seq")
                .fetch_all(&self.pool)
                .await?;
        for (id, kind) in pending {
            let prefix = ActionKind::parse(&kind).map_or("action", ActionKind::task_prefix);
            let started = self
                .runtime
                .start_task(Task {
                    id: format!("{prefix}-{id}"),
                    input: json!({ "id": id }),
                })
                .await;
            match started {
                Ok(()) => {
                    sqlx::query("UPDATE durable_actions SET dispatched=true WHERE id=$1")
                        .bind(&id)
                        .execute(&self.pool)
                        .await?;
                }
                Err(error) => {
                    tracing::warn!(action = %id, %error, "durable action waiting for the runtime");
                    sqlx::query("UPDATE durable_actions SET error=$2 WHERE id=$1")
                        .bind(&id)
                        .bind(format!("Waiting for the runtime: {error}"))
                        .execute(&self.pool)
                        .await?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    async fn queued(pool: &PgPool, age: i64, kind: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO durable_actions(id,workspace_id,actor_id,kind,input,summary,created_at) VALUES($1,'local','owner',$2,$3,'fixture',extract(epoch FROM now())::bigint-$4)")
            .bind(&id)
            .bind(kind)
            .bind(json!({"kind":"switch","key":"lamps","on":true}))
            .bind(age)
            .execute(pool)
            .await
            .unwrap();
        id
    }
    async fn state(pool: &PgPool, id: &str) -> (ActionState, Option<String>, Option<i64>) {
        sqlx::query_as("SELECT state,error,finished_at FROM durable_actions WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }
    fn effects(pool: &PgPool) -> Effects {
        Effects {
            pool: pool.clone(),
            home: Home::new(Arc::new(MemoryStore::default())),
        }
    }

    #[sqlx::test]
    async fn a_failing_switch_retries_while_wanted_and_never_applies_once_stale(pool: PgPool) {
        // Port 9 (discard) refuses connections, so every attempt fails fast.
        sqlx::query("INSERT INTO home_connections(workspace_id,base_url) VALUES('local','http://127.0.0.1:9')")
            .execute(&pool)
            .await
            .unwrap();
        let effects = effects(&pool);
        let fresh = queued(&pool, 0, "home").await;
        assert!(matches!(
            attempt(&effects, &fresh).await,
            Err(TaskError::Retry(_))
        ));
        let (now, error, finished) = state(&pool, &fresh).await;
        assert_eq!((now, finished), (ActionState::Running, None));
        assert!(error.unwrap().contains("unreachable"));

        // A switch that waited out its window fails without being attempted.
        let stale = queued(&pool, 31, "home").await;
        attempt(&effects, &stale).await.unwrap();
        let (now, error, finished) = state(&pool, &stale).await;
        assert_eq!(now, ActionState::Failed);
        assert_eq!(
            error.as_deref(),
            Some("Expired before it could be applied.")
        );
        assert!(finished.is_some());
        // A late repeat of a finished action changes nothing.
        attempt(&effects, &stale).await.unwrap();
        assert_eq!(state(&pool, &stale).await.0, ActionState::Failed);
    }

    #[sqlx::test]
    async fn an_unknown_kind_fails_for_good(pool: PgPool) {
        sqlx::query("ALTER TABLE durable_actions DROP CONSTRAINT durable_actions_kind_check")
            .execute(&pool)
            .await
            .unwrap();
        let id = queued(&pool, 0, "future_kind").await;
        assert!(matches!(
            attempt(&effects(&pool), &id).await,
            Err(TaskError::Permanent(_))
        ));
        assert_eq!(state(&pool, &id).await.0, ActionState::Failed);
    }

    /// A control center that records each switch it is asked to set.
    async fn recording_control_center(pool: &PgPool) -> Arc<std::sync::Mutex<Vec<Value>>> {
        use axum::{Json, Router, routing::post};
        let calls: Arc<std::sync::Mutex<Vec<Value>>> = Arc::default();
        let recorded = calls.clone();
        let app = Router::new().route(
            "/trpc/controls.toggle",
            post(move |Json(input): Json<Value>| {
                let recorded = recorded.clone();
                async move {
                    recorded.lock().unwrap().push(input);
                    Json(json!({"result": {"data": {}}}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        sqlx::query("INSERT INTO home_connections(workspace_id,base_url) VALUES('local',$1)")
            .bind(url)
            .execute(pool)
            .await
            .unwrap();
        calls
    }
    async fn switch(pool: &PgPool, key: &str, on: bool) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO durable_actions(id,workspace_id,actor_id,kind,input,summary) VALUES($1,'local','owner','home',$2,'fixture')")
            .bind(&id)
            .bind(json!({"kind":"switch","key":key,"on":on}))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    #[sqlx::test]
    async fn smart_home_changes_apply_in_the_order_they_were_made(pool: PgPool) {
        let calls = recording_control_center(&pool).await;
        let effects = effects(&pool);
        let everything_off = switch(&pool, "all", false).await;
        let bedroom_on = switch(&pool, "bedroom_lamps", true).await;
        // The newer change waits while the older one is unfinished.
        assert!(matches!(
            attempt(&effects, &bedroom_on).await,
            Err(TaskError::Retry(_))
        ));
        assert!(calls.lock().unwrap().is_empty());
        attempt(&effects, &everything_off).await.unwrap();
        attempt(&effects, &bedroom_on).await.unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            [
                json!({"key":"all","on":false}),
                json!({"key":"bedroomLamps","on":true})
            ]
        );
        assert_eq!(state(&pool, &bedroom_on).await.0, ActionState::Completed);
    }

    #[sqlx::test]
    async fn a_newer_change_that_covers_an_older_one_supersedes_it(pool: PgPool) {
        let calls = recording_control_center(&pool).await;
        let effects = effects(&pool);
        let bedroom_on = switch(&pool, "bedroom_lamps", true).await;
        let everything_off = switch(&pool, "all", false).await;
        attempt(&effects, &bedroom_on).await.unwrap();
        assert_eq!(state(&pool, &bedroom_on).await.0, ActionState::Superseded);
        attempt(&effects, &everything_off).await.unwrap();
        assert_eq!(*calls.lock().unwrap(), [json!({"key":"all","on":false})]);
    }
}
