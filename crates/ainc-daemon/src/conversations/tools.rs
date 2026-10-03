//! Evee uses the same scoped commands as the owner UI. Coding effects remain on
//! assigned Tickets; Conversations receive no file, shell or git capability.
use crate::{
    receipts::OperationId,
    tickets::{self, Actor, TicketCommand, TicketCommandRequest},
};
use futures::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{future::Future, marker::PhantomData, sync::OnceLock};
use turnkeel::{Tool, ToolCtx, ToolError};

/// The tool arguments for one API command, with every `$ref` inlined so a model sees a
/// self-contained schema. Built from the OpenAPI document.
fn command_schema(name: &str) -> Value {
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
    json!({"type":"object","properties":{"command":inline(&api["components"]["schemas"][name],&api)},"required":["command"],"additionalProperties":false})
}

/// One family of API commands and the read of its state, offered to a Conversation as a
/// [`ReadTool`] and a [`CommandTool`]. Each family is described here once.
pub(crate) trait CommandFamily: Send + Sync + 'static {
    type Command: DeserializeOwned + Send + 'static;
    /// What the family is called in messages: "Ticket", "Automation".
    const LABEL: &'static str;
    const READ_TOOL: &'static str;
    const READ_DESCRIPTION: &'static str;
    const COMMAND_TOOL: &'static str;
    const COMMAND_DESCRIPTION: &'static str;
    /// The command tool's argument schema, computed once per process.
    fn schema() -> &'static Value;
    /// Who acts when the Conversation `conversation` of `workspace` calls a tool.
    fn actor(workspace: String, conversation: i64) -> Actor;
    fn execute(
        pool: &PgPool,
        actor: &Actor,
        operation_id: String,
        command: Self::Command,
    ) -> impl Future<Output = Result<Value, ToolError>> + Send;
    fn read(pool: &PgPool, actor: &Actor) -> impl Future<Output = Result<Value, ToolError>> + Send;
}

pub(crate) struct Tickets;
impl CommandFamily for Tickets {
    type Command = TicketCommand;
    const LABEL: &'static str = "Ticket";
    const READ_TOOL: &'static str = "list_tickets";
    const READ_DESCRIPTION: &'static str = "Read the owner's Tickets (status, priority, labels, board order), relationships, Comments, assignees and work results.";
    const COMMAND_TOOL: &'static str = "ticket_command";
    const COMMAND_DESCRIPTION: &'static str = "Apply a Ticket command requested in this Conversation. Read current revisions and assignee IDs first. Assigning actionable work to an agent starts that work. Autonomous file, shell and git work must use an assigned Ticket.";
    fn schema() -> &'static Value {
        static SCHEMA: OnceLock<Value> = OnceLock::new();
        SCHEMA.get_or_init(|| command_schema("TicketCommand"))
    }
    fn actor(workspace: String, conversation: i64) -> Actor {
        // History records that this Conversation made the change through Evee.
        Actor {
            conversation: Some(conversation),
            ..Actor::owner_in(workspace)
        }
    }
    async fn execute(
        pool: &PgPool,
        actor: &Actor,
        operation_id: String,
        command: TicketCommand,
    ) -> Result<Value, ToolError> {
        let receipt = tickets::execute(
            pool,
            actor,
            TicketCommandRequest {
                operation_id,
                command,
            },
        )
        .await?;
        Ok(json!(receipt))
    }
    async fn read(pool: &PgPool, actor: &Actor) -> Result<Value, ToolError> {
        tickets::snapshot(pool, actor)
            .await
            .map(|s| json!(s))
            .map_err(ToolError::from)
    }
}

pub(crate) struct Automations;
impl CommandFamily for Automations {
    type Command = crate::automations::AutomationCommand;
    const LABEL: &'static str = "Automation";
    const READ_TOOL: &'static str = "list_automations";
    const READ_DESCRIPTION: &'static str =
        "Read the owner's Automation rules, occurrences, linked Tickets and missed firing history.";
    const COMMAND_TOOL: &'static str = "automation_command";
    const COMMAND_DESCRIPTION: &'static str = "Save or control an Automation explicitly requested in this Conversation. A saved rule grants recurring creation and assignment of Tickets. Read rules and agent IDs first.";
    fn schema() -> &'static Value {
        static SCHEMA: OnceLock<Value> = OnceLock::new();
        SCHEMA.get_or_init(|| command_schema("AutomationCommand"))
    }
    fn actor(workspace: String, _conversation: i64) -> Actor {
        Actor::owner_in(workspace)
    }
    async fn execute(
        pool: &PgPool,
        actor: &Actor,
        operation_id: String,
        command: Self::Command,
    ) -> Result<Value, ToolError> {
        let receipt = crate::automations::execute(
            pool,
            actor,
            crate::automations::AutomationRequest {
                operation_id,
                command,
            },
        )
        .await?;
        Ok(json!(receipt))
    }
    async fn read(pool: &PgPool, actor: &Actor) -> Result<Value, ToolError> {
        crate::automations::snapshot(pool, actor)
            .await
            .map(|s| json!(s))
            .map_err(ToolError::from)
    }
}

/// The Conversation a tool call speaks for, if it is still active.
async fn origin<F: CommandFamily>(pool: &PgPool, session_id: &str) -> Result<Actor, ToolError> {
    let origin = sqlx::query!(
        "SELECT c.workspace_id,c.id FROM conversation_sessions s JOIN conversations c ON c.id=s.conversation_id WHERE s.id=$1 AND s.state='active'",
        session_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| ToolError::Failed(format!("{} service unavailable", F::LABEL)))?;
    let Some(origin) = origin else {
        return Err(ToolError::InvalidArguments(
            "Conversation is no longer active".into(),
        ));
    };
    Ok(F::actor(origin.workspace_id, origin.id))
}

/// Applies one of a family's commands on behalf of a Conversation.
pub(crate) struct CommandTool<F> {
    pool: PgPool,
    session_id: String,
    family: PhantomData<fn() -> F>,
}
/// Reads a family's state on behalf of a Conversation.
pub(crate) struct ReadTool<F> {
    pool: PgPool,
    session_id: String,
    family: PhantomData<fn() -> F>,
}
impl<F> CommandTool<F> {
    pub(crate) fn new(pool: &PgPool, session_id: &str) -> Self {
        Self {
            pool: pool.clone(),
            session_id: session_id.to_owned(),
            family: PhantomData,
        }
    }
}
impl<F> ReadTool<F> {
    pub(crate) fn new(pool: &PgPool, session_id: &str) -> Self {
        Self {
            pool: pool.clone(),
            session_id: session_id.to_owned(),
            family: PhantomData,
        }
    }
}
impl<F: CommandFamily> Tool for CommandTool<F> {
    fn name(&self) -> &str {
        F::COMMAND_TOOL
    }
    fn description(&self) -> &str {
        F::COMMAND_DESCRIPTION
    }
    fn schema(&self) -> Value {
        F::schema().clone()
    }
    fn idempotent(&self) -> bool {
        true
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let pool = self.pool.clone();
        let session_id = self.session_id.clone();
        Box::pin(async move {
            let actor = origin::<F>(&pool, &session_id).await?;
            let command = serde_json::from_value(args["command"].clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid {} command: {e}", F::LABEL))
            })?;
            let operation_id = OperationId::from_idempotency_key(ctx.idempotency_key()).to_string();
            F::execute(&pool, &actor, operation_id, command).await
        })
    }
}
impl<F: CommandFamily> Tool for ReadTool<F> {
    fn name(&self) -> &str {
        F::READ_TOOL
    }
    fn description(&self) -> &str {
        F::READ_DESCRIPTION
    }
    fn schema(&self) -> Value {
        json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn idempotent(&self) -> bool {
        false
    }
    fn call(&self, _ctx: ToolCtx, _args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let pool = self.pool.clone();
        let session_id = self.session_id.clone();
        Box::pin(async move {
            let actor = origin::<F>(&pool, &session_id).await?;
            F::read(&pool, &actor).await
        })
    }
}
