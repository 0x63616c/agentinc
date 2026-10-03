//! The state of a piece of agent work, however the daemon spells it. Ticket runs
//! and Assistant turns use lowercase snake case, Automation occurrences add two
//! worker states, and Temporal executions use its own PascalCase names. Every
//! page reads the state through this type; the wire strings live only here.
use super::Tone;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkState {
    WaitingForWorker,
    WorkerUnavailable,
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
    Terminated,
    TimedOut,
    ContinuedAsNew,
}

impl WorkState {
    /// The daemon's spelling of a state, or `None` for one this app does not know.
    pub fn parse(wire: &str) -> Option<Self> {
        Some(match wire {
            "waiting_for_worker" => Self::WaitingForWorker,
            "worker_unavailable" => Self::WorkerUnavailable,
            "queued" => Self::Queued,
            "running" | "Running" => Self::Running,
            "completed" | "done" | "Completed" => Self::Done,
            "failed" | "Failed" => Self::Failed,
            "cancelled" | "Canceled" => Self::Cancelled,
            "Terminated" => Self::Terminated,
            "TimedOut" => Self::TimedOut,
            "ContinuedAsNew" => Self::ContinuedAsNew,
            _ => return None,
        })
    }

    /// The daemon's spelling for Ticket runs, turns and occurrences. Temporal-only
    /// states return Temporal's spelling. Fixtures use it; pages only parse.
    #[cfg(any(test, feature = "fixtures"))]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WaitingForWorker => "waiting_for_worker",
            Self::WorkerUnavailable => "worker_unavailable",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Done => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Terminated => "Terminated",
            Self::TimedOut => "TimedOut",
            Self::ContinuedAsNew => "ContinuedAsNew",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::WaitingForWorker => "Waiting for worker",
            Self::WorkerUnavailable => "Worker unavailable",
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Done => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Terminated => "Terminated",
            Self::TimedOut => "Timed out",
            Self::ContinuedAsNew => "Continued as new",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Self::WaitingForWorker | Self::Queued => Tone::Warning,
            Self::WorkerUnavailable => Tone::Warning,
            Self::Running => Tone::Info,
            Self::Done => Tone::Success,
            Self::Failed | Self::Terminated | Self::TimedOut => Tone::Danger,
            Self::Cancelled => Tone::Neutral,
            Self::ContinuedAsNew => Tone::Accent,
        }
    }

    /// Work that is still in flight: queued or running.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

/// Whether a wire state names work that is still in flight.
pub fn is_active(wire: &str) -> bool {
    WorkState::parse(wire).is_some_and(WorkState::is_active)
}

/// A human label for a wire state; unknown states read as their words.
pub fn state_label(wire: &str) -> String {
    WorkState::parse(wire).map_or_else(|| wire.replace('_', " "), |s| s.label().to_owned())
}

/// The tone for a wire state; unknown states are neutral.
pub fn state_tone(wire: &str) -> Tone {
    WorkState::parse(wire).map_or(Tone::Neutral, WorkState::tone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_round_trips_through_its_wire_name() {
        for state in [
            WorkState::WaitingForWorker,
            WorkState::WorkerUnavailable,
            WorkState::Queued,
            WorkState::Running,
            WorkState::Done,
            WorkState::Failed,
            WorkState::Cancelled,
            WorkState::Terminated,
            WorkState::TimedOut,
            WorkState::ContinuedAsNew,
        ] {
            assert_eq!(WorkState::parse(state.as_str()), Some(state));
        }
    }

    #[test]
    fn spellings_from_every_source_agree() {
        assert_eq!(WorkState::parse("Canceled"), Some(WorkState::Cancelled));
        assert_eq!(WorkState::parse("done"), Some(WorkState::Done));
        assert_eq!(WorkState::parse("Completed"), Some(WorkState::Done));
        assert_eq!(WorkState::Cancelled.label(), "Cancelled");
        assert_eq!(WorkState::parse("dispatched"), None);
    }

    #[test]
    fn only_queued_and_running_are_active() {
        assert!(is_active("queued"));
        assert!(is_active("running"));
        assert!(!is_active("completed"));
        assert!(!is_active("nonsense"));
    }

    #[test]
    fn unknown_states_still_read() {
        assert_eq!(state_label("worker_unavailable"), "Worker unavailable");
        assert_eq!(state_label("something_else"), "something else");
        assert_eq!(state_tone("something_else"), Tone::Neutral);
        assert_eq!(state_tone("TimedOut"), Tone::Danger);
    }
}
