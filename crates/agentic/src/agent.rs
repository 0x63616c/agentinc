use crate::{Model, Tool, ToolSet};
use std::sync::Arc;

/// An agent definition: a model, instructions, and tools. Cheap to clone.
#[derive(Clone)]
pub struct Agent {
    pub(crate) name: String,
    pub(crate) model: Arc<dyn Model>,
    pub(crate) instructions: String,
    pub(crate) tools: ToolSet,
    pub(crate) max_turns: u32,
}

impl Agent {
    pub fn builder(name: impl Into<String>) -> AgentBuilder {
        AgentBuilder {
            name: name.into(),
            model: None,
            instructions: String::new(),
            tools: ToolSet::default(),
            max_turns: 20,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

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
            .field("max_turns", &self.max_turns)
            .finish()
    }
}

/// Builds an [`Agent`].
pub struct AgentBuilder {
    name: String,
    model: Option<Arc<dyn Model>>,
    instructions: String,
    tools: ToolSet,
    max_turns: u32,
}

impl AgentBuilder {
    pub fn model(mut self, model: impl Model) -> Self {
        self.model = Some(Arc::new(model));
        self
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = instructions.into();
        self
    }

    pub fn tool(mut self, tool: impl Tool) -> Self {
        self.tools.insert(tool);
        self
    }

    /// Stop the run with an error after this many model calls. Default 20.
    pub fn max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = max_turns;
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
            max_turns: self.max_turns,
        }
    }
}
