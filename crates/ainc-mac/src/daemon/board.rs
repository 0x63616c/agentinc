//! The in-memory daemon's Ticket board: the daemon's visible rules (column
//! order, no-op edits, repeat-safe links, history sides) over a snapshot, so
//! tests and captures show what the real app would. Pure: state in, state out.
use crate::tickets::model::{apply_move, history_sides, priority_key, status_key};
use ainc_client::types::{
    ActivityKind, Assignee, AssigneeKind, Comment, ErrorBody, ErrorCode, LinkKind, Ticket,
    TicketActivity, TicketCommand, TicketLink, TicketPriority, TicketSnapshot, TicketStatus,
};

fn refused(message: &str) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::Invalid,
        message: message.into(),
    }
}

/// Apply one Ticket command, recording history. Returns the acknowledged id.
pub fn apply(
    s: &mut TicketSnapshot,
    log: &mut Vec<TicketActivity>,
    now: i64,
    command: TicketCommand,
) -> Result<Option<i64>, ErrorBody> {
    let mut record = |ticket_id: i64, kind, from: Option<String>, to: Option<String>| {
        let id = log.len() as i64 + 1;
        log.push(TicketActivity {
            id,
            ticket_id,
            actor_id: "owner".into(),
            kind,
            from_value: from,
            to_value: to,
            conversation_id: None,
            run_id: None,
            created_at: now,
        });
    };
    let index = |s: &TicketSnapshot, id: i64| -> Result<usize, ErrorBody> {
        s.tickets
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| refused("No such Ticket"))
    };
    match command {
        TicketCommand::Delete { id, .. } => {
            s.tickets.retain(|t| t.id != id);
            s.comments.retain(|c| c.ticket_id != id);
            s.links.retain(|l| l.from_id != id && l.to_id != id);
            Ok(Some(id))
        }
        TicketCommand::Create { title } => create(
            s,
            TicketCommand::CreateDetailed {
                title,
                description: None,
                status: None,
                priority: None,
                labels: vec![],
                assignee_id: None,
            },
            now,
            &mut record,
        ),
        command @ TicketCommand::CreateDetailed { .. } => create(s, command, now, &mut record),
        TicketCommand::SetStatus { id, status, .. }
        | TicketCommand::Move {
            id,
            status,
            after: None,
            ..
        } => {
            let from = s.tickets[index(s, id)?].status;
            apply_move(&mut s.tickets, id, status, None);
            if from != status {
                record(
                    id,
                    ActivityKind::Status,
                    Some(status_key(from).into()),
                    Some(status_key(status).into()),
                );
            }
            Ok(Some(id))
        }
        TicketCommand::Move {
            id, status, after, ..
        } => {
            let from = s.tickets[index(s, id)?].status;
            if !apply_move(&mut s.tickets, id, status, after) {
                return Err(refused("Stale board"));
            }
            if from != status {
                record(
                    id,
                    ActivityKind::Status,
                    Some(status_key(from).into()),
                    Some(status_key(status).into()),
                );
            }
            Ok(Some(id))
        }
        TicketCommand::Rename { id, title, .. } => {
            let i = index(s, id)?;
            let title = title.trim().to_owned();
            if s.tickets[i].title != title {
                let old = std::mem::replace(&mut s.tickets[i].title, title.clone());
                s.tickets[i].revision += 1;
                record(id, ActivityKind::Renamed, Some(old), Some(title));
            }
            Ok(Some(id))
        }
        TicketCommand::Describe {
            id, description, ..
        } => {
            let i = index(s, id)?;
            if s.tickets[i].description != description.trim() {
                s.tickets[i].description = description.trim().into();
                s.tickets[i].revision += 1;
                record(id, ActivityKind::Described, None, None);
            }
            Ok(Some(id))
        }
        TicketCommand::SetPriority { id, priority, .. } => {
            let i = index(s, id)?;
            let old = s.tickets[i].priority;
            if old != priority {
                s.tickets[i].priority = priority;
                s.tickets[i].revision += 1;
                record(
                    id,
                    ActivityKind::Priority,
                    Some(priority_key(old).into()),
                    Some(priority_key(priority).into()),
                );
            }
            Ok(Some(id))
        }
        TicketCommand::SetLabels { id, labels, .. } => {
            let i = index(s, id)?;
            if s.tickets[i].labels != labels {
                let old = std::mem::replace(&mut s.tickets[i].labels, labels.clone());
                s.tickets[i].revision += 1;
                record(
                    id,
                    ActivityKind::Labels,
                    Some(old.join(",")),
                    Some(labels.join(",")),
                );
            }
            Ok(Some(id))
        }
        TicketCommand::Assign {
            id,
            assignee_id,
            assignee_kind,
            ..
        } => {
            let i = index(s, id)?;
            let old = std::mem::replace(&mut s.tickets[i].assignee_id, assignee_id.clone());
            s.tickets[i].assignee_kind = assignee_kind;
            s.tickets[i].revision += 1;
            record(id, ActivityKind::Assigned, Some(old), Some(assignee_id));
            Ok(Some(id))
        }
        TicketCommand::Link {
            from_id,
            to_id,
            link,
        } => {
            let (from_id, to_id) = if link == LinkKind::RelatesTo {
                (from_id.min(to_id), from_id.max(to_id))
            } else {
                (from_id, to_id)
            };
            if !s
                .links
                .iter()
                .any(|l| (l.from_id, l.to_id, l.kind) == (from_id, to_id, link))
            {
                s.links.push(TicketLink {
                    from_id,
                    to_id,
                    kind: link,
                });
                let (source, target) = history_sides(link);
                record(
                    from_id,
                    ActivityKind::Linked,
                    Some(source.into()),
                    Some(to_id.to_string()),
                );
                record(
                    to_id,
                    ActivityKind::Linked,
                    Some(target.into()),
                    Some(from_id.to_string()),
                );
            }
            Ok(Some(from_id))
        }
        TicketCommand::Unlink {
            from_id,
            to_id,
            link,
        } => {
            let before = s.links.len();
            s.links
                .retain(|l| (l.from_id, l.to_id, l.kind) != (from_id, to_id, link));
            if s.links.len() < before {
                let (source, target) = history_sides(link);
                record(
                    from_id,
                    ActivityKind::Unlinked,
                    Some(source.into()),
                    Some(to_id.to_string()),
                );
                record(
                    to_id,
                    ActivityKind::Unlinked,
                    Some(target.into()),
                    Some(from_id.to_string()),
                );
            }
            Ok(Some(from_id))
        }
        TicketCommand::AddComment { ticket_id, body } => {
            let id = s.comments.len() as i64 + 1;
            s.comments.push(Comment {
                id,
                ticket_id,
                body,
                author_id: "owner".into(),
                created_at: now,
            });
            Ok(Some(id))
        }
        TicketCommand::RegisterAgent { name, .. } => {
            let id = format!("agent-{}", s.assignees.len());
            s.assignees.push(Assignee {
                id,
                name,
                kind: AssigneeKind::Agent,
            });
            Ok(None)
        }
        _ => Err(refused("No in-memory rule for this Ticket command")),
    }
}

fn create(
    s: &mut TicketSnapshot,
    command: TicketCommand,
    now: i64,
    record: &mut impl FnMut(i64, ActivityKind, Option<String>, Option<String>),
) -> Result<Option<i64>, ErrorBody> {
    let TicketCommand::CreateDetailed {
        title,
        description,
        status,
        priority,
        labels,
        assignee_id,
    } = command
    else {
        return Err(refused("Not a create command"));
    };
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 500 {
        return Err(refused("Invalid title"));
    }
    let id = s.tickets.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    let status = status.unwrap_or(TicketStatus::ToDo);
    let (assignee_kind, assignee_id) = match assignee_id {
        Some(id) => (
            s.assignees
                .iter()
                .find(|a| a.id == id)
                .ok_or_else(|| refused("Unknown assignee"))?
                .kind,
            id,
        ),
        None => (AssigneeKind::Human, "owner".into()),
    };
    s.tickets.push(Ticket {
        id,
        title: title.into(),
        description: description.unwrap_or_default(),
        status,
        priority: priority.unwrap_or(TicketPriority::None),
        labels,
        assignee_id,
        assignee_kind,
        position: 0,
        generation: 0,
        revision: 0,
        created_at: now,
        updated_at: now,
        conversation_id: None,
    });
    apply_move(&mut s.tickets, id, status, None);
    record(id, ActivityKind::Created, None, Some(title.into()));
    Ok(Some(id))
}
