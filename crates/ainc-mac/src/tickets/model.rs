//! The Tickets page's vocabulary and board rules, independent of GPUI: status
//! and priority presentation, filters, relationship readings and the optimistic
//! move that mirrors the daemon's column ordering.
use crate::storage::{LinkKind, Ticket, TicketLink, TicketPriority, TicketStatus};
use crate::ui::*;

/// Board order, left to right.
pub const STATUSES: [TicketStatus; 6] = [
    TicketStatus::Backlog,
    TicketStatus::ToDo,
    TicketStatus::InProgress,
    TicketStatus::Blocked,
    TicketStatus::Done,
    TicketStatus::Cancelled,
];
/// Most to least urgent.
pub const PRIORITIES: [TicketPriority; 5] = [
    TicketPriority::Urgent,
    TicketPriority::High,
    TicketPriority::Medium,
    TicketPriority::Low,
    TicketPriority::None,
];

pub fn status_name(status: TicketStatus) -> &'static str {
    match status {
        TicketStatus::Backlog => "Backlog",
        TicketStatus::ToDo => "To do",
        TicketStatus::InProgress => "In progress",
        TicketStatus::Blocked => "Blocked",
        TicketStatus::Done => "Done",
        TicketStatus::Cancelled => "Cancelled",
    }
}
/// The wire name, used in element IDs and history entries.
pub fn status_key(status: TicketStatus) -> &'static str {
    match status {
        TicketStatus::Backlog => "backlog",
        TicketStatus::ToDo => "to_do",
        TicketStatus::InProgress => "in_progress",
        TicketStatus::Blocked => "blocked",
        TicketStatus::Done => "done",
        TicketStatus::Cancelled => "cancelled",
    }
}
pub fn status_icon(status: TicketStatus) -> &'static str {
    match status {
        TicketStatus::Backlog => "status-backlog",
        TicketStatus::ToDo => "status-todo",
        TicketStatus::InProgress => "status-progress",
        TicketStatus::Blocked => "status-blocked",
        TicketStatus::Done => "status-done",
        TicketStatus::Cancelled => "status-cancelled",
    }
}
/// Status glyphs stay gray until work is moving, stuck or finished.
pub fn status_color(status: TicketStatus) -> u32 {
    match status {
        TicketStatus::Backlog | TicketStatus::Cancelled => TEXT_TERTIARY,
        TicketStatus::ToDo => TEXT_SECONDARY,
        TicketStatus::InProgress => STATUS_AMBER,
        TicketStatus::Blocked => STATUS_RED,
        TicketStatus::Done => STATUS_GREEN,
    }
}
/// An agent assignee works on a Ticket only in these statuses.
pub fn actionable(status: TicketStatus) -> bool {
    matches!(status, TicketStatus::ToDo | TicketStatus::InProgress)
}
pub fn status_from_key(key: &str) -> Option<TicketStatus> {
    STATUSES
        .into_iter()
        .find(|status| status_key(*status) == key)
}

pub fn priority_name(priority: TicketPriority) -> &'static str {
    match priority {
        TicketPriority::Urgent => "Urgent",
        TicketPriority::High => "High",
        TicketPriority::Medium => "Medium",
        TicketPriority::Low => "Low",
        TicketPriority::None => "No priority",
    }
}
pub fn priority_key(priority: TicketPriority) -> &'static str {
    match priority {
        TicketPriority::Urgent => "urgent",
        TicketPriority::High => "high",
        TicketPriority::Medium => "medium",
        TicketPriority::Low => "low",
        TicketPriority::None => "none",
    }
}
pub fn priority_icon(priority: TicketPriority) -> &'static str {
    match priority {
        TicketPriority::Urgent => "priority-urgent",
        TicketPriority::High => "priority-high",
        TicketPriority::Medium => "priority-medium",
        TicketPriority::Low => "priority-low",
        TicketPriority::None => "priority-none",
    }
}
/// Only urgent work takes a color; the bars carry the rest.
pub fn priority_color(priority: TicketPriority) -> u32 {
    match priority {
        TicketPriority::Urgent => STATUS_RED,
        TicketPriority::None => TEXT_TERTIARY,
        _ => TEXT_SECONDARY,
    }
}
pub fn priority_from_key(key: &str) -> Option<TicketPriority> {
    PRIORITIES
        .into_iter()
        .find(|priority| priority_key(*priority) == key)
}

/// `T-12`.
pub fn ticket_key(id: i64) -> String {
    format!("T-{id}")
}

/// A stable tone per label name, so a label looks the same everywhere.
pub fn label_tone(label: &str) -> Tone {
    const TONES: [Tone; 5] = [
        Tone::Info,
        Tone::Success,
        Tone::Warning,
        Tone::Accent,
        Tone::Danger,
    ];
    let hash = label
        .to_lowercase()
        .bytes()
        .fold(2166136261u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(16777619)
        });
    TONES[hash as usize % TONES.len()]
}

/// A short age: `now`, `5m`, `3h`, `2d`, then a date.
pub fn relative_time(then: i64, now: i64) -> String {
    let seconds = (now - then).max(0);
    match seconds {
        0..60 => "now".into(),
        60..3600 => format!("{}m", seconds / 60),
        3600..86400 => format!("{}h", seconds / 3600),
        86400..604800 => format!("{}d", seconds / 86400),
        _ => chrono::DateTime::from_timestamp(then, 0)
            .map(|t| t.with_timezone(&chrono::Local).format("%b %-d").to_string())
            .unwrap_or_default(),
    }
}

/// What the board and list show: a search plus any number of values per facet.
/// An empty facet matches everything.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filters {
    pub query: String,
    pub priorities: Vec<TicketPriority>,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
}
impl Filters {
    pub fn active(&self) -> bool {
        !self.query.trim().is_empty() || self.facets() > 0
    }
    /// How many facet values are chosen, for the Clear control.
    pub fn facets(&self) -> usize {
        self.priorities.len() + self.labels.len() + self.assignees.len()
    }
    pub fn matches(&self, ticket: &Ticket) -> bool {
        (self.priorities.is_empty() || self.priorities.contains(&ticket.priority))
            && (self.assignees.is_empty() || self.assignees.contains(&ticket.assignee_id))
            && (self.labels.is_empty()
                || self.labels.iter().any(|wanted| {
                    ticket
                        .labels
                        .iter()
                        .any(|label| label.eq_ignore_ascii_case(wanted))
                }))
            && self.query_matches(ticket)
    }
    /// Every word must appear in the key, title, description or a label.
    fn query_matches(&self, ticket: &Ticket) -> bool {
        let haystack = format!(
            "{} {} {} {} {}",
            ticket_key(ticket.id),
            ticket.id,
            ticket.title,
            ticket.description,
            ticket.labels.join(" ")
        )
        .to_lowercase();
        self.query
            .to_lowercase()
            .split_whitespace()
            .all(|word| haystack.contains(word))
    }
    pub fn toggle_priority(&mut self, priority: TicketPriority) {
        toggle(&mut self.priorities, priority);
    }
    pub fn toggle_label(&mut self, label: &str) {
        toggle(&mut self.labels, label.to_owned());
    }
    pub fn toggle_assignee(&mut self, id: &str) {
        toggle(&mut self.assignees, id.to_owned());
    }
}
fn toggle<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if let Some(index) = values.iter().position(|v| *v == value) {
        values.remove(index);
    } else {
        values.push(value);
    }
}

/// Every distinct label on the board, alphabetically.
pub fn all_labels(tickets: &[Ticket]) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for label in tickets.iter().flat_map(|t| &t.labels) {
        if !labels.iter().any(|seen| seen.eq_ignore_ascii_case(label)) {
            labels.push(label.clone());
        }
    }
    labels.sort_by_key(|label| label.to_lowercase());
    labels
}

/// A relationship read from one Ticket's side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Blocks,
    BlockedBy,
    RelatesTo,
    Duplicates,
    DuplicatedBy,
    /// The other Ticket is this one's parent.
    Parent,
    /// The other Ticket is a sub-issue of this one.
    SubIssue,
}
impl Relation {
    /// The order relationships are listed and offered in.
    pub const ALL: [Self; 7] = [
        Self::Parent,
        Self::SubIssue,
        Self::BlockedBy,
        Self::Blocks,
        Self::RelatesTo,
        Self::Duplicates,
        Self::DuplicatedBy,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Blocks => "Blocks",
            Self::BlockedBy => "Blocked by",
            Self::RelatesTo => "Relates to",
            Self::Duplicates => "Duplicates",
            Self::DuplicatedBy => "Duplicated by",
            Self::Parent => "Parent",
            Self::SubIssue => "Sub-issue",
        }
    }
    /// The stored link that says `this` has this relation to `other`.
    pub fn link(self, this: i64, other: i64) -> (i64, i64, LinkKind) {
        match self {
            Self::Blocks => (this, other, LinkKind::Blocks),
            Self::BlockedBy => (other, this, LinkKind::Blocks),
            Self::RelatesTo => (this, other, LinkKind::RelatesTo),
            Self::Duplicates => (this, other, LinkKind::Duplicates),
            Self::DuplicatedBy => (other, this, LinkKind::Duplicates),
            Self::Parent => (other, this, LinkKind::ParentOf),
            Self::SubIssue => (this, other, LinkKind::ParentOf),
        }
    }
    /// How a history entry names this relation (the daemon's side names).
    pub fn from_history(side: &str) -> Option<Self> {
        Some(match side {
            "blocks" => Self::Blocks,
            "blocked_by" => Self::BlockedBy,
            "relates_to" => Self::RelatesTo,
            "duplicates" => Self::Duplicates,
            "duplicated_by" => Self::DuplicatedBy,
            "child_of" => Self::Parent,
            "parent_of" => Self::SubIssue,
            _ => return None,
        })
    }
}

/// Every relationship touching `id`, read from its side, in `Relation::ALL`
/// order then by the other Ticket's ID.
pub fn relations(links: &[TicketLink], id: i64) -> Vec<(Relation, i64)> {
    let mut found: Vec<(Relation, i64)> = links
        .iter()
        .filter_map(|link| {
            let outgoing = link.from_id == id;
            if !outgoing && link.to_id != id {
                return None;
            }
            let other = if outgoing { link.to_id } else { link.from_id };
            let relation = match (link.kind, outgoing) {
                (LinkKind::Blocks, true) => Relation::Blocks,
                (LinkKind::Blocks, false) => Relation::BlockedBy,
                (LinkKind::RelatesTo, _) => Relation::RelatesTo,
                (LinkKind::Duplicates, true) => Relation::Duplicates,
                (LinkKind::Duplicates, false) => Relation::DuplicatedBy,
                (LinkKind::ParentOf, true) => Relation::SubIssue,
                (LinkKind::ParentOf, false) => Relation::Parent,
            };
            Some((relation, other))
        })
        .collect();
    found.sort_by_key(|(relation, other)| {
        (Relation::ALL.iter().position(|r| r == relation), *other)
    });
    found
}

/// Blockers of `id` that are still open, for the card's warning.
pub fn open_blockers(tickets: &[Ticket], links: &[TicketLink], id: i64) -> Vec<i64> {
    relations(links, id)
        .into_iter()
        .filter(|(relation, _)| *relation == Relation::BlockedBy)
        .map(|(_, other)| other)
        .filter(|other| {
            tickets.iter().any(|t| {
                t.id == *other && !matches!(t.status, TicketStatus::Done | TicketStatus::Cancelled)
            })
        })
        .collect()
}

/// The Tickets in one column, top first: by position, newest first on ties.
pub fn in_column(tickets: &[Ticket], status: TicketStatus) -> Vec<&Ticket> {
    let mut column: Vec<&Ticket> = tickets.iter().filter(|t| t.status == status).collect();
    column.sort_by_key(|t| (t.position, std::cmp::Reverse(t.id)));
    column
}

/// Apply a board move locally, exactly as the daemon will: the Ticket lands in
/// `status` directly below `after` (or first) and that column is renumbered. A
/// status change bumps the revision; a reorder does not. Returns false when
/// `after` is not in the column or is the Ticket itself.
pub fn apply_move(
    tickets: &mut [Ticket],
    id: i64,
    status: TicketStatus,
    after: Option<i64>,
) -> bool {
    if after == Some(id) {
        return false;
    }
    let mut order: Vec<i64> = in_column(tickets, status)
        .into_iter()
        .map(|t| t.id)
        .filter(|other| *other != id)
        .collect();
    let index = match after {
        None => 0,
        Some(after) => match order.iter().position(|other| *other == after) {
            Some(index) => index + 1,
            None => return false,
        },
    };
    order.insert(index, id);
    for ticket in tickets.iter_mut() {
        if ticket.id == id && ticket.status != status {
            ticket.status = status;
            ticket.revision += 1;
        }
        if let Some(position) = order.iter().position(|other| *other == ticket.id) {
            ticket.position = position as i64;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::AssigneeKind;

    fn ticket(id: i64, status: TicketStatus, position: i64) -> Ticket {
        Ticket {
            id,
            title: format!("Ticket {id}"),
            description: String::new(),
            status,
            priority: TicketPriority::None,
            labels: vec![],
            assignee_kind: AssigneeKind::Human,
            assignee_id: "owner".into(),
            position,
            revision: 0,
            generation: 0,
            created_at: 0,
            updated_at: 0,
            conversation_id: None,
        }
    }
    fn ids(tickets: &[Ticket], status: TicketStatus) -> Vec<i64> {
        in_column(tickets, status).iter().map(|t| t.id).collect()
    }

    #[test]
    fn optimistic_moves_match_the_daemon_rules() {
        let mut board = vec![
            ticket(1, TicketStatus::ToDo, 0),
            ticket(2, TicketStatus::ToDo, 1),
            ticket(3, TicketStatus::ToDo, 2),
            ticket(4, TicketStatus::Done, 0),
        ];
        assert!(apply_move(&mut board, 1, TicketStatus::ToDo, Some(3)));
        assert_eq!(ids(&board, TicketStatus::ToDo), [2, 3, 1]);
        assert_eq!(board[0].revision, 0, "reordering is not an edit");
        assert!(apply_move(&mut board, 3, TicketStatus::Done, None));
        assert_eq!(ids(&board, TicketStatus::Done), [3, 4]);
        assert_eq!(ids(&board, TicketStatus::ToDo), [2, 1]);
        assert_eq!(board[2].revision, 1);
        assert!(!apply_move(&mut board, 2, TicketStatus::Done, Some(1)));
        assert!(!apply_move(&mut board, 2, TicketStatus::ToDo, Some(2)));
        // Equal positions fall back to newest first, as the daemon orders them.
        let tied = vec![
            ticket(5, TicketStatus::Backlog, 0),
            ticket(6, TicketStatus::Backlog, 0),
        ];
        assert_eq!(ids(&tied, TicketStatus::Backlog), [6, 5]);
    }

    #[test]
    fn filters_combine_search_and_facets() {
        let mut first = ticket(12, TicketStatus::ToDo, 0);
        first.title = "Pay the electricity bill".into();
        first.labels = vec!["Home".into()];
        first.priority = TicketPriority::High;
        let mut second = ticket(13, TicketStatus::ToDo, 1);
        second.description = "Quarterly taxes".into();
        second.assignee_id = "agent-1".into();
        let mut filters = Filters::default();
        assert!(!filters.active());
        filters.query = "pay BILL".into();
        assert!(filters.matches(&first) && !filters.matches(&second));
        filters.query = "t-13".into();
        assert!(filters.matches(&second) && !filters.matches(&first));
        filters.query = "taxes".into();
        assert!(filters.matches(&second));
        filters.query.clear();
        filters.toggle_label("home");
        assert!(filters.matches(&first) && !filters.matches(&second));
        filters.toggle_label("home");
        filters.toggle_assignee("agent-1");
        filters.toggle_priority(TicketPriority::High);
        assert_eq!(filters.facets(), 2);
        assert!(!filters.matches(&first) && !filters.matches(&second));
        filters.toggle_priority(TicketPriority::High);
        assert!(filters.matches(&second));
    }

    #[test]
    fn relations_read_each_link_from_either_side() {
        let links = [
            TicketLink {
                from_id: 1,
                to_id: 2,
                kind: LinkKind::Blocks,
            },
            TicketLink {
                from_id: 3,
                to_id: 1,
                kind: LinkKind::ParentOf,
            },
            TicketLink {
                from_id: 1,
                to_id: 4,
                kind: LinkKind::RelatesTo,
            },
            TicketLink {
                from_id: 5,
                to_id: 1,
                kind: LinkKind::Duplicates,
            },
        ];
        assert_eq!(
            relations(&links, 1),
            [
                (Relation::Parent, 3),
                (Relation::Blocks, 2),
                (Relation::RelatesTo, 4),
                (Relation::DuplicatedBy, 5),
            ]
        );
        assert_eq!(relations(&links, 2), [(Relation::BlockedBy, 1)]);
        assert_eq!(relations(&links, 3), [(Relation::SubIssue, 1)]);
        for relation in Relation::ALL {
            let (from, to, kind) = relation.link(7, 8);
            let stored = [TicketLink {
                from_id: from,
                to_id: to,
                kind,
            }];
            assert_eq!(relations(&stored, 7), [(relation, 8)], "{relation:?}");
        }
        let mut blocker = ticket(1, TicketStatus::InProgress, 0);
        assert_eq!(
            open_blockers(std::slice::from_ref(&blocker), &links, 2),
            [1]
        );
        blocker.status = TicketStatus::Done;
        assert!(open_blockers(&[blocker], &links, 2).is_empty());
    }

    #[test]
    fn vocabulary_round_trips_and_labels_keep_one_tone() {
        for status in STATUSES {
            assert_eq!(status_from_key(status_key(status)), Some(status));
        }
        for priority in PRIORITIES {
            assert_eq!(priority_from_key(priority_key(priority)), Some(priority));
        }
        assert_eq!(label_tone("Home"), label_tone("home"));
        assert_eq!(relative_time(100, 130), "now");
        assert_eq!(relative_time(0, 7200), "2h");
        assert_eq!(ticket_key(42), "T-42");
    }
}
