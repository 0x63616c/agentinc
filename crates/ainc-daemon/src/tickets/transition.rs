//! What a status change does to a Ticket's live work, decided from who asks
//! and where the Ticket is. Pure, so the rule is tested without Postgres.
use super::{AssigneeKind, TicketStatus};

/// The steps a status change takes, in order: stop the current generation's
/// work, bump the generation, move the Ticket, then start new work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    /// Cancel any queued or running run of the current generation.
    pub cancel: bool,
    /// Begin a new assignment generation, making the old one's credentials stale.
    pub bump_generation: bool,
    /// Queue a run for the new generation.
    pub start: bool,
}

/// The actor may not make this change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refused;

/// `None` when the Ticket is already in `to`. The owner's change stops live
/// work and starts a fresh generation when `to` is actionable for an agent
/// assignee; an agent may only move the In progress work it holds, and that
/// finishes the generation rather than replacing it.
pub(super) fn plan(
    by_owner: bool,
    from: TicketStatus,
    to: TicketStatus,
    assignee: AssigneeKind,
) -> Result<Option<Plan>, Refused> {
    if from == to {
        return Ok(None);
    }
    if !by_owner {
        if from != TicketStatus::InProgress {
            return Err(Refused);
        }
        return Ok(Some(Plan {
            cancel: false,
            bump_generation: false,
            start: false,
        }));
    }
    Ok(Some(Plan {
        cancel: true,
        bump_generation: true,
        start: to.actionable() && assignee == AssigneeKind::Agent,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use TicketStatus::*;

    const OWNER: bool = true;
    const AGENT: bool = false;

    #[test]
    fn same_status_is_a_no_op_for_anyone() {
        for status in TicketStatus::ALL {
            assert_eq!(plan(OWNER, status, status, AssigneeKind::Agent), Ok(None));
            assert_eq!(plan(AGENT, status, status, AssigneeKind::Agent), Ok(None));
        }
    }

    #[test]
    fn the_owner_stops_work_and_restarts_it_only_where_an_agent_can_act() {
        let restart = Some(Plan {
            cancel: true,
            bump_generation: true,
            start: true,
        });
        let stop = Some(Plan {
            cancel: true,
            bump_generation: true,
            start: false,
        });
        assert_eq!(plan(OWNER, Backlog, ToDo, AssigneeKind::Agent), Ok(restart));
        assert_eq!(
            plan(OWNER, Blocked, InProgress, AssigneeKind::Agent),
            Ok(restart)
        );
        assert_eq!(
            plan(OWNER, InProgress, ToDo, AssigneeKind::Agent),
            Ok(restart)
        );
        for to in [Backlog, Blocked, Done, Cancelled] {
            assert_eq!(plan(OWNER, InProgress, to, AssigneeKind::Agent), Ok(stop));
        }
        // A human assignee has no run to start; the generation still turns over.
        assert_eq!(plan(OWNER, Backlog, ToDo, AssigneeKind::Human), Ok(stop));
    }

    #[test]
    fn an_agent_only_finishes_in_progress_work_and_keeps_its_generation() {
        let finish = Some(Plan {
            cancel: false,
            bump_generation: false,
            start: false,
        });
        assert_eq!(
            plan(AGENT, InProgress, Done, AssigneeKind::Agent),
            Ok(finish)
        );
        for from in [Backlog, ToDo, Blocked, Done, Cancelled] {
            if from != Done {
                assert_eq!(plan(AGENT, from, Done, AssigneeKind::Agent), Err(Refused));
            }
        }
    }
}
