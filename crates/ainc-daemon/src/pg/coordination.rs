//! How daemon processes and background runners coordinate through Postgres: the NOTIFY
//! channels they wake each other on and the advisory locks that give each runner one owner.

/// A Ticket was assigned or its work was cancelled; the dispatcher has something to do.
pub const DISPATCH: &str = "agentinc_dispatch";
/// A Conversation turn was queued.
pub const TURNS: &str = "agentinc_turns";
/// An execution (Ticket run or Conversation turn) reached a terminal state.
pub const RESULTS: &str = "agentinc_results";
/// An Automation was saved, paused or run manually.
pub const AUTOMATIONS: &str = "agentinc_automations";
/// An Automation's saved revision was applied to its schedule.
pub const AUTOMATIONS_APPLIED: &str = "agentinc_automations_applied";

/// Advisory lock held by the one daemon dispatching Ticket work.
pub const TICKET_DISPATCH_LOCK: i64 = 7_358_710_303;
/// Advisory lock held by the one daemon running Conversation turns.
pub const CONVERSATION_LOCK: i64 = 7_358_710_202;
/// Advisory lock held by the one daemon reconciling Automations.
pub const AUTOMATION_LOCK: i64 = 7_358_710_404;
