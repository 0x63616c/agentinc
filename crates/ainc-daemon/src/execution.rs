//! At-least-once dispatch and fenced, idempotent projection of SDK results.
use crate::{
    coding::{CodingTool, Permission, WorkspacePolicy},
    tickets::Actor,
};
use futures::future::BoxFuture;
use sqlx::{FromRow, PgPool, postgres::PgListener};
use std::{collections::HashSet, sync::Arc};
use turnkeel::{
    Agent, Model, ModelError, ModelRequest, ModelResponse, RunId, Runtime, RuntimeConfig,
};

/// Receives reply text as it streams from a provider.
pub type DeltaSink = Arc<dyn Fn(&str) + Send + Sync>;

/// Resolve an immutable model ID when reconstructing a persisted agent definition.
/// Tests supply scripted models; production supplies the Connection's model adapter.
pub trait ModelCatalog: Send + Sync + 'static {
    fn resolve(&self, id: &str) -> Result<Arc<dyn Model>, ModelError>;
    /// Resolve a model that reports reply text as it streams. Catalogs without
    /// streaming transports return the plain model.
    fn resolve_streaming(&self, id: &str, sink: DeltaSink) -> Result<Arc<dyn Model>, ModelError> {
        let _ = sink;
        self.resolve(id)
    }
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
#[derive(Clone, FromRow)]
struct Definition {
    run_id: String,
    ticket_id: i64,
    generation: i64,
    agent_id: String,
    model: String,
    instructions: String,
    prompt: String,
    workspace_id: String,
}
impl Definition {
    fn agent(
        &self,
        pool: &PgPool,
        models: &dyn ModelCatalog,
        policy: &Arc<WorkspacePolicy>,
    ) -> anyhow::Result<Agent> {
        let actor = Actor {
            workspace: self.workspace_id.clone(),
            id: self.agent_id.clone(),
            assignment: Some((self.ticket_id, self.generation)),
        };
        let mut builder=Agent::builder(format!("ticket-{}-v1",self.run_id))
            .model(SharedModel(models.resolve(&self.model)?))
            .instructions(format!("{}\nWork only on this assigned Ticket. Tool effects are recorded as Comments. Inspect unknown outcomes before taking more action. Finish with a concise account of work and evidence.",self.instructions));
        for permission in [
            Permission::ReadFile,
            Permission::WriteFile,
            Permission::Shell,
            Permission::Git,
        ] {
            if policy.allows(permission) {
                builder = builder.tool(CodingTool {
                    pool: pool.clone(),
                    actor: actor.clone(),
                    run_id: self.run_id.clone(),
                    policy: policy.clone(),
                    permission,
                });
            }
        }
        Ok(builder
            .tool(crate::coding::CommentTool {
                pool: pool.clone(),
                actor,
            })
            .build())
    }
}

pub struct Runner {
    pool: PgPool,
    runtime: Arc<Runtime>,
    models: Arc<dyn ModelCatalog>,
    policy: Arc<WorkspacePolicy>,
    listener: PgListener,
    owner: sqlx::PgConnection,
}
impl Runner {
    pub async fn start(
        pool: PgPool,
        config: RuntimeConfig,
        models: Arc<dyn ModelCatalog>,
        policy: WorkspacePolicy,
    ) -> anyhow::Result<Self> {
        let mut owner = pool.acquire().await?.detach();
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(7358710303)")
            .fetch_one(&mut owner)
            .await?;
        anyhow::ensure!(locked, "another daemon owns Ticket dispatch");
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen("agentinc_dispatch").await?;
        let policy = Arc::new(policy);
        let definitions:Vec<Definition>=sqlx::query_as("SELECT r.run_id,r.ticket_id,r.generation,r.agent_id,r.model,r.instructions,r.prompt,t.workspace_id FROM ticket_runs r JOIN tickets t ON t.id=r.ticket_id WHERE r.state IN ('queued','running')").fetch_all(&pool).await?;
        let agents = definitions
            .iter()
            .map(|d| d.agent(&pool, models.as_ref(), &policy))
            .collect::<Result<Vec<_>, _>>()?;
        let runtime = Arc::new(Runtime::configured(config, &agents).await?);
        Ok(Self {
            pool,
            runtime,
            models,
            policy,
            listener,
            owner,
        })
    }
    pub async fn run(self) -> anyhow::Result<()> {
        self.run_until(std::future::pending()).await
    }
    pub async fn run_until(
        mut self,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> anyhow::Result<()> {
        tokio::pin!(shutdown);
        let mut active = HashSet::new();
        let mut results = tokio::task::JoinSet::new();
        loop {
            self.dispatch().await?;
            let definitions:Vec<Definition>=sqlx::query_as("SELECT r.run_id,r.ticket_id,r.generation,r.agent_id,r.model,r.instructions,r.prompt,t.workspace_id FROM ticket_runs r JOIN tickets t ON t.id=r.ticket_id WHERE r.state='running'").fetch_all(&self.pool).await?;
            for definition in definitions {
                if active.insert(definition.run_id.clone()) {
                    let runtime = self.runtime.clone();
                    let pool = self.pool.clone();
                    results.spawn(async move {
                        let run = runtime.run_by_id(RunId::new(&definition.run_id));
                        let result = match run.result().await {
                            Ok(text) => Ok(text),
                            Err(
                                error
                                @ (turnkeel::Error::RunFailed(_) | turnkeel::Error::Cancelled),
                            ) => Err(error.to_string()),
                            Err(error) => return Err(error.into()),
                        };
                        project(&pool, &definition, result).await?;
                        anyhow::Ok(definition.run_id)
                    });
                }
            }
            tokio::select! {
                _ = &mut shutdown => {
                    results.shutdown().await;
                    std::sync::Arc::try_unwrap(self.runtime).map_err(|_| anyhow::anyhow!("runtime still owned during drain"))?.shutdown().await?;
                    return Ok(());
                }

                notification=self.listener.recv()=> {notification?;}
                Some(result)=results.join_next(),if !results.is_empty()=> {active.remove(&result??);}
                _=tokio::time::sleep(std::time::Duration::from_secs(1))=> {
                    // Also reconciles a lost NOTIFY and notices loss of exclusive ownership.
                    sqlx::query("SELECT 1").execute(&mut self.owner).await?;
                }
            }
        }
    }
    async fn dispatch(&self) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO worker_health(id,last_seen) VALUES('tickets',extract(epoch FROM clock_timestamp())::bigint) ON CONFLICT(id) DO UPDATE SET last_seen=excluded.last_seen")
            .execute(&self.pool).await?;
        let rows: Vec<(i64, String, String)> = sqlx::query_as(
            "SELECT id,action,run_id FROM dispatch_outbox WHERE NOT dispatched ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        for (id, action, run_id) in rows {
            if action == "cancel" {
                // A queued start can have been cancelled before any SDK run existed.
                let started:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM dispatch_outbox WHERE run_id=$1 AND action='start' AND dispatched)").bind(&run_id).fetch_one(&self.pool).await?;
                if started {
                    match self.runtime.run_by_id(RunId::new(&run_id)).cancel().await {
                        Ok(()) | Err(turnkeel::Error::NotFound) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
            } else {
                let definition:Option<Definition>=sqlx::query_as("SELECT r.run_id,r.ticket_id,r.generation,r.agent_id,r.model,r.instructions,r.prompt,t.workspace_id FROM ticket_runs r JOIN tickets t ON t.id=r.ticket_id AND t.generation=r.generation WHERE r.run_id=$1 AND r.state IN ('queued','running')").bind(&run_id).fetch_optional(&self.pool).await?;
                if let Some(definition) = definition {
                    let agent = definition.agent(&self.pool, self.models.as_ref(), &self.policy)?;
                    // Mark running before dispatch so tools can pass their authorization
                    // fence. A crash here leaves the outbox pending for the next process.
                    let mut tx = self.pool.begin().await?;
                    sqlx::query("UPDATE tickets SET status='in_progress',revision=revision+1 WHERE id=$1 AND generation=$2 AND status='to_do'").bind(definition.ticket_id).bind(definition.generation).execute(&mut *tx).await?;
                    sqlx::query(
                        "UPDATE ticket_runs SET state='running' WHERE run_id=$1 AND state='queued'",
                    )
                    .bind(&run_id)
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    self.runtime
                        .start_with_id(RunId::new(&run_id), &agent, definition.prompt)
                        .await?;
                }
            }
            // If the process dies after dispatch, the same stable ID is submitted again.
            sqlx::query("UPDATE dispatch_outbox SET dispatched=true WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

async fn project(
    pool: &PgPool,
    definition: &Definition,
    result: Result<String, String>,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tickets t JOIN ticket_runs r ON r.ticket_id=t.id AND r.generation=t.generation WHERE r.run_id=$1 AND r.state='running')").bind(&definition.run_id).fetch_one(&mut *tx).await?;
    if !current {
        return Ok(());
    }
    // The row lock serializes completion with cancellation/reassignment.
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM tickets WHERE id=$1 FOR UPDATE")
            .bind(definition.ticket_id)
            .fetch_one(&mut *tx)
            .await?;
    if generation != definition.generation {
        return Ok(());
    }
    let (state, body, error) = match result {
        Ok(text) => (
            "completed",
            if text.trim().is_empty() {
                "Work completed without a summary. Inspect the tool Comments for evidence.".into()
            } else {
                text
            },
            None,
        ),
        Err(error) => ("failed", format!("Work stopped: {error}"), Some(error)),
    };
    let changed =
        sqlx::query("UPDATE ticket_runs SET state=$2,error=$3 WHERE run_id=$1 AND state='running'")
            .bind(&definition.run_id)
            .bind(state)
            .bind(error)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    if changed == 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO comments(ticket_id,author_id,body,effect_key) VALUES ($1,$2,$3,$4) ON CONFLICT(effect_key) DO NOTHING").bind(definition.ticket_id).bind(&definition.agent_id).bind(body.chars().take(32000).collect::<String>()).bind(format!("{}/result",definition.run_id)).execute(&mut *tx).await?;
    if state == "completed" {
        sqlx::query(
            "UPDATE tickets SET status='done',revision=revision+1 WHERE id=$1 AND generation=$2",
        )
        .bind(definition.ticket_id)
        .bind(definition.generation)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("SELECT pg_notify('agentinc_results',$1)")
        .bind(&definition.run_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tickets::{
        self, AssigneeKind, TicketCommand as Command, TicketCommandRequest, TicketStatus,
    };
    use turnkeel::testing::{Script, ScriptedModel, Server, text, tool_call};
    struct Models(Arc<dyn Model>);
    impl ModelCatalog for Models {
        fn resolve(&self, _: &str) -> Result<Arc<dyn Model>, ModelError> {
            Ok(self.0.clone())
        }
    }
    async fn apply(pool: &PgPool, command: Command) -> Option<i64> {
        tickets::execute(
            pool,
            &Actor::owner(),
            TicketCommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command,
            },
        )
        .await
        .unwrap()
        .result_id
    }
    async fn assign(pool: &PgPool) -> Definition {
        apply(
            pool,
            Command::RegisterAgent {
                name: "Fixture".into(),
                model: "fixture".into(),
                instructions: "Make evidence".into(),
            },
        )
        .await;
        let agent: String = sqlx::query_scalar("SELECT id FROM agents")
            .fetch_one(pool)
            .await
            .unwrap();
        let id = apply(
            pool,
            Command::Create {
                title: "Produce evidence".into(),
            },
        )
        .await
        .unwrap();
        apply(
            pool,
            Command::Assign {
                id,
                revision: 0,
                assignee_kind: AssigneeKind::Agent,
                assignee_id: agent,
            },
        )
        .await;
        sqlx::query_as("SELECT r.run_id,r.ticket_id,r.generation,r.agent_id,r.model,r.instructions,r.prompt,t.workspace_id FROM ticket_runs r JOIN tickets t ON t.id=r.ticket_id").fetch_one(pool).await.unwrap()
    }

    #[sqlx::test]
    async fn assigned_ticket_dispatches_once_and_projects_inspectable_comments(pool: PgPool) {
        let definition = assign(&pool).await;
        let server = Server::start().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let model = ScriptedModel::new()
            .on_user(
                "Produce evidence",
                tool_call(
                    "comment",
                    serde_json::json!({"body":"Fixture evidence from our SDK tool"}),
                ),
            )
            .on_tool_result("comment", text("Completed with fixture evidence"));
        let runner = Runner::start(
            pool.clone(),
            server.config(),
            Arc::new(Models(Arc::new(model))),
            WorkspacePolicy::new(dir.path(), vec![]).unwrap(),
        )
        .await
        .unwrap();
        let mut listener = PgListener::connect_with(&pool).await.unwrap();
        listener.listen("agentinc_results").await.unwrap();
        let worker = tokio::spawn(runner.run());
        assert_eq!(listener.recv().await.unwrap().payload(), definition.run_id);
        let state = tickets::snapshot(&pool, &Actor::owner()).await.unwrap();
        assert_eq!(state.tickets[0].status, TicketStatus::Done);
        assert_eq!(state.comments.len(), 2);
        assert!(state.comments[0].body.contains("SDK tool"));
        assert!(state.comments[1].body.contains("Completed"));
        // Repeat dispatch after a lost acknowledgement and projection after a lost reply.
        sqlx::query("UPDATE dispatch_outbox SET dispatched=false")
            .execute(&pool)
            .await
            .unwrap();
        project(&pool, &definition, Ok("Should not appear twice".into()))
            .await
            .unwrap();
        assert_eq!(
            tickets::snapshot(&pool, &Actor::owner())
                .await
                .unwrap()
                .comments
                .len(),
            2
        );
        worker.abort();
        let _ = worker.await;
        server.replay(&RunId::new(definition.run_id)).await.unwrap();
        server.shutdown().await.unwrap();
    }

    #[sqlx::test]
    async fn cancellation_before_dispatch_and_during_a_turn_fences_old_results(pool: PgPool) {
        let definition = assign(&pool).await;
        let server = Server::start().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = Script::new();
        let runner = Runner::start(
            pool.clone(),
            server.config(),
            Arc::new(Models(Arc::new(script.model()))),
            WorkspacePolicy::new(dir.path(), vec![]).unwrap(),
        )
        .await
        .unwrap();
        apply(
            &pool,
            Command::Cancel {
                id: definition.ticket_id,
                revision: 1,
            },
        )
        .await;
        runner.dispatch().await.unwrap(); // No SDK run existed: cancellation is still safe.
        apply(
            &pool,
            Command::Assign {
                id: definition.ticket_id,
                revision: 2,
                assignee_kind: AssigneeKind::Agent,
                assignee_id: definition.agent_id.clone(),
            },
        )
        .await;
        runner.dispatch().await.unwrap();
        let mut call = script.next_model_call().await;
        let current = tickets::snapshot(&pool, &Actor::owner())
            .await
            .unwrap()
            .tickets
            .remove(0);
        apply(
            &pool,
            Command::Cancel {
                id: current.id,
                revision: current.revision,
            },
        )
        .await;
        runner.dispatch().await.unwrap();
        call.cancelled().await;
        project(
            &pool,
            &definition,
            Ok("Stale completion must not change the Ticket".into()),
        )
        .await
        .unwrap();
        let state = tickets::snapshot(&pool, &Actor::owner()).await.unwrap();
        assert_eq!(state.tickets[0].status, TicketStatus::ToDo);
        assert!(state.comments.is_empty());
        drop(runner);
        server.shutdown().await.unwrap();
    }

    #[cfg(target_os = "macos")]
    #[sqlx::test]
    async fn assigned_ticket_writes_workspace_file_and_records_tool_evidence(pool: PgPool) {
        let definition = assign(&pool).await;
        let server = Server::start().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let model = ScriptedModel::new()
            .on_user(
                "Produce evidence",
                tool_call(
                    "write_file",
                    serde_json::json!({"path":"evidence.txt","content":"inspectable fixture"}),
                ),
            )
            .on_tool_result("write_file", text("Created evidence.txt"));
        let runner = Runner::start(
            pool.clone(),
            server.config(),
            Arc::new(Models(Arc::new(model))),
            WorkspacePolicy::new(dir.path(), vec![Permission::WriteFile]).unwrap(),
        )
        .await
        .unwrap();
        let mut listener = PgListener::connect_with(&pool).await.unwrap();
        listener.listen("agentinc_results").await.unwrap();
        let worker = tokio::spawn(runner.run());
        listener.recv().await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("evidence.txt")).unwrap(),
            "inspectable fixture"
        );
        let state = tickets::snapshot(&pool, &Actor::owner()).await.unwrap();
        assert_eq!(state.tickets[0].status, TicketStatus::Done);
        assert_eq!(state.comments.len(), 3);
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM tool_effects WHERE run_id=$1 AND result IS NOT NULL",
        )
        .bind(&definition.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        worker.abort();
        let _ = worker.await;
        server.shutdown().await.unwrap();
    }
    #[cfg(target_os = "macos")]
    #[sqlx::test]
    async fn effect_receipts_reconcile_duplicates_and_refuse_unknown_outcomes(pool: PgPool) {
        let definition = assign(&pool).await;
        let server = Server::start().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let args = serde_json::json!({"command":"printf x >> count.txt"});
        let model = ScriptedModel::new()
            .on_user("Produce evidence", tool_call("shell", args.clone()))
            .on_tool_result("shell", text("One effect"));
        let models = Arc::new(Models(Arc::new(model)));
        let runner = Runner::start(
            pool.clone(),
            server.config(),
            models.clone(),
            WorkspacePolicy::new(dir.path(), vec![Permission::Shell]).unwrap(),
        )
        .await
        .unwrap();
        runner.dispatch().await.unwrap();
        assert_eq!(
            runner
                .runtime
                .run_by_id(RunId::new(&definition.run_id))
                .result()
                .await
                .unwrap(),
            "One effect"
        );
        sqlx::query("UPDATE dispatch_outbox SET dispatched=false")
            .execute(&pool)
            .await
            .unwrap();
        runner.dispatch().await.unwrap();
        let key: String = sqlx::query_scalar("SELECT effect_key FROM tool_effects")
            .fetch_one(&pool)
            .await
            .unwrap();
        let agent = definition
            .agent(&pool, models.as_ref(), &runner.policy)
            .unwrap();
        let tool = agent.tools().get("shell").unwrap();
        tool.call(turnkeel::ToolCtx::new(&key), args.clone())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("count.txt")).unwrap(),
            "x"
        );
        sqlx::query("UPDATE tool_effects SET result=NULL")
            .execute(&pool)
            .await
            .unwrap();
        let error = tool
            .call(turnkeel::ToolCtx::new(&key), args.clone())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("unknown"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("count.txt")).unwrap(),
            "x"
        );
        let ticket = tickets::snapshot(&pool, &Actor::owner())
            .await
            .unwrap()
            .tickets
            .remove(0);
        apply(
            &pool,
            Command::Assign {
                id: ticket.id,
                revision: ticket.revision,
                assignee_kind: AssigneeKind::Human,
                assignee_id: "owner".into(),
            },
        )
        .await;
        assert!(
            tool.call(turnkeel::ToolCtx::new("new-key"), args)
                .await
                .unwrap_err()
                .to_string()
                .contains("no longer active")
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("count.txt")).unwrap(),
            "x"
        );
        drop(runner);
        server.shutdown().await.unwrap();
    }
}
