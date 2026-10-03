//! Durable SDK sessions with a Postgres message outbox and event projection.
use super::tools::{Automations, CommandTool, ReadTool, Tickets};
use crate::{
    execution::ModelCatalog,
    pg::coordination,
    worker::{Leased, Reconcile, Tasks},
};
use anyhow::Result;
use futures::{StreamExt, future::BoxFuture};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use turnkeel::{
    Agent, AgentSource, Content, Event, Message, Role, Runtime, RuntimeConfig, SessionId,
};

#[derive(Clone)]
struct StoredSession {
    id: String,
    model: String,
    history: sqlx::types::Json<Vec<Message>>,
    event_offset: i64,
}
/// `query_as!` of a [`StoredSession`]: its columns, then `$tail` (a literal starting
/// at the table), then the query's arguments.
macro_rules! select_session {
    ($tail:literal $(, $arg:expr)* $(,)?) => {
        sqlx::query_as!(
            StoredSession,
            r#"SELECT id,model,history AS "history: sqlx::types::Json<Vec<Message>>",event_offset FROM conversation_sessions "#
                + $tail
            $(, $arg)*
        )
    };
}

/// Each Conversation session acts through its own agent definition, named after the
/// session so its tools know which Conversation they speak for. Workers ask for the
/// definition by name instead of preloading every active session at startup.
#[derive(Clone)]
struct ConversationAgents {
    pool: PgPool,
    models: Arc<dyn ModelCatalog>,
}
fn agent_name(session_id: &str) -> String {
    format!("conversation-{session_id}-v1")
}
fn session_of(agent_name: &str) -> Option<&str> {
    agent_name
        .strip_prefix("conversation-")?
        .strip_suffix("-v1")
}
impl AgentSource for ConversationAgents {
    fn resolve(&self, name: &str) -> BoxFuture<'static, Result<Option<Agent>, turnkeel::Error>> {
        let this = self.clone();
        let session_id = session_of(name).map(str::to_owned);
        Box::pin(async move {
            let Some(session_id) = session_id else {
                return Ok(None);
            };
            let session = select_session!("WHERE id=$1", session_id)
                .fetch_optional(&this.pool)
                .await
                .map_err(|error| turnkeel::Error::Other(error.into()))?;
            session
                .map(|s| s.agent(&this.pool, this.models.as_ref()))
                .transpose()
                .map_err(turnkeel::Error::Other)
        })
    }
}
impl StoredSession {
    fn agent(&self, pool: &PgPool, models: &dyn ModelCatalog) -> Result<Agent> {
        Ok(Agent::builder(agent_name(&self.id))
            .model(models.resolve(&self.model)?)
            .instructions("You are Evee, the personal assistant in AgentInc. Use Ticket tools for requested product changes. Autonomous work belongs on an assigned Ticket; do not claim to perform external actions without tools.")
            .tool(ReadTool::<Tickets>::new(pool, &self.id))
            .tool(CommandTool::<Tickets>::new(pool, &self.id))
            .tool(ReadTool::<Automations>::new(pool, &self.id))
            .tool(CommandTool::<Automations>::new(pool, &self.id))
            .build())
    }
}
pub struct Runner {
    lease: Leased,
    turns: Turns,
}
struct Turns {
    pool: PgPool,
    runtime: Arc<Runtime>,
    models: Arc<dyn ModelCatalog>,
}
impl Runner {
    pub async fn start(
        pool: PgPool,
        mut config: RuntimeConfig,
        models: Arc<dyn ModelCatalog>,
    ) -> Result<Self> {
        let lease = Leased::acquire(
            &pool,
            coordination::CONVERSATION_LOCK,
            &[coordination::TURNS],
            Duration::from_secs(1),
            "another daemon owns Conversation execution",
        )
        .await?;
        config.worker_group.push_str("-conversations");
        let runtime = Arc::new(
            Runtime::configured_with(
                config,
                ConversationAgents {
                    pool: pool.clone(),
                    models: models.clone(),
                },
            )
            .await?,
        );
        Ok(Self {
            lease,
            turns: Turns {
                pool,
                runtime,
                models,
            },
        })
    }
    pub async fn run(self) -> Result<()> {
        self.run_until(std::future::pending()).await
    }
    pub async fn run_until(self, shutdown: impl std::future::Future<Output = ()>) -> Result<()> {
        self.lease.run_until(self.turns, shutdown).await
    }
}
impl Reconcile for Turns {
    async fn reconcile(&mut self, tasks: &mut Tasks) -> Result<()> {
        let closed = select_session!("WHERE state='closed'")
            .fetch_all(&self.pool)
            .await?;
        for session in closed {
            let agent = session.agent(&self.pool, self.models.as_ref())?;
            match self
                .runtime
                .session_by_id(&agent, SessionId::new(&session.id))
                .cancel()
                .await
            {
                Ok(()) | Err(turnkeel::Error::NotFound) => {}
                Err(error) => return Err(error.into()),
            }
            sqlx::query!(
                "UPDATE conversation_sessions SET state='cancelled' WHERE id=$1 AND state='closed'",
                session.id
            )
            .execute(&self.pool)
            .await?;
        }
        let pending = sqlx::query_scalar!(
            "SELECT id FROM turns WHERE state IN ('queued','running') ORDER BY id"
        )
        .fetch_all(&self.pool)
        .await?;
        for id in pending {
            let pool = self.pool.clone();
            let runtime = self.runtime.clone();
            let models = self.models.clone();
            tasks.spawn(id.to_string(), async move {
                drive(&pool, &runtime, models.as_ref(), id).await
            });
        }
        Ok(())
    }
    async fn drain(self) -> Result<()> {
        Arc::try_unwrap(self.runtime)
            .map_err(|_| anyhow::anyhow!("runtime still owned during drain"))?
            .shutdown()
            .await?;
        Ok(())
    }
}

async fn prepare(pool: &PgPool, id: i64) -> Result<(StoredSession, String, i64)> {
    let mut tx = pool.begin().await?;
    let parent = sqlx::query_scalar!("SELECT conversation_id FROM turns WHERE id=$1", id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query!(
        "SELECT id FROM conversations WHERE id=$1 FOR UPDATE",
        parent
    )
    .fetch_optional(&mut *tx)
    .await?;
    // The Conversation is locked above; a turn never changes its Conversation.
    let conversation = parent;
    let turn = sqlx::query!(
        "SELECT prompt,model,session_id,attempt FROM turns WHERE id=$1 FOR UPDATE",
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    let (prompt, attempt) = (turn.prompt, turn.attempt);
    let model = turn
        .model
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "connection-default".into());
    let mut session = if let Some(session_id) = turn.session_id {
        select_session!("WHERE id=$1", session_id)
            .fetch_optional(&mut *tx)
            .await?
    } else {
        select_session!(
            "WHERE conversation_id=$1 AND model=$2 AND state='active'",
            conversation,
            model
        )
        .fetch_optional(&mut *tx)
        .await?
    };
    if session.is_none() {
        sqlx::query!(
            "UPDATE conversation_sessions SET state='closed' WHERE conversation_id=$1 AND state='active'",
            conversation
        )
        .execute(&mut *tx)
        .await?;
        let prior = sqlx::query!(
            r#"SELECT prompt,response AS "response!" FROM turns WHERE conversation_id=$1 AND id<$2 AND state='completed' ORDER BY id"#,
            conversation,
            id
        )
        .fetch_all(&mut *tx)
        .await?;
        let history = prior
            .into_iter()
            .flat_map(|turn| {
                [
                    Message::user(turn.prompt),
                    Message::assistant(vec![Content::Text {
                        text: turn.response,
                    }]),
                ]
            })
            .collect::<Vec<_>>();
        let new = StoredSession {
            id: uuid::Uuid::new_v4().to_string(),
            model,
            history: sqlx::types::Json(history),
            event_offset: 0,
        };
        sqlx::query!(
            "INSERT INTO conversation_sessions(id,conversation_id,model,history,state) VALUES ($1,$2,$3,$4,'active')",
            new.id,
            conversation,
            new.model,
            &new.history as _
        )
        .execute(&mut *tx)
        .await?;
        session = Some(new);
    }
    let session = session.expect("existing or newly inserted session");
    sqlx::query!(
        "UPDATE turns SET state='running',session_id=$2 WHERE id=$1 AND state IN ('queued','running')",
        id,
        session.id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((session, prompt, attempt))
}

async fn drive(pool: &PgPool, runtime: &Runtime, models: &dyn ModelCatalog, id: i64) -> Result<()> {
    let (stored, prompt, attempt) = prepare(pool, id).await?;
    let agent = stored.agent(pool, models)?;
    let session = runtime
        .open_session(SessionId::new(&stored.id), &agent, stored.history.0.clone())
        .await?;
    session
        .send_once(format!("turn-{id}-{attempt}"), prompt)
        .await?;
    let mut offset = stored.event_offset;
    let mut events = session.events_from(usize::try_from(offset)?);
    while let Some(event) = events.next().await {
        match event {
            Ok(event) => {
                let ended = event == Event::TurnEnded;
                let mut tx = pool.begin().await?;
                let advanced = sqlx::query!(
                    "UPDATE conversation_sessions SET event_offset=event_offset+1 WHERE id=$1 AND event_offset=$2",
                    stored.id,
                    offset
                )
                .execute(&mut *tx)
                .await?
                .rows_affected();
                if advanced != 0 {
                    if let Event::Message(message) = event
                        && message.role == Role::Assistant
                    {
                        sqlx::query!(
                            "UPDATE turns SET response=COALESCE(response,'')||$2 WHERE id=$1 AND state='running' AND attempt=$3",
                            id,
                            message.text(),
                            attempt
                        )
                        .execute(&mut *tx)
                        .await?;
                    }
                    if ended {
                        sqlx::query!(
                            "UPDATE turns SET state='completed',response=COALESCE(response,''),error=NULL WHERE id=$1 AND state='running' AND attempt=$2",
                            id,
                            attempt
                        )
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query!(
                            "SELECT pg_notify($1,$2)",
                            coordination::RESULTS,
                            id.to_string()
                        )
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                tx.commit().await?;
                offset += 1;
                if ended {
                    return Ok(());
                }
            }
            Err(turnkeel::Error::RunFailed(error)) => {
                let mut tx = pool.begin().await?;
                sqlx::query!(
                    "UPDATE turns SET state='failed',error=$2 WHERE id=$1 AND state='running' AND attempt=$3",
                    id,
                    error,
                    attempt
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query!(
                    "UPDATE conversation_sessions SET state='failed' WHERE id=$1",
                    stored.id
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query!(
                    "SELECT pg_notify($1,$2)",
                    coordination::RESULTS,
                    id.to_string()
                )
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                return Ok(());
            }
            Err(error) => return Err(error.into()), // Infrastructure outages retain pending work.
        }
    }
    anyhow::bail!("Conversation event stream ended before its turn")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversations::{
        ConversationCommand as Command, ConversationCommandRequest as CommandRequest,
        tools::{CommandTool, Tickets},
    };
    use serde_json::json;
    use turnkeel::{Model, ModelError};
    use turnkeel::{
        Tool, ToolCtx,
        testing::{ScriptedModel, Server, text, tool_call},
    };
    struct Models(Arc<dyn Model>);
    impl ModelCatalog for Models {
        fn resolve(&self, _: &str) -> Result<Arc<dyn Model>, ModelError> {
            Ok(self.0.clone())
        }
    }
    async fn apply(pool: &PgPool, command: Command) -> Option<i64> {
        super::super::execute(
            pool,
            "local",
            CommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command,
            },
        )
        .await
        .unwrap()
        .result_id
    }
    #[sqlx::test]
    async fn evee_commands_share_receipts_and_replaced_sessions_keep_history(pool: PgPool) {
        let server = Server::start().await.unwrap();
        let models = Arc::new(Models(Arc::new(
            ScriptedModel::new()
                .on_user(
                    "Create",
                    tool_call(
                        "ticket_command",
                        json!({"command":{"kind":"create","title":"Requested in Conversation"}}),
                    ),
                )
                .on_tool_result("ticket_command", text("Created your Ticket"))
                .on_user("Again", text("History retained")),
        )));
        let runner = Runner::start(pool.clone(), server.config(), models.clone())
            .await
            .unwrap();
        let conversation = apply(&pool, Command::Create).await.unwrap();
        let first = apply(
            &pool,
            Command::Send {
                conversation_id: conversation,
                prompt: "Create".into(),
            },
        )
        .await
        .unwrap();
        drive(&pool, &runner.turns.runtime, models.as_ref(), first)
            .await
            .unwrap();
        let session: String = sqlx::query_scalar("SELECT session_id FROM turns WHERE id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tickets")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        // The Ticket and its history name the Conversation that asked for it.
        let origin: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT t.conversation_id,a.conversation_id FROM tickets t JOIN ticket_activity a ON a.ticket_id=t.id AND a.kind='created'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(origin, (Some(conversation), Some(conversation)));
        let tool = CommandTool::<Tickets>::new(&pool, &session);
        assert!(!tool.schema().to_string().contains("$ref"));
        assert!(tool.schema()["properties"]["command"]["oneOf"].is_array());
        let args = json!({"command":{"kind":"create","title":"Deduplicated"}});
        let receipt = tool
            .call(ToolCtx::new("fixture-command"), args.clone())
            .await
            .unwrap();
        assert_eq!(
            tool.call(ToolCtx::new("fixture-command"), args.clone())
                .await
                .unwrap(),
            receipt
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tickets")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
        apply(
            &pool,
            Command::SelectModel {
                model: "changed-fixture".into(),
            },
        )
        .await;
        let second = apply(
            &pool,
            Command::Send {
                conversation_id: conversation,
                prompt: "Again".into(),
            },
        )
        .await
        .unwrap();
        drive(&pool, &runner.turns.runtime, models.as_ref(), second)
            .await
            .unwrap();
        let restored: sqlx::types::Json<Vec<Message>> =
            sqlx::query_scalar("SELECT history FROM conversation_sessions WHERE state='active'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(restored.0.len(), 2);
        assert_eq!(restored.0[1].text(), "Created your Ticket");
        assert!(tool.call(ToolCtx::new("after-close"), args).await.is_err());
        apply(&pool, Command::Delete { id: conversation }).await;
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM conversation_sessions WHERE conversation_id IS NULL AND state='closed'").fetch_one(&pool).await.unwrap(),2);
        drop(runner);
        server.shutdown().await.unwrap();
    }
}
