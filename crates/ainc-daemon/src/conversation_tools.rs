//! Evee uses the same scoped commands as the owner UI. Coding effects remain on
//! assigned Tickets; Conversations receive no file, shell or git capability.
use crate::{
    home::Home,
    tickets::{self, Actor, TicketCommandRequest},
};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use turnkeel::{Tool, ToolCtx, ToolError};

/// A read tool takes no arguments; a command tool takes one API command.
fn schema(command: Option<&str>) -> Value {
    let Some(command) = command else {
        return json!({"type":"object","properties":{},"additionalProperties":false});
    };
    let api = crate::openapi();
    fn inline(value: &Value, api: &Value) -> Value {
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            return inline(
                api.pointer(reference.trim_start_matches('#'))
                    .expect("local API schema reference"),
                api,
            );
        }
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, value)| (key.clone(), inline(value, api)))
                    .collect(),
            ),
            Value::Array(values) => Value::Array(values.iter().map(|v| inline(v, api)).collect()),
            value => value.clone(),
        }
    }
    json!({"type":"object","properties":{"command":inline(&api["components"]["schemas"][command],&api)},"required":["command"],"additionalProperties":false})
}
/// The owner in the Conversation's workspace, while the session is active.
async fn actor(pool: &PgPool, session_id: &str, service: &str) -> Result<Actor, ToolError> {
    let workspace: Option<String> = sqlx::query_scalar("SELECT c.workspace_id FROM conversation_sessions s JOIN conversations c ON c.id=s.conversation_id WHERE s.id=$1 AND s.state='active'")
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ToolError::Failed(format!("{service} service unavailable")))?;
    workspace
        .map(Actor::owner_in)
        .ok_or_else(|| ToolError::InvalidArguments("Conversation is no longer active".into()))
}
/// A retried tool call reuses its operation ID, so its receipt replays.
fn operation_id(ctx: &ToolCtx) -> String {
    let digest = Sha256::digest(ctx.idempotency_key().as_bytes());
    uuid::Uuid::from_bytes(digest[..16].try_into().expect("SHA-256 length")).to_string()
}
fn command<T: serde::de::DeserializeOwned>(args: &Value, what: &str) -> Result<T, ToolError> {
    serde_json::from_value(args["command"].clone())
        .map_err(|e| ToolError::InvalidArguments(format!("Invalid {what} command: {e}")))
}

#[derive(Clone)]
pub(crate) struct TicketsTool {
    pub pool: PgPool,
    pub session_id: String,
    pub mutation: bool,
}
impl Tool for TicketsTool {
    fn name(&self) -> &str {
        if self.mutation {
            "ticket_command"
        } else {
            "list_tickets"
        }
    }
    fn description(&self) -> &str {
        if self.mutation {
            "Apply a Ticket command requested in this Conversation. Read current revisions and assignee IDs first. Assigning actionable work to an agent starts that work. Autonomous file, shell and git work must use an assigned Ticket."
        } else {
            "Read the owner's Tickets, Comments, assignees and work results."
        }
    }
    fn schema(&self) -> Value {
        schema(self.mutation.then_some("TicketCommand"))
    }
    fn idempotent(&self) -> bool {
        self.mutation
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let this = self.clone();
        Box::pin(async move {
            let actor = actor(&this.pool, &this.session_id, "Ticket").await?;
            if this.mutation {
                let command = command(&args, "Ticket")?;
                let receipt=tickets::execute(&this.pool,&actor,TicketCommandRequest{operation_id:operation_id(&ctx),command}).await.map_err(|_|ToolError::InvalidArguments("Command refused. Read current Ticket revisions before retrying; verify the assignee and arguments.".into()))?;
                Ok(json!(receipt))
            } else {
                tickets::snapshot(&this.pool, &actor)
                    .await
                    .map(|s| json!(s))
                    .map_err(|_| ToolError::Failed("Ticket service unavailable".into()))
            }
        })
    }
}

#[derive(Clone)]
pub(crate) struct AutomationsTool {
    pub pool: PgPool,
    pub session_id: String,
    pub mutation: bool,
}
impl Tool for AutomationsTool {
    fn name(&self) -> &str {
        if self.mutation {
            "automation_command"
        } else {
            "list_automations"
        }
    }
    fn description(&self) -> &str {
        if self.mutation {
            "Save or control an Automation explicitly requested in this Conversation. A saved rule grants recurring creation and assignment of Tickets. Read rules and agent IDs first."
        } else {
            "Read the owner's Automation rules, occurrences, linked Tickets and missed firing history."
        }
    }
    fn schema(&self) -> Value {
        schema(self.mutation.then_some("AutomationCommand"))
    }
    fn idempotent(&self) -> bool {
        self.mutation
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let this = self.clone();
        Box::pin(async move {
            let actor = actor(&this.pool, &this.session_id, "Automation").await?;
            if this.mutation {
                let command = command(&args, "Automation")?;
                let receipt=crate::automations::execute(&this.pool,&actor,crate::automations::AutomationRequest{operation_id:operation_id(&ctx),command}).await.map_err(|_|ToolError::InvalidArguments("Command refused. Read current Automation revisions before retrying; verify the assignee and arguments.".into()))?;
                Ok(json!(receipt))
            } else {
                crate::automations::snapshot(&this.pool, &actor)
                    .await
                    .map(|s| json!(s))
                    .map_err(|_| ToolError::Failed("Automation service unavailable".into()))
            }
        })
    }
}

#[derive(Clone)]
pub(crate) struct HomeTool {
    pub pool: PgPool,
    pub home: Home,
    pub session_id: String,
    pub mutation: bool,
}
impl Tool for HomeTool {
    fn name(&self) -> &str {
        if self.mutation {
            "home_command"
        } else {
            "read_home"
        }
    }
    fn description(&self) -> &str {
        if self.mutation {
            "Switch lights and lamps or change the thermostat when asked in this Conversation. The change is applied durably within seconds; read the home again to confirm it."
        } else {
            "Read the home's lights, lamps, indoor temperature, thermostat and recent Smart Home changes."
        }
    }
    fn schema(&self) -> Value {
        schema(self.mutation.then_some("HomeCommand"))
    }
    fn idempotent(&self) -> bool {
        self.mutation
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let this = self.clone();
        Box::pin(async move {
            let actor = actor(&this.pool, &this.session_id, "Smart Home").await?;
            if this.mutation {
                let command = command(&args, "Smart Home")?;
                let receipt = crate::home::execute(
                    &this.pool,
                    &actor,
                    crate::home::HomeRequest {
                        operation_id: operation_id(&ctx),
                        command,
                    },
                )
                .await
                .map_err(|e| {
                    ToolError::InvalidArguments(format!("Command refused: {}", e.message()))
                })?;
                Ok(json!(receipt))
            } else {
                crate::home::snapshot(&this.pool, &this.home, &actor)
                    .await
                    .map(|s| json!(s))
                    .map_err(|_| ToolError::Failed("Smart Home service unavailable".into()))
            }
        })
    }
}

#[derive(Clone)]
pub(crate) struct CalendarTool {
    pub pool: PgPool,
    pub session_id: String,
    pub mutation: bool,
}
impl Tool for CalendarTool {
    fn name(&self) -> &str {
        if self.mutation {
            "calendar_command"
        } else {
            "list_calendar"
        }
    }
    fn description(&self) -> &str {
        if self.mutation {
            "Create, edit or delete the owner's AgentInc calendar events when asked. Times are Unix seconds; all-day events run from one local midnight to the next. Events mirrored from the Mac's calendars are read-only. Read current revisions first."
        } else {
            "Read the owner's calendar events between two Unix times (defaults: 31 days ago to 180 days ahead)."
        }
    }
    fn schema(&self) -> Value {
        if self.mutation {
            return schema(Some("CalendarCommand"));
        }
        json!({"type":"object","properties":{"from":{"type":"integer"},"to":{"type":"integer"}},"additionalProperties":false})
    }
    fn idempotent(&self) -> bool {
        self.mutation
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let this = self.clone();
        Box::pin(async move {
            let actor = actor(&this.pool, &this.session_id, "Calendar").await?;
            if this.mutation {
                let command = command(&args, "Calendar")?;
                let receipt = crate::calendar::execute(
                    &this.pool,
                    &actor,
                    crate::calendar::CalendarRequest {
                        operation_id: operation_id(&ctx),
                        command,
                    },
                )
                .await
                .map_err(|e| {
                    ToolError::InvalidArguments(format!("Command refused: {}", e.message()))
                })?;
                Ok(json!(receipt))
            } else {
                let mut snapshot = crate::calendar::snapshot(
                    &this.pool,
                    &actor,
                    args["from"].as_i64(),
                    args["to"].as_i64(),
                )
                .await
                .map_err(|e| ToolError::InvalidArguments(e.message().to_owned()))?;
                // Notes on mirrored events come from whoever sent the invite;
                // they are not instructions and stay out of the model's view.
                for event in &mut snapshot.events {
                    if event.source == crate::calendar::EventSource::Macos {
                        event.notes = None;
                    }
                }
                Ok(json!(snapshot))
            }
        })
    }
}
