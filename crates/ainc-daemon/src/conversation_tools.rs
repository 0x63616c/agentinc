//! Evee uses the same scoped commands as the owner UI. Coding effects remain on
//! assigned Tickets; Conversations receive no file, shell or git capability.
use crate::tickets::{self, Actor, TicketCommandRequest};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use turnkeel::{Tool, ToolCtx, ToolError};

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
            "Read the owner's Tickets (status, priority, labels, board order), relationships, Comments, assignees and work results."
        }
    }
    fn schema(&self) -> Value {
        if !self.mutation {
            return json!({"type":"object","properties":{},"additionalProperties":false});
        }
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
                Value::Array(values) => {
                    Value::Array(values.iter().map(|v| inline(v, api)).collect())
                }
                value => value.clone(),
            }
        }
        json!({"type":"object","properties":{"command":inline(&api["components"]["schemas"]["TicketCommand"],&api)},"required":["command"],"additionalProperties":false})
    }
    fn idempotent(&self) -> bool {
        self.mutation
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let this = self.clone();
        Box::pin(async move {
            let origin: Option<(String, i64)> = sqlx::query_as("SELECT c.workspace_id,c.id FROM conversation_sessions s JOIN conversations c ON c.id=s.conversation_id WHERE s.id=$1 AND s.state='active'").bind(&this.session_id).fetch_optional(&this.pool).await.map_err(|_|ToolError::Failed("Ticket service unavailable".into()))?;
            let Some((workspace, conversation)) = origin else {
                return Err(ToolError::InvalidArguments(
                    "Conversation is no longer active".into(),
                ));
            };
            // History records that this Conversation made the change through Evee.
            let actor = Actor {
                conversation: Some(conversation),
                ..Actor::owner_in(workspace)
            };
            if this.mutation {
                let command = serde_json::from_value(args["command"].clone()).map_err(|e| {
                    ToolError::InvalidArguments(format!("Invalid Ticket command: {e}"))
                })?;
                let digest = Sha256::digest(ctx.idempotency_key().as_bytes());
                let operation_id =
                    uuid::Uuid::from_bytes(digest[..16].try_into().expect("SHA-256 length"))
                        .to_string();
                let receipt=tickets::execute(&this.pool,&actor,TicketCommandRequest{operation_id,command}).await.map_err(|_|ToolError::InvalidArguments("Command refused. Read current Ticket revisions before retrying; verify the assignee and arguments.".into()))?;
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
        if !self.mutation {
            return json!({"type":"object","properties":{},"additionalProperties":false});
        }
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
                Value::Array(values) => {
                    Value::Array(values.iter().map(|v| inline(v, api)).collect())
                }
                value => value.clone(),
            }
        }
        json!({"type":"object","properties":{"command":inline(&api["components"]["schemas"]["AutomationCommand"],&api)},"required":["command"],"additionalProperties":false})
    }
    fn idempotent(&self) -> bool {
        self.mutation
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let this = self.clone();
        Box::pin(async move {
            let workspace: Option<String> = sqlx::query_scalar("SELECT c.workspace_id FROM conversation_sessions s JOIN conversations c ON c.id=s.conversation_id WHERE s.id=$1 AND s.state='active'").bind(&this.session_id).fetch_optional(&this.pool).await.map_err(|_|ToolError::Failed("Automation service unavailable".into()))?;
            let Some(workspace) = workspace else {
                return Err(ToolError::InvalidArguments(
                    "Conversation is no longer active".into(),
                ));
            };
            let actor = Actor::owner_in(workspace);
            if this.mutation {
                let command = serde_json::from_value(args["command"].clone()).map_err(|e| {
                    ToolError::InvalidArguments(format!("Invalid Automation command: {e}"))
                })?;
                let digest = Sha256::digest(ctx.idempotency_key().as_bytes());
                let operation_id =
                    uuid::Uuid::from_bytes(digest[..16].try_into().expect("SHA-256 length"))
                        .to_string();
                let receipt=crate::automations::execute(&this.pool,&actor,crate::automations::AutomationRequest{operation_id,command}).await.map_err(|_|ToolError::InvalidArguments("Command refused. Read current Automation revisions before retrying; verify the assignee and arguments.".into()))?;
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
