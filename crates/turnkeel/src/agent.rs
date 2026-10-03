use crate::{Error, Model, Tool, ToolSet};
use std::sync::Arc;

/// An agent definition: a model, instructions, and tools. Cheap to clone.
#[derive(Clone)]
pub struct Agent {
    pub(crate) name: String,
    pub(crate) model: Arc<dyn Model>,
    pub(crate) instructions: String,
    pub(crate) tools: ToolSet,
}

impl Agent {
    /// Starts building an agent with the given name; set a model with [`AgentBuilder::model`] before building.
    pub fn builder(name: impl Into<String>) -> AgentBuilder {
        AgentBuilder {
            name: name.into(),
            model: None,
            instructions: String::new(),
            tools: ToolSet::default(),
        }
    }

    /// The agent's name, used to look up its tools and model.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The tools this agent can call.
    pub fn tools(&self) -> &ToolSet {
        &self.tools
    }
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent")
            .field("name", &self.name)
            .field("model", &self.model.id())
            .field("tools", &self.tools)
            .finish()
    }
}

/// Builds an [`Agent`].
pub struct AgentBuilder {
    name: String,
    model: Option<Arc<dyn Model>>,
    instructions: String,
    tools: ToolSet,
}

impl AgentBuilder {
    /// Sets the model that answers for this agent. Required before [`AgentBuilder::build`].
    pub fn model(mut self, model: impl Model) -> Self {
        self.model = Some(Arc::new(model));
        self
    }

    /// Sets the instructions given to the model on every request.
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = instructions.into();
        self
    }

    /// Adds a tool the model may call; a later tool with the same name replaces an earlier one.
    pub fn tool(mut self, tool: impl Tool) -> Self {
        self.tools.insert(tool);
        self
    }

    /// # Panics
    /// If no model was set.
    pub fn build(self) -> Agent {
        Agent {
            name: self.name,
            model: self
                .model
                .expect("Agent::builder(..).model(..) is required"),
            instructions: self.instructions,
            tools: self.tools,
        }
    }
}

/// Where a runtime finds agent definitions it was not given up front.
///
/// Consulted on the worker the first time a run or session names an agent this runtime
/// does not hold, and cached afterwards. Return `Ok(None)` when no such definition
/// exists: the run fails with a clear error. Return `Err` for a transient failure such as
/// an unreachable database: the step retries.
pub trait AgentSource: Send + Sync + 'static {
    /// Looks up the agent definition with this name, or returns `Ok(None)` if there is none.
    fn resolve(
        &self,
        name: &str,
    ) -> futures::future::BoxFuture<'static, Result<Option<Agent>, Error>>;
}

impl<S: AgentSource + ?Sized> AgentSource for Arc<S> {
    fn resolve(
        &self,
        name: &str,
    ) -> futures::future::BoxFuture<'static, Result<Option<Agent>, Error>> {
        (**self).resolve(name)
    }
}
