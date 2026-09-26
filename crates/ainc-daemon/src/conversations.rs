//! Durable SDK sessions with a Postgres message outbox and event projection.
use crate::{agent_tools, execution::ModelCatalog, providers::DeltaSink};
use anyhow::Result;
use futures::{StreamExt, future::BoxFuture};
use sqlx::{FromRow, PgPool, postgres::PgListener};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};
use turnkeel::{
    Agent, Content, Event, Message, Model, ModelError, ModelRequest, ModelResponse, Role, Runtime,
    RuntimeConfig, SessionId,
};

pub const INSTRUCTIONS: &str = "You are Evee, the coordinating assistant in AgentInc, a personal operating system where the user and agents share the same constructs: Tickets (tracked work with Backlog, To do, In progress and Done), Automations (scheduled rules that create Tickets), Conversations and Runs (durable executions of every turn, Ticket and Automation). Use tools for anything real: list_tickets and ticket_command to read and change Tickets, list_automations and automation_command for rules, list_runs to inspect running or finished work, and http_request for web APIs and pages. Autonomous file, shell and git work belongs on an assigned Ticket. Never claim an action or a fact from the outside world without a tool result; say what you could not do. Answer in Markdown, concise and direct.";

#[derive(Clone, FromRow)]
struct StoredSession {
    id: String,
    model: String,
    history: sqlx::types::Json<Vec<Message>>,
    event_offset: i64,
    workspace_id: String,
}
#[derive(Clone)]
struct SharedModel(Arc<dyn Model>);
impl Model for SharedModel {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
        self.0.complete(request)
    }
}
impl StoredSession {
    fn agent(
        &self,
        pool: &PgPool,
        models: &dyn ModelCatalog,
        config: &RuntimeConfig,
        sink: Option<DeltaSink>,
    ) -> Result<Agent> {
        let model = match sink {
            Some(sink) => models.resolve_streaming(&self.model, sink)?,
            None => models.resolve(&self.model)?,
        };
        let toolbox = agent_tools::Toolbox {
            pool: pool.clone(),
            config: config.clone(),
            sink: None,
        };
        Ok(Agent::builder(format!("conversation-{}-v2", self.id))
            .model(SharedModel(model))
            .instructions(INSTRUCTIONS)
            .tool(crate::conversation_tools::TicketsTool {
                pool: pool.clone(),
                session_id: self.id.clone(),
                mutation: false,
            })
            .tool(crate::conversation_tools::TicketsTool {
                pool: pool.clone(),
                session_id: self.id.clone(),
                mutation: true,
            })
            .tool(crate::conversation_tools::AutomationsTool {
                pool: pool.clone(),
                session_id: self.id.clone(),
                mutation: false,
            })
            .tool(crate::conversation_tools::AutomationsTool {
                pool: pool.clone(),
                session_id: self.id.clone(),
                mutation: true,
            })
            .tool(toolbox.http(&self.workspace_id))
            .tool(toolbox.runs())
            .build())
    }
}
pub struct Runner {
    pool: PgPool,
    owner: sqlx::PgConnection,
    listener: PgListener,
    runtime: Arc<Runtime>,
    models: Arc<dyn ModelCatalog>,
    config: RuntimeConfig,
}
impl Runner {
    pub async fn start(
        pool: PgPool,
        mut config: RuntimeConfig,
        models: Arc<dyn ModelCatalog>,
    ) -> Result<Self> {
        let mut owner = pool.acquire().await?.detach();
        let owned: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(7358710202)")
            .fetch_one(&mut owner)
            .await?;
        anyhow::ensure!(owned, "another daemon owns Conversation execution");
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen("agentinc_turns").await?;
        let sessions: Vec<StoredSession> = sqlx::query_as(SESSION_COLUMNS_ACTIVE)
            .fetch_all(&pool)
            .await?;
        config.worker_group.push_str("-conversations");
        let agents = sessions
            .iter()
            .map(|s| s.agent(&pool, models.as_ref(), &config, None))
            .collect::<Result<Vec<_>>>()?;
        let runtime = Arc::new(Runtime::configured(config.clone(), &agents).await?);
        Ok(Self {
            pool,
            owner,
            listener,
            runtime,
            models,
            config,
        })
    }
    pub async fn run(self) -> Result<()> {
        self.run_until(std::future::pending()).await
    }
    pub async fn run_until(
        mut self,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> Result<()> {
        tokio::pin!(shutdown);
        let mut active = HashSet::new();
        let mut turns = tokio::task::JoinSet::new();
        loop {
            let closed: Vec<StoredSession> = sqlx::query_as(SESSION_COLUMNS_CLOSED)
                .fetch_all(&self.pool)
                .await?;
            for session in closed {
                let agent = session.agent(&self.pool, self.models.as_ref(), &self.config, None)?;
                match self
                    .runtime
                    .session_by_id(&agent, SessionId::new(&session.id))
                    .cancel()
                    .await
                {
                    Ok(()) | Err(turnkeel::Error::NotFound) => {}
                    Err(error) => return Err(error.into()),
                }
                sqlx::query("UPDATE conversation_sessions SET state='cancelled' WHERE id=$1 AND state='closed'").bind(session.id).execute(&self.pool).await?;
            }
            let pending: Vec<i64> = sqlx::query_scalar(
                "SELECT id FROM turns WHERE state IN ('queued','running') ORDER BY id",
            )
            .fetch_all(&self.pool)
            .await?;
            for id in pending {
                if active.insert(id) {
                    let pool = self.pool.clone();
                    let runtime = self.runtime.clone();
                    let models = self.models.clone();
                    let config = self.config.clone();
                    turns.spawn(async move {
                        drive(&pool, &runtime, models.as_ref(), &config, id).await?;
                        anyhow::Ok(id)
                    });
                }
            }
            tokio::select! {
                _ = &mut shutdown => {
                    turns.shutdown().await;
                    std::sync::Arc::try_unwrap(self.runtime).map_err(|_| anyhow::anyhow!("runtime still owned during drain"))?.shutdown().await?;
                    return Ok(());
                }

                result=self.listener.recv()=>{result?;}
                Some(result)=turns.join_next(),if !turns.is_empty()=>{active.remove(&result??);}
                _=tokio::time::sleep(Duration::from_secs(1))=>{sqlx::query("SELECT 1").execute(&mut self.owner).await?;}
            }
        }
    }
}

const SESSION_COLUMNS: &str = "SELECT s.id,s.model,s.history,s.event_offset,COALESCE(c.workspace_id,'local') AS workspace_id FROM conversation_sessions s LEFT JOIN conversations c ON c.id=s.conversation_id";
const SESSION_COLUMNS_ACTIVE: &str = "SELECT s.id,s.model,s.history,s.event_offset,COALESCE(c.workspace_id,'local') AS workspace_id FROM conversation_sessions s LEFT JOIN conversations c ON c.id=s.conversation_id WHERE s.state='active'";
const SESSION_COLUMNS_CLOSED: &str = "SELECT s.id,s.model,s.history,s.event_offset,COALESCE(c.workspace_id,'local') AS workspace_id FROM conversation_sessions s LEFT JOIN conversations c ON c.id=s.conversation_id WHERE s.state='closed'";

async fn prepare(pool: &PgPool, id: i64) -> Result<(StoredSession, String, i64)> {
    let mut tx = pool.begin().await?;
    let parent: i64 = sqlx::query_scalar("SELECT conversation_id FROM turns WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("SELECT id FROM conversations WHERE id=$1 FOR UPDATE")
        .bind(parent)
        .execute(&mut *tx)
        .await?;
    let (conversation, prompt, model, session_id, attempt): (
        i64,
        String,
        Option<String>,
        Option<String>,
        i64,
    ) = sqlx::query_as(
        "SELECT conversation_id,prompt,model,session_id,attempt FROM turns WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("SELECT id FROM conversations WHERE id=$1 FOR UPDATE")
        .bind(conversation)
        .execute(&mut *tx)
        .await?;
    let model = model
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "connection-default".into());
    let workspace: String =
        sqlx::query_scalar("SELECT workspace_id FROM conversations WHERE id=$1")
            .bind(conversation)
            .fetch_one(&mut *tx)
            .await?;
    let mut session: Option<StoredSession> = if let Some(session_id) = session_id {
        sqlx::query_as(&format!("{SESSION_COLUMNS} WHERE s.id=$1"))
            .bind(session_id)
            .fetch_optional(&mut *tx)
            .await?
    } else {
        sqlx::query_as(&format!(
            "{SESSION_COLUMNS} WHERE s.conversation_id=$1 AND s.model=$2 AND s.state='active'"
        ))
        .bind(conversation)
        .bind(&model)
        .fetch_optional(&mut *tx)
        .await?
    };
    if session.is_none() {
        sqlx::query("UPDATE conversation_sessions SET state='closed' WHERE conversation_id=$1 AND state='active'").bind(conversation).execute(&mut *tx).await?;
        let prior:Vec<(String,String)>=sqlx::query_as("SELECT prompt,response FROM turns WHERE conversation_id=$1 AND id<$2 AND state='completed' ORDER BY id").bind(conversation).bind(id).fetch_all(&mut *tx).await?;
        let history = prior
            .into_iter()
            .flat_map(|(input, output)| {
                [
                    Message::user(input),
                    Message::assistant(vec![Content::Text { text: output }]),
                ]
            })
            .collect::<Vec<_>>();
        let new = StoredSession {
            id: uuid::Uuid::new_v4().to_string(),
            model,
            history: sqlx::types::Json(history),
            event_offset: 0,
            workspace_id: workspace,
        };
        sqlx::query("INSERT INTO conversation_sessions(id,conversation_id,model,history,state) VALUES ($1,$2,$3,$4,'active')").bind(&new.id).bind(conversation).bind(&new.model).bind(&new.history).execute(&mut *tx).await?;
        session = Some(new);
    }
    let session = session.expect("existing or newly inserted session");
    sqlx::query("UPDATE turns SET state='running',session_id=$2,started_at=COALESCE(started_at,extract(epoch FROM now())::bigint) WHERE id=$1 AND state IN ('queued','running')").bind(id).bind(&session.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok((session, prompt, attempt))
}

/// Reply text streams into `turns.draft` until its step is recorded. A late
/// flush after that step is a no-op because the text step count moved on.
struct Draft {
    sink: DeltaSink,
    flusher: tokio::task::JoinHandle<()>,
    generation: Arc<AtomicI64>,
}
fn draft(pool: PgPool, id: i64, attempt: i64) -> Draft {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let generation = Arc::new(AtomicI64::new(0));
    let seen = generation.clone();
    let flusher = tokio::spawn(async move {
        let mut buffer = String::new();
        let mut tick = tokio::time::interval(Duration::from_millis(120));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut open = true;
        while open {
            tokio::select! {
                delta = rx.recv() => match delta {
                    Some(delta) => buffer.push_str(&delta),
                    None => open = false,
                },
                _ = tick.tick() => {}
            }
            if buffer.is_empty() {
                continue;
            }
            let text = std::mem::take(&mut buffer);
            let _ = sqlx::query("UPDATE turns SET draft=COALESCE(draft,'')||$2 WHERE id=$1 AND state='running' AND attempt=$3 AND (SELECT count(*) FROM turn_steps WHERE turn_id=$1 AND attempt=$3 AND kind='text')=$4")
                .bind(id)
                .bind(&text)
                .bind(attempt)
                .bind(seen.load(Ordering::Relaxed))
                .execute(&pool)
                .await;
        }
    });
    Draft {
        sink: Arc::new(move |delta: &str| {
            let _ = tx.send(delta.to_owned());
        }),
        flusher,
        generation,
    }
}

async fn drive(
    pool: &PgPool,
    runtime: &Runtime,
    models: &dyn ModelCatalog,
    config: &RuntimeConfig,
    id: i64,
) -> Result<()> {
    let (stored, prompt, attempt) = prepare(pool, id).await?;
    let draft = draft(pool.clone(), id, attempt);
    let agent = stored.agent(pool, models, config, Some(draft.sink.clone()))?;
    let session = runtime
        .open_session(SessionId::new(&stored.id), &agent, stored.history.0.clone())
        .await?;
    session
        .send_once(format!("turn-{id}-{attempt}"), prompt)
        .await?;
    let mut offset = stored.event_offset;
    let mut events = session.events_from(usize::try_from(offset)?);
    let outcome = loop {
        let Some(event) = events.next().await else {
            break Err(anyhow::anyhow!(
                "Conversation event stream ended before its turn"
            ));
        };
        match event {
            Ok(event) => {
                let ended = event == Event::TurnEnded;
                let mut tx = pool.begin().await?;
                let advanced=sqlx::query("UPDATE conversation_sessions SET event_offset=event_offset+1 WHERE id=$1 AND event_offset=$2").bind(&stored.id).bind(offset).execute(&mut *tx).await?.rows_affected();
                if advanced != 0 {
                    if let Event::Message(message) = &event {
                        project_steps(&mut tx, id, attempt, offset, message, &draft.generation)
                            .await?;
                    }
                    if ended {
                        sqlx::query("UPDATE turns SET state='completed',response=COALESCE(response,''),error=NULL,draft=NULL,finished_at=extract(epoch FROM now())::bigint WHERE id=$1 AND state='running' AND attempt=$2").bind(id).bind(attempt).execute(&mut *tx).await?;
                        sqlx::query("SELECT pg_notify('agentinc_results',$1)")
                            .bind(id.to_string())
                            .execute(&mut *tx)
                            .await?;
                    }
                }
                tx.commit().await?;
                offset += 1;
                if ended {
                    break Ok(());
                }
            }
            Err(turnkeel::Error::RunFailed(error)) => {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE turns SET state='failed',error=$2,draft=NULL,finished_at=extract(epoch FROM now())::bigint WHERE id=$1 AND state='running' AND attempt=$3").bind(id).bind(error).bind(attempt).execute(&mut *tx).await?;
                sqlx::query("UPDATE conversation_sessions SET state='failed' WHERE id=$1")
                    .bind(&stored.id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("SELECT pg_notify('agentinc_results',$1)")
                    .bind(id.to_string())
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                break Ok(());
            }
            Err(error) => break Err(error.into()), // Infrastructure outages retain pending work.
        }
    };
    // The registered agent keeps a sink clone alive, so stop the flusher
    // explicitly; the turn is no longer running, so any pending flush is a no-op.
    draft.flusher.abort();
    outcome
}

/// Record one session event as durable turn steps and keep the joined reply.
async fn project_steps(
    tx: &mut sqlx::PgConnection,
    id: i64,
    attempt: i64,
    offset: i64,
    message: &Message,
    generation: &AtomicI64,
) -> Result<()> {
    let mut seq = offset * 16;
    for block in &message.content {
        let (kind, content) = match (message.role, block) {
            (Role::Assistant, Content::Text { text }) => {
                sqlx::query("UPDATE turns SET response=COALESCE(response,'')||$2,draft=NULL WHERE id=$1 AND state='running' AND attempt=$3").bind(id).bind(text).bind(attempt).execute(&mut *tx).await?;
                generation.fetch_add(1, Ordering::Relaxed);
                ("text", serde_json::json!({"text": text}))
            }
            (
                Role::Assistant,
                Content::ToolUse {
                    id: call,
                    name,
                    input,
                },
            ) => (
                "tool_use",
                serde_json::json!({"id": call, "name": name, "input": input}),
            ),
            (
                Role::User,
                Content::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                },
            ) => (
                "tool_result",
                serde_json::json!({"tool_use_id": tool_use_id, "content": content, "is_error": is_error}),
            ),
            _ => continue,
        };
        sqlx::query("INSERT INTO turn_steps(turn_id,attempt,seq,kind,content) VALUES ($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING")
            .bind(id)
            .bind(attempt)
            .bind(seq)
            .bind(kind)
            .bind(content)
            .execute(&mut *tx)
            .await?;
        seq += 1;
    }
    Ok(())
}
pub async fn save_result(pool: &PgPool, id: i64, result: Result<String>) -> Result<()> {
    let (response, error, state) = match result {
        Ok(text) => (Some(text), None, "completed"),
        Err(error) => (None, Some(error.to_string()), "failed"),
    };
    // Retain the completed result while Postgres is unavailable; saving again
    // must not call the provider a second time.
    loop {
        let result = sqlx::query(
            "WITH saved AS (UPDATE turns SET response=$2,error=$3,state=$4 WHERE id=$1 AND state='running' RETURNING id) SELECT pg_notify('agentinc_results',id::text) FROM saved",
        )
        .bind(id)
        .bind(&response)
        .bind(&error)
        .bind(state)
        .execute(pool)
        .await;
        match result {
            Ok(_) => return Ok(()),
            Err(error) if pool.is_closed() => return Err(error.into()),
            Err(error) => {
                tracing::warn!(turn_id=id, %error, "reply retained; retrying persistence");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation_tools::TicketsTool;
    use crate::product::{self, Command, CommandRequest};
    use serde_json::json;
    use turnkeel::{
        Tool, ToolCtx,
        testing::{ScriptedModel, Server, text, tool_call},
    };
    struct Models(Arc<dyn Model>);
    impl ModelCatalog for Models {
        fn resolve(&self, _: &str) -> Result<Arc<dyn Model>, ModelError> {
            Ok(self.0.clone())
        }
        fn resolve_streaming(
            &self,
            _: &str,
            sink: DeltaSink,
        ) -> Result<Arc<dyn Model>, ModelError> {
            Ok(Arc::new(Streaming {
                inner: self.0.clone(),
                sink,
            }))
        }
    }
    /// Reports every reply through the sink first, like a real streaming transport.
    struct Streaming {
        inner: Arc<dyn Model>,
        sink: DeltaSink,
    }
    impl Model for Streaming {
        fn id(&self) -> &str {
            self.inner.id()
        }
        fn complete(
            &self,
            request: ModelRequest,
        ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
            let inner = self.inner.clone();
            let sink = self.sink.clone();
            Box::pin(async move {
                let response = inner.complete(request).await?;
                for block in &response.content {
                    if let Content::Text { text } = block {
                        for word in text.split_inclusive(' ') {
                            sink(word);
                        }
                    }
                }
                Ok(response)
            })
        }
    }
    async fn apply(pool: &PgPool, command: Command) -> Option<i64> {
        product::execute(
            pool,
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
        let conversation = apply(&pool, Command::CreateConversation).await.unwrap();
        let first = apply(
            &pool,
            Command::Send {
                conversation_id: conversation,
                prompt: "Create".into(),
                command: None,
            },
        )
        .await
        .unwrap();
        drive(
            &pool,
            &runner.runtime,
            models.as_ref(),
            &runner.config,
            first,
        )
        .await
        .unwrap();
        let session: String = sqlx::query_scalar("SELECT session_id FROM turns WHERE id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
        let steps: Vec<(String, serde_json::Value)> =
            sqlx::query_as("SELECT kind,content FROM turn_steps WHERE turn_id=$1 ORDER BY seq")
                .bind(first)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(steps[0].0, "tool_use");
        assert_eq!(steps[0].1["name"], "ticket_command");
        assert_eq!(steps[1].0, "tool_result");
        assert_eq!(steps[1].1["is_error"], false);
        assert_eq!(steps[2].0, "text");
        assert_eq!(steps[2].1["text"], "Created your Ticket");
        assert_eq!(steps.len(), 3);
        let (response, draft, finished): (Option<String>, Option<String>, Option<i64>) =
            sqlx::query_as("SELECT response,draft,finished_at FROM turns WHERE id=$1")
                .bind(first)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(response.as_deref(), Some("Created your Ticket"));
        assert_eq!(draft, None);
        assert!(finished.is_some());
        let snapshot = product::snapshot(&pool).await.unwrap();
        assert_eq!(snapshot.turns[0].steps.len(), 3);
        assert_eq!(snapshot.turns[0].steps[0].kind, "tool_use");
        assert_eq!(snapshot.settings.http_policy.allow, vec!["*"]);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tickets")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        let tool = TicketsTool {
            pool: pool.clone(),
            session_id: session.clone(),
            mutation: true,
        };
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
                command: None,
            },
        )
        .await
        .unwrap();
        drive(
            &pool,
            &runner.runtime,
            models.as_ref(),
            &runner.config,
            second,
        )
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
        apply(&pool, Command::DeleteConversation { id: conversation }).await;
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM conversation_sessions WHERE conversation_id IS NULL AND state='closed'").fetch_one(&pool).await.unwrap(),2);
        drop(runner);
        server.shutdown().await.unwrap();
    }
}
