//! Saved Ticket proposals, recurring authorization, and idempotent occurrence commits.
use crate::{
    product::{ApiError, ErrorBody, Product},
    tickets::{self, Actor, TicketCommand, TicketCommandRequest, TicketProposal},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, postgres::PgListener};
use std::{sync::Arc, time::Duration};
use turnkeel::{Occurrence, RecurringAction, RecurringRule, Runtime, RuntimeConfig};
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub agent_id: String,
    pub every_minutes: i64,
    pub paused: bool,
    pub revision: i64,
    pub applied_revision: i64,
    pub error: Option<String>,
    pub missed: i64,
    pub overlap_skipped: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct OccurrenceView {
    pub id: String,
    pub automation_id: String,
    pub ticket_id: Option<i64>,
    pub state: String,
    pub detail: Option<String>,
    pub scheduled_at: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct HistoryEntry {
    pub id: i64,
    pub automation_id: String,
    pub kind: String,
    pub count: i64,
    pub observed_at: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AutomationSnapshot {
    pub rules: Vec<Automation>,
    pub occurrences: Vec<OccurrenceView>,
    pub history: Vec<HistoryEntry>,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutomationCommand {
    Save {
        id: Option<String>,
        revision: Option<i64>,
        name: String,
        proposal: TicketProposal,
        every_minutes: i64,
    },
    Pause {
        id: String,
        revision: i64,
        paused: bool,
    },
    RunNow {
        id: String,
        revision: i64,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AutomationRequest {
    pub operation_id: String,
    pub command: AutomationCommand,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AutomationReceipt {
    pub result_id: String,
}
fn invalid(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid", message)
}
pub fn router(product: Product) -> Router {
    Router::new()
        .route("/v1/automations", get(state))
        .route("/v1/automations/commands", post(command))
        .with_state(product)
}
#[utoipa::path(get,path="/v1/automations",operation_id="automations_state",responses((status=200,body=AutomationSnapshot),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn state(
    State(product): State<Product>,
    headers: HeaderMap,
) -> Result<Json<AutomationSnapshot>, ApiError> {
    product.authorize(&headers)?;
    let actor = Actor::owner_in(crate::workspaces::current(&product.pool).await?);
    Ok(Json(snapshot(&product.pool, &actor).await?))
}
#[utoipa::path(post,path="/v1/automations/commands",operation_id="automations_command",request_body=AutomationRequest,responses((status=200,body=AutomationReceipt),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=409,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn command(
    State(product): State<Product>,
    headers: HeaderMap,
    Json(request): Json<AutomationRequest>,
) -> Result<Json<AutomationReceipt>, ApiError> {
    product.authorize(&headers)?;
    let actor = Actor::owner_in(crate::workspaces::current(&product.pool).await?);
    Ok(Json(execute(&product.pool, &actor, request).await?))
}
pub(crate) async fn snapshot(pool: &PgPool, actor: &Actor) -> Result<AutomationSnapshot, ApiError> {
    let rules = sqlx::query_as("SELECT id,name,prompt,agent_id,every_minutes,paused,revision,applied_revision,error,missed,overlap_skipped FROM automations WHERE workspace_id=$1 ORDER BY name,id").bind(&actor.workspace).fetch_all(pool).await?;
    let occurrences = sqlx::query_as("SELECT o.id,o.automation_id,o.ticket_id,CASE WHEN r.state IN ('queued','running') AND NOT EXISTS(SELECT 1 FROM worker_health WHERE id='tickets' AND last_seen > extract(epoch FROM clock_timestamp())::bigint-5) THEN 'worker_unavailable' ELSE COALESCE(r.state,o.state) END AS state,o.detail,o.scheduled_at FROM occurrences o JOIN automations a ON a.id=o.automation_id LEFT JOIN ticket_runs r ON r.ticket_id=o.ticket_id AND r.generation=1 WHERE a.workspace_id=$1 ORDER BY o.scheduled_at DESC,o.id").bind(&actor.workspace).fetch_all(pool).await?;
    let history = sqlx::query_as("SELECT h.id,h.automation_id,h.kind,h.count,h.observed_at FROM automation_history h JOIN automations a ON a.id=h.automation_id WHERE a.workspace_id=$1 ORDER BY h.id DESC").bind(&actor.workspace).fetch_all(pool).await?;
    Ok(AutomationSnapshot {
        rules,
        occurrences,
        history,
    })
}
pub(crate) async fn execute(
    pool: &PgPool,
    actor: &Actor,
    request: AutomationRequest,
) -> Result<AutomationReceipt, ApiError> {
    if actor.assignment.is_some() {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Assigned agents cannot grant recurring work.",
        ));
    }
    if uuid::Uuid::parse_str(&request.operation_id).is_err() {
        return Err(invalid("Use a UUID operation ID."));
    }
    let payload = json!(request.command);
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "automation/{}/{}",
            actor.workspace, request.operation_id
        ))
        .execute(&mut *tx)
        .await?;
    let prior: Option<(Value,String)> = sqlx::query_as("SELECT command,result_id FROM automation_receipts WHERE workspace_id=$1 AND operation_id=$2").bind(&actor.workspace).bind(&request.operation_id).fetch_optional(&mut *tx).await?;
    if let Some((old, result_id)) = prior {
        if old != payload {
            return Err(ApiError::conflict());
        }
        return Ok(AutomationReceipt { result_id });
    }
    let result_id = match request.command {
        AutomationCommand::Save {
            id,
            revision,
            name,
            proposal,
            every_minutes,
        } => {
            if !(1..=525600).contains(&every_minutes)
                || name.trim().is_empty()
                || name.chars().count() > 120
                || proposal.title.trim().is_empty()
                || proposal.title.chars().count() > 500
            {
                return Err(invalid(
                    "Use a name (1–120 characters), prompt (1–500 characters), and interval of 1–525600 minutes.",
                ));
            }
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM agents WHERE workspace_id=$1 AND id=$2)",
            )
            .bind(&actor.workspace)
            .bind(&proposal.agent_id)
            .fetch_one(&mut *tx)
            .await?;
            if !valid {
                return Err(invalid("Choose a registered agent."));
            }
            if let Some(id) = id {
                let changed=sqlx::query("UPDATE automations SET name=$4,prompt=$5,agent_id=$6,every_minutes=$7,revision=revision+1,error=NULL WHERE id=$1 AND workspace_id=$2 AND revision=$3").bind(&id).bind(&actor.workspace).bind(revision).bind(name.trim()).bind(proposal.title.trim()).bind(proposal.agent_id).bind(every_minutes).execute(&mut *tx).await?.rows_affected();
                if changed == 0 {
                    return Err(ApiError::conflict());
                }
                id
            } else {
                if revision.is_some() {
                    return Err(invalid("New rules have no revision."));
                }
                let id = uuid::Uuid::new_v4().to_string();
                sqlx::query("INSERT INTO automations(id,workspace_id,name,prompt,agent_id,every_minutes) VALUES($1,$2,$3,$4,$5,$6)").bind(&id).bind(&actor.workspace).bind(name.trim()).bind(proposal.title.trim()).bind(proposal.agent_id).bind(every_minutes).execute(&mut *tx).await?;
                id
            }
        }
        AutomationCommand::Pause {
            id,
            revision,
            paused,
        } => {
            let changed=sqlx::query("UPDATE automations SET paused=$4,revision=revision+1,error=NULL WHERE id=$1 AND workspace_id=$2 AND revision=$3").bind(&id).bind(&actor.workspace).bind(revision).bind(paused).execute(&mut *tx).await?.rows_affected();
            if changed == 0 {
                return Err(ApiError::conflict());
            }
            id
        }
        AutomationCommand::RunNow { id, revision } => {
            let current: Option<i64> = sqlx::query_scalar(
                "SELECT revision FROM automations WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
            )
            .bind(&id)
            .bind(&actor.workspace)
            .fetch_optional(&mut *tx)
            .await?;
            if current != Some(revision) {
                return Err(ApiError::conflict());
            }
            let occurrence = format!("manual-{}", request.operation_id);
            sqlx::query(
                "INSERT INTO occurrences(id,automation_id,revision,manual) VALUES($1,$2,$3,true)",
            )
            .bind(&occurrence)
            .bind(id)
            .bind(revision)
            .execute(&mut *tx)
            .await?;
            occurrence
        }
    };
    sqlx::query("INSERT INTO automation_receipts(workspace_id,operation_id,command,result_id) VALUES($1,$2,$3,$4)").bind(&actor.workspace).bind(request.operation_id).bind(payload).bind(&result_id).execute(&mut *tx).await?;
    sqlx::query("SELECT pg_notify('agentinc_automations','')")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(AutomationReceipt { result_id })
}

#[derive(Clone)]
struct Action(PgPool);
impl RecurringAction for Action {
    fn execute(&self, occurrence: Occurrence) -> BoxFuture<'static, Result<(), turnkeel::Error>> {
        let pool = self.0.clone();
        Box::pin(async move {
            apply_occurrence(&pool, occurrence)
                .await
                .map_err(turnkeel::Error::Other)
        })
    }
}
async fn apply_occurrence(pool: &PgPool, occurrence: Occurrence) -> anyhow::Result<()> {
    let id = occurrence.input["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing rule ID"))?;
    let revision = occurrence.input["revision"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("missing rule revision"))?;
    let manual = occurrence.input["manual"].as_bool().unwrap_or(false);
    let mut tx = pool.begin().await?;
    let rule:(String,String,String,i64,bool)=sqlx::query_as("SELECT workspace_id,prompt,agent_id,revision,paused FROM automations WHERE id=$1 FOR UPDATE").bind(id).fetch_one(&mut *tx).await?;
    sqlx::query("INSERT INTO occurrences(id,automation_id,revision,manual,dispatched) VALUES($1,$2,$3,$4,true) ON CONFLICT(id) DO NOTHING").bind(&occurrence.id).bind(id).bind(revision).bind(manual).execute(&mut *tx).await?;
    let (mut ticket, state): (Option<i64>, String) =
        sqlx::query_as("SELECT ticket_id,state FROM occurrences WHERE id=$1")
            .bind(&occurrence.id)
            .fetch_one(&mut *tx)
            .await?;
    if ticket.is_none() && state == "waiting_for_worker" {
        let overlap:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM occurrences o JOIN ticket_runs r ON r.ticket_id=o.ticket_id WHERE o.automation_id=$1 AND r.state IN ('queued','running'))").bind(id).fetch_one(&mut *tx).await?;
        let reason = if rule.3 != revision {
            Some("superseded")
        } else if rule.4 && !manual {
            Some("paused")
        } else if overlap {
            Some("overlap_skipped")
        } else {
            None
        };
        if let Some(reason) = reason {
            sqlx::query("UPDATE occurrences SET state=$2 WHERE id=$1")
                .bind(&occurrence.id)
                .bind(reason)
                .execute(&mut *tx)
                .await?;
        } else {
            let digest = Sha256::digest(occurrence.id.as_bytes());
            let operation_id =
                uuid::Uuid::from_bytes(digest[..16].try_into().expect("digest length")).to_string();
            let actor = Actor::owner_in(rule.0);
            ticket = tickets::execute_in(
                &mut tx,
                &actor,
                TicketCommandRequest {
                    operation_id,
                    command: TicketCommand::CreateAssigned {
                        proposal: TicketProposal {
                            title: rule.1,
                            agent_id: rule.2,
                        },
                    },
                },
            )
            .await
            .map_err(|e| anyhow::anyhow!("Ticket proposal refused: {e:?}"))?
            .result_id;
            sqlx::query("UPDATE occurrences SET ticket_id=$2,state='queued' WHERE id=$1")
                .bind(&occurrence.id)
                .bind(ticket)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    // Subscribe before checking state: a completion between the two cannot be lost.
    if let Some(ticket) = ticket {
        let mut listener = PgListener::connect_with(pool).await?;
        listener.listen("agentinc_results").await?;
        listener.listen("agentinc_dispatch").await?;
        loop {
            let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ticket_runs WHERE ticket_id=$1 AND generation=1 AND state IN ('queued','running'))").bind(ticket).fetch_one(pool).await?;
            if !active {
                break;
            }
            tokio::select! { result=listener.recv()=> {result?;}, _=tokio::time::sleep(Duration::from_secs(2))=>{} }
        }
    }
    Ok(())
}

pub struct Runner {
    pool: PgPool,
    runtime: Runtime,
    listener: PgListener,
    owner: sqlx::PgConnection,
}
impl Runner {
    pub async fn start(pool: PgPool, mut config: RuntimeConfig) -> anyhow::Result<Self> {
        let mut owner = pool.acquire().await?.detach();
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(7358710404)")
            .fetch_one(&mut owner)
            .await?;
        anyhow::ensure!(locked, "another daemon owns Automations");
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen("agentinc_automations").await?;
        config.worker_group.push_str("-automations");
        let runtime = Runtime::recurring(config, Arc::new(Action(pool.clone()))).await?;
        Ok(Self {
            pool,
            runtime,
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
        loop {
            sqlx::query("SELECT 1").execute(&mut self.owner).await?;
            self.reconcile().await?;
            tokio::select! {
                           _ = &mut shutdown => { self.runtime.shutdown().await?; return Ok(()); }
            result=self.listener.recv()=> { result?; }, _=tokio::time::sleep(Duration::from_secs(2))=>{} }
        }
    }
    async fn reconcile(&self) -> anyhow::Result<()> {
        let rules:Vec<Automation>=sqlx::query_as("SELECT id,name,prompt,agent_id,every_minutes,paused,revision,applied_revision,error,missed,overlap_skipped FROM automations ORDER BY id").fetch_all(&self.pool).await?;
        for rule in rules {
            if let Err(error) = self.reconcile_rule(&rule).await {
                sqlx::query("UPDATE automations SET error=$2 WHERE id=$1")
                    .bind(&rule.id)
                    .bind(error.to_string())
                    .execute(&self.pool)
                    .await?;
            }
        }
        let manual: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT id,automation_id,revision FROM occurrences WHERE manual AND NOT dispatched",
        )
        .fetch_all(&self.pool)
        .await?;
        for (id, rule, revision) in manual {
            match self
                .runtime
                .run_occurrence(Occurrence {
                    id: id.clone(),
                    input: json!({"id":rule,"revision":revision,"manual":true}),
                })
                .await
            {
                Ok(()) => {
                    sqlx::query("UPDATE occurrences SET dispatched=true,detail=NULL WHERE id=$1")
                        .bind(id)
                        .execute(&self.pool)
                        .await?;
                }
                Err(error) => {
                    sqlx::query("UPDATE occurrences SET detail=$2 WHERE id=$1")
                        .bind(id)
                        .bind(format!("Waiting for runtime: {error}"))
                        .execute(&self.pool)
                        .await?;
                }
            }
        }
        Ok(())
    }
    async fn reconcile_rule(&self, rule: &Automation) -> anyhow::Result<()> {
        if rule.revision != rule.applied_revision {
            self.runtime
                .apply_rule(RecurringRule {
                    id: rule.id.clone(),
                    every: Duration::from_secs(rule.every_minutes as u64 * 60),
                    paused: rule.paused,
                    input: json!({"id":rule.id,"revision":rule.revision}),
                })
                .await?;
            sqlx::query(
                "UPDATE automations SET applied_revision=$2,error=NULL WHERE id=$1 AND revision=$2",
            )
            .bind(&rule.id)
            .bind(rule.revision)
            .execute(&self.pool)
            .await?;
        }
        let observed = self.runtime.recurring_state(&rule.id).await?;
        let mut tx = self.pool.begin().await?;
        for (kind, count, old) in [
            ("missed", observed.missed, rule.missed),
            (
                "overlap_skipped",
                observed.overlap_skipped,
                rule.overlap_skipped,
            ),
        ] {
            if count > old {
                sqlx::query(
                    "INSERT INTO automation_history(automation_id,kind,count) VALUES($1,$2,$3)",
                )
                .bind(&rule.id)
                .bind(kind)
                .bind(count - old)
                .execute(&mut *tx)
                .await?;
            }
        }
        for recent in observed.recent {
            sqlx::query("INSERT INTO occurrences(id,automation_id,revision,dispatched,scheduled_at) VALUES($1,$2,$3,true,$4) ON CONFLICT(id) DO UPDATE SET scheduled_at=excluded.scheduled_at").bind(recent.id).bind(&rule.id).bind(rule.revision).bind(recent.scheduled_at).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE automations SET missed=$2,overlap_skipped=$3,error=$4 WHERE id=$1")
            .bind(&rule.id)
            .bind(observed.missed)
            .bind(observed.overlap_skipped)
            .bind(if observed.paused != rule.paused {
                Some("Rule pause state differs from the applied definition")
            } else {
                None
            })
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn save(pool: &PgPool) -> String {
        tickets::execute(
            pool,
            &Actor::owner(),
            TicketCommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: TicketCommand::RegisterAgent {
                    name: "Fixture".into(),
                    instructions: "Scripted work".into(),
                    model: "fixture".into(),
                },
            },
        )
        .await
        .unwrap();
        let agent_id = sqlx::query_scalar("SELECT id FROM agents")
            .fetch_one(pool)
            .await
            .unwrap();
        execute(
            pool,
            &Actor::owner(),
            AutomationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: AutomationCommand::Save {
                    id: None,
                    revision: None,
                    name: "Every minute".into(),
                    proposal: TicketProposal {
                        title: "One scheduled Ticket".into(),
                        agent_id,
                    },
                    every_minutes: 1,
                },
            },
        )
        .await
        .unwrap()
        .result_id
    }
    async fn occurrence(pool: &PgPool, id: &str) -> OccurrenceView {
        loop {
            let snapshot = snapshot(pool, &Actor::owner()).await.unwrap();
            if let Some(o) = snapshot
                .occurrences
                .into_iter()
                .find(|o| o.automation_id == id && o.ticket_id.is_some())
            {
                return o;
            }
            tokio::task::yield_now().await;
        }
    }
    #[sqlx::test]
    async fn receipts_scope_revisions_and_bounded_authorization(pool: PgPool) {
        let id = save(&pool).await;
        let request = AutomationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command: AutomationCommand::RunNow {
                id: id.clone(),
                revision: 0,
            },
        };
        let owner = Actor::owner();
        let (a, b) = tokio::join!(
            execute(&pool, &owner, request.clone()),
            execute(&pool, &owner, request.clone())
        );
        assert_eq!(a.unwrap().result_id, b.unwrap().result_id);
        assert_eq!(
            snapshot(&pool, &Actor::owner())
                .await
                .unwrap()
                .occurrences
                .len(),
            1
        );
        let mut other = request.clone();
        other.command = AutomationCommand::Pause {
            id: id.clone(),
            revision: 0,
            paused: true,
        };
        assert!(execute(&pool, &Actor::owner(), other).await.is_err());
        let alien = Actor::owner_in("other".into());
        assert!(snapshot(&pool, &alien).await.unwrap().rules.is_empty());
        assert!(execute(&pool, &alien, request.clone()).await.is_err());
        let assigned = Actor {
            assignment: Some((1, 1)),
            ..Actor::owner()
        };
        assert!(execute(&pool, &assigned, request).await.is_err());
        execute(
            &pool,
            &Actor::owner(),
            AutomationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: AutomationCommand::Pause {
                    id: id.clone(),
                    revision: 0,
                    paused: true,
                },
            },
        )
        .await
        .unwrap();
        let invocation = Occurrence {
            id: "old-rule-firing".into(),
            input: json!({"id":id,"revision":0}),
        };
        apply_occurrence(&pool, invocation).await.unwrap();
        let snapshot = snapshot(&pool, &Actor::owner()).await.unwrap();
        assert!(snapshot.occurrences.iter().any(|o| o.state == "superseded"));
        assert!(snapshot.occurrences.iter().all(|o| o.ticket_id.is_none()));
    }
    #[sqlx::test]
    async fn real_rule_survives_worker_restart_and_occurrence_retry_without_duplicate_assignment(
        pool: PgPool,
    ) {
        let id = save(&pool).await;
        let server = turnkeel::testing::Server::start().await.unwrap();
        let runner = Runner::start(pool.clone(), server.config()).await.unwrap();
        runner.reconcile().await.unwrap();
        assert_eq!(
            snapshot(&pool, &Actor::owner()).await.unwrap().rules[0].applied_revision,
            0
        );
        runner.runtime.shutdown().await.unwrap();
        drop(runner.owner);
        // This is a real retained Schedule, fired with its worker unavailable.
        server.fire_rule(&id).await.unwrap();
        let runner = Runner::start(pool.clone(), server.config()).await.unwrap();
        runner.reconcile().await.unwrap();
        let first = occurrence(&pool, &id).await;
        assert_eq!(first.state, "worker_unavailable");
        let ticket = first.ticket_id.unwrap();
        server.fire_rule(&id).await.unwrap();
        loop {
            runner.reconcile().await.unwrap();
            let history = snapshot(&pool, &Actor::owner()).await.unwrap();
            if history.rules[0].overlap_skipped > 0 {
                assert!(
                    history
                        .history
                        .iter()
                        .any(|h| h.kind == "overlap_skipped" && h.count > 0)
                );
                break;
            }
            tokio::task::yield_now().await;
        }
        // Retry the same invocation while its work is still open.
        let duplicate = tokio::spawn({
            let pool = pool.clone();
            let id = id.clone();
            let occurrence_id = first.id.clone();
            async move {
                apply_occurrence(
                    &pool,
                    Occurrence {
                        id: occurrence_id,
                        input: json!({"id":id,"revision":0}),
                    },
                )
                .await
            }
        });
        let manual = execute(
            &pool,
            &Actor::owner(),
            AutomationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: AutomationCommand::RunNow {
                    id: id.clone(),
                    revision: 0,
                },
            },
        )
        .await
        .unwrap();
        apply_occurrence(
            &pool,
            Occurrence {
                id: manual.result_id.clone(),
                input: json!({"id":id,"revision":0,"manual":true}),
            },
        )
        .await
        .unwrap();
        let snapshot = snapshot(&pool, &Actor::owner()).await.unwrap();
        assert_eq!(
            snapshot
                .occurrences
                .iter()
                .find(|o| o.id == manual.result_id)
                .unwrap()
                .state,
            "overlap_skipped"
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM dispatch_outbox")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        // Terminal completion releases both executions and retains a single receipt.
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("UPDATE ticket_runs SET state='completed' WHERE ticket_id=$1")
            .bind(ticket)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("SELECT pg_notify('agentinc_results','')")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        duplicate.await.unwrap().unwrap();
        apply_occurrence(
            &pool,
            Occurrence {
                id: first.id,
                input: json!({"id":id,"revision":0}),
            },
        )
        .await
        .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        runner.runtime.shutdown().await.unwrap();
        server.shutdown().await.unwrap();
    }
    #[sqlx::test]
    async fn scheduled_ticket_effect_is_not_repeated_after_occurrence_retry(pool: PgPool) {
        use crate::{
            coding::WorkspacePolicy,
            execution::{ModelCatalog, Runner as TicketRunner},
        };
        use turnkeel::{
            Model, ModelError,
            testing::{ScriptedModel, text, tool_call},
        };
        struct Models(Arc<dyn Model>);
        impl ModelCatalog for Models {
            fn resolve(&self, _: &str) -> Result<Arc<dyn Model>, ModelError> {
                Ok(self.0.clone())
            }
        }
        let id = save(&pool).await;
        let server = turnkeel::testing::Server::start().await.unwrap();
        let runner = Runner::start(pool.clone(), server.config()).await.unwrap();
        runner.reconcile().await.unwrap();
        let model = ScriptedModel::new()
            .on_user(
                "One scheduled Ticket",
                tool_call("comment", json!({"body":"One scheduled effect"})),
            )
            .on_tool_result("comment", text("Scheduled work completed"));
        let directory = tempfile::tempdir().unwrap();
        let tickets = TicketRunner::start(
            pool.clone(),
            server.config(),
            Arc::new(Models(Arc::new(model))),
            WorkspacePolicy::new(directory.path(), vec![]).unwrap(),
        )
        .await
        .unwrap();
        let mut results = PgListener::connect_with(&pool).await.unwrap();
        results.listen("agentinc_results").await.unwrap();
        let worker = tokio::spawn(tickets.run());
        server.fire_rule(&id).await.unwrap();
        results.recv().await.unwrap();
        let first = occurrence(&pool, &id).await;
        assert_eq!(first.state, "completed");
        apply_occurrence(
            &pool,
            Occurrence {
                id: first.id,
                input: json!({"id":id,"revision":0}),
            },
        )
        .await
        .unwrap();
        let bodies: Vec<String> = sqlx::query_scalar("SELECT body FROM comments ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            bodies,
            vec!["One scheduled effect", "Scheduled work completed"]
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        worker.abort();
        let _ = worker.await;
        runner.runtime.shutdown().await.unwrap();
        server.shutdown().await.unwrap();
    }
}
