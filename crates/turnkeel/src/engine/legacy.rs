//! Durable names from before the SDK was called turnkeel.
//!
//! Workflow and activity type names are written into every retained history. Runs and
//! sessions started under these names must still replay and keep taking turns, so the
//! worker registers them as aliases of the current `turnkeel.*` definitions. Nothing new
//! is ever started under them. This is the one file exempt from the SDK vocabulary check.

pub(crate) const RUN: &str = "agentinc.run";
pub(crate) const SESSION: &str = "agentinc.session";
pub(crate) const MODEL_STEP: &str = "agentinc.model_step";
pub(crate) const CALL_TOOL: &str = "agentinc.call_tool";
