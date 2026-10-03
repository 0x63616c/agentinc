//! Every integration test that runs against the local Temporal dev server, linked as one
//! binary so the heavy dependency graph is linked once. `recovery` re-executes this binary
//! with `--exact recovery::recovery_worker` and `recovery::effect_worker`.

mod agent_loop;
mod history;
mod recovery;
mod recurring;
mod script;
mod session;
mod source;
