//! # agentinc
//!
//! Durable AI agents in Rust.
//!
//! ```ignore
//! use agentinc::{Agent, Agentinc, tool};
//!
//! /// Get the current weather for a city.
//! #[tool]
//! async fn get_weather(city: String) -> anyhow::Result<String> { Ok(format!("{city}: sunny")) }
//!
//! let agentinc = Agentinc::local().await?;
//! let agent = Agent::builder("weather-bot").model(model).tool(get_weather).build();
//! let answer = agentinc.start(&agent, "Weather in Lisbon?").await?.result().await?;
//! ```

mod agent;
mod engine;
mod error;
mod event;
mod message;
mod model;
mod run;
mod runtime;
mod session;
pub mod testing;
mod tool;

pub use agent::{Agent, AgentBuilder};
pub use agentinc_macros::tool;
pub use error::Error;
pub use event::Event;
pub use message::{Content, Message, Role};
pub use model::{Model, ModelError, ModelRequest, ModelResponse, StopReason, ToolSpec};
pub use run::{Run, RunId};
pub use runtime::Agentinc;
pub use session::{Session, SessionId};
pub use tool::{Tool, ToolCtx, ToolError, ToolSet};

/// Re-exports used by generated code. Not part of the public API.
#[doc(hidden)]
pub mod __private {
    pub use futures::future::BoxFuture;
    pub use schemars;
    pub use serde;
    pub use serde_json;
}
