//! Durable SDK sessions with a Postgres message outbox and event projection.
use crate::execution::ModelCatalog;
use anyhow::Result;
use futures::{StreamExt, future::BoxFuture};
use sqlx::{FromRow, PgPool, postgres::PgListener};
use std::{collections::HashSet, sync::Arc, time::Duration};
use turnkeel::{
    Agent, Content, Event, Message, Model, ModelError, ModelRequest, ModelResponse, Role, Runtime,
    RuntimeConfig, SessionId,
};

#[derive(Clone, FromRow)]
struct StoredSession {
    id: String,
    model: String,
    history: sqlx::types::Json<Vec<Message>>,
    event_offset: i64,
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
    fn agent(&self, pool: &PgPool, models: &dyn ModelCatalog) -> Result<Agent> {
        Ok(Agent::builder(format!("conversation-{}-v1",self.id))
            .model(SharedModel(models.resolve(&self.model)?))
            .instructions("You are Evee, the personal assistant in AgentInc. Use Ticket tools for requested product changes. Autonomous work belongs on an assigned Ticket; do not claim to perform external actions without tools.")
            .tool(crate::conversation_tools::TicketsTool { pool:pool.clone(), session_id:self.id.clone(), mutation:false })
            .tool(crate::conversation_tools::TicketsTool { pool:pool.clone(), session_id:self.id.clone(), mutation:true })
            .build())
    }
}
pub struct Runner {
    pool: PgPool,
    owner: sqlx::PgConnection,
    listener: PgListener,
    runtime: Arc<Runtime>,
    models: Arc<dyn ModelCatalog>,
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
        let sessions:Vec<StoredSession>=sqlx::query_as("SELECT id,conversation_id,model,history,event_offset FROM conversation_sessions WHERE state='active'").fetch_all(&pool).await?;
        let agents = sessions
            .iter()
            .map(|s| s.agent(&pool, models.as_ref()))
            .collect::<Result<Vec<_>>>()?;
        config.worker_group.push_str("-conversations");
        let runtime = Arc::new(Runtime::configured(config, &agents).await?);
        Ok(Self {
            pool,
            owner,
            listener,
            runtime,
            models,
        })
    }
    pub async fn run(mut self) -> Result<()> {
        let mut active = HashSet::new();
        let mut turns = tokio::task::JoinSet::new();
        loop {
            let closed:Vec<StoredSession>=sqlx::query_as("SELECT id,model,history,event_offset FROM conversation_sessions WHERE state='closed'").fetch_all(&self.pool).await?;
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
                    turns.spawn(async move {
                        drive(&pool, &runtime, models.as_ref(), id).await?;
                        anyhow::Ok(id)
                    });
                }
            }
            tokio::select! {
                result=self.listener.recv()=>{result?;}
                Some(result)=turns.join_next(),if !turns.is_empty()=>{active.remove(&result??);}
                _=tokio::time::sleep(Duration::from_secs(1))=>{sqlx::query("SELECT 1").execute(&mut self.owner).await?;}
            }
        }
    }
}

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
    let mut session: Option<StoredSession> = if let Some(session_id) = session_id {
        sqlx::query_as("SELECT id,conversation_id,model,history,event_offset FROM conversation_sessions WHERE id=$1").bind(session_id).fetch_optional(&mut *tx).await?
    } else {
        sqlx::query_as("SELECT id,conversation_id,model,history,event_offset FROM conversation_sessions WHERE conversation_id=$1 AND model=$2 AND state='active'").bind(conversation).bind(&model).fetch_optional(&mut *tx).await?
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
        };
        sqlx::query("INSERT INTO conversation_sessions(id,conversation_id,model,history,state) VALUES ($1,$2,$3,$4,'active')").bind(&new.id).bind(conversation).bind(&new.model).bind(&new.history).execute(&mut *tx).await?;
        session = Some(new);
    }
    let session = session.expect("existing or newly inserted session");
    sqlx::query("UPDATE turns SET state='running',session_id=$2 WHERE id=$1 AND state IN ('queued','running')").bind(id).bind(&session.id).execute(&mut *tx).await?;
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
                let advanced=sqlx::query("UPDATE conversation_sessions SET event_offset=event_offset+1 WHERE id=$1 AND event_offset=$2").bind(&stored.id).bind(offset).execute(&mut *tx).await?.rows_affected();
                if advanced != 0 {
                    if let Event::Message(message) = event
                        && message.role == Role::Assistant
                    {
                        sqlx::query("UPDATE turns SET response=COALESCE(response,'')||$2 WHERE id=$1 AND state='running' AND attempt=$3").bind(id).bind(message.text()).bind(attempt).execute(&mut *tx).await?;
                    }
                    if ended {
                        sqlx::query("UPDATE turns SET state='completed',response=COALESCE(response,''),error=NULL WHERE id=$1 AND state='running' AND attempt=$2").bind(id).bind(attempt).execute(&mut *tx).await?;
                        sqlx::query("SELECT pg_notify('agentinc_results',$1)")
                            .bind(id.to_string())
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
                sqlx::query("UPDATE turns SET state='failed',error=$2 WHERE id=$1 AND state='running' AND attempt=$3").bind(id).bind(error).bind(attempt).execute(&mut *tx).await?;
                sqlx::query("UPDATE conversation_sessions SET state='failed' WHERE id=$1")
                    .bind(&stored.id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("SELECT pg_notify('agentinc_results',$1)")
                    .bind(id.to_string())
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
            },
        )
        .await
        .unwrap();
        drive(&pool, &runner.runtime, models.as_ref(), first)
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
            },
        )
        .await
        .unwrap();
        drive(&pool, &runner.runtime, models.as_ref(), second)
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
