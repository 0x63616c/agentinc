#![allow(dead_code)] // Driven by the rendered-shell harness only.
//! A believable board for the rendered-shell captures: two agents, Tickets in
//! every column, labels, priorities, relationships, Comments, runs and history.
use super::*;
use crate::storage::{ActivityKind, Comment, LinkKind};
use ainc_client::types::WorkRun;

const HOUR: i64 = 3600;

impl TicketsPage {
    pub(crate) fn fixture_board(&mut self, cx: &mut Context<Self>) -> i64 {
        let store = self.store.clone().expect("fixture store");
        for (name, model) in [
            ("Evee", "connection-default"),
            ("Scout", "connection-default"),
        ] {
            store
                .ticket_command(TicketCommand::RegisterAgent {
                    name: name.into(),
                    instructions: "Plan and execute".into(),
                    model: model.into(),
                })
                .expect("fixture agent");
        }
        let agents: Vec<String> = store
            .tickets()
            .assignees
            .into_iter()
            .filter(|a| a.kind == AssigneeKind::Agent)
            .map(|a| a.id)
            .collect();
        let (evee, scout) = (agents[0].clone(), agents[1].clone());
        let rows: [(&str, TicketStatus, TicketPriority, &[&str], Option<&String>); 13] = [
            (
                "Sell the old bike on Marketplace",
                TicketStatus::Cancelled,
                TicketPriority::None,
                &[],
                None,
            ),
            (
                "Cancel unused streaming subscriptions",
                TicketStatus::Done,
                TicketPriority::Low,
                &["Money"],
                None,
            ),
            (
                "Set up smart plugs in the living room",
                TicketStatus::Done,
                TicketPriority::Medium,
                &["Home"],
                None,
            ),
            (
                "File the home office tax deduction",
                TicketStatus::Blocked,
                TicketPriority::High,
                &["Money", "Admin"],
                None,
            ),
            (
                "Migrate the photo library to the NAS",
                TicketStatus::InProgress,
                TicketPriority::Medium,
                &["Home"],
                Some(&scout),
            ),
            (
                "Reconcile September budget and receipts",
                TicketStatus::InProgress,
                TicketPriority::High,
                &["Money"],
                Some(&evee),
            ),
            (
                "Book a dentist cleaning",
                TicketStatus::ToDo,
                TicketPriority::Low,
                &["Health"],
                None,
            ),
            (
                "Draft the Q4 savings plan",
                TicketStatus::ToDo,
                TicketPriority::High,
                &["Money"],
                Some(&evee),
            ),
            (
                "Renew the car registration before Oct 15",
                TicketStatus::ToDo,
                TicketPriority::Urgent,
                &["Admin"],
                None,
            ),
            (
                "Book a campsite at Kirk Creek",
                TicketStatus::Backlog,
                TicketPriority::None,
                &["Travel"],
                None,
            ),
            (
                "Research standing desks under $600",
                TicketStatus::Backlog,
                TicketPriority::Low,
                &["Home"],
                None,
            ),
            (
                "Plan the October weekend in Big Sur",
                TicketStatus::Backlog,
                TicketPriority::Medium,
                &["Travel"],
                None,
            ),
            (
                "Compare home insurance renewal quotes",
                TicketStatus::Backlog,
                TicketPriority::None,
                &[],
                None,
            ),
        ];
        let mut ids = Vec::new();
        for (title, status, priority, labels, assignee) in rows {
            let id = store
                .ticket_command(TicketCommand::CreateDetailed {
                    title: title.into(),
                    description: None,
                    status: Some(status),
                    priority: Some(priority),
                    labels: labels.iter().map(|l| (*l).to_owned()).collect(),
                    assignee_id: assignee.cloned(),
                })
                .expect("fixture Ticket")
                .expect("Ticket id");
            ids.push(id);
        }
        let id = |title: &str| {
            ids[rows
                .iter()
                .position(|row| row.0 == title)
                .expect("fixture title")]
        };
        let budget = id("Reconcile September budget and receipts");
        let taxes = id("File the home office tax deduction");
        let trip = id("Plan the October weekend in Big Sur");
        let campsite = id("Book a campsite at Kirk Creek");
        let savings = id("Draft the Q4 savings plan");
        let streaming = id("Cancel unused streaming subscriptions");
        for (from_id, to_id, link) in [
            (budget, taxes, LinkKind::Blocks),
            (trip, campsite, LinkKind::ParentOf),
            (savings, streaming, LinkKind::RelatesTo),
        ] {
            store
                .ticket_command(TicketCommand::Link {
                    from_id,
                    to_id,
                    link,
                })
                .expect("fixture link");
        }
        store
            .ticket_command(TicketCommand::Describe {
                id: budget,
                revision: 0,
                description: "Pull the September statements from both checking accounts and the card, match every receipt, and flag anything over $200 without one.\n\nSummarize the totals by category in a Comment when done.".into(),
            })
            .expect("fixture description");
        let now = list::now();
        store.fixture_edit(|snapshot, history| {
            for (age, ticket) in snapshot.tickets.iter_mut().rev().enumerate() {
                ticket.created_at = now - (age as i64 + 2) * 9 * HOUR;
                ticket.updated_at = now - age as i64 * 2 * HOUR - 300;
            }
            for entry in history.iter_mut() {
                entry.created_at = now - 30 * HOUR;
            }
            if let Some(ticket) = snapshot.tickets.iter_mut().find(|t| t.id == budget) {
                ticket.generation = 1;
            }
            snapshot.runs.push(WorkRun {
                run_id: "8a37e3d2-6a42-4918-a5d2-98fc38ea2274".into(),
                ticket_id: budget,
                generation: 1,
                state: "running".into(),
                error: None,
            });
            let mut next = history.len() as i64;
            let mut push = |kind, actor: &str, from: Option<&str>, to: Option<&str>, age: i64| {
                next += 1;
                history.push(TicketActivity {
                    id: next,
                    ticket_id: budget,
                    actor_id: actor.into(),
                    kind,
                    from_value: from.map(str::to_owned),
                    to_value: to.map(str::to_owned),
                    conversation_id: None,
                    run_id: None,
                    created_at: now - age,
                });
            };
            push(ActivityKind::Assigned, "owner", Some("owner"), Some(&evee), 26 * HOUR);
            push(ActivityKind::Work, "owner", None, Some("queued"), 26 * HOUR);
            push(ActivityKind::Status, &evee, Some("to_do"), Some("in_progress"), 26 * HOUR - 20);
            let comment_id = snapshot.comments.len() as i64;
            snapshot.comments.extend([
                Comment {
                    id: comment_id + 1,
                    ticket_id: budget,
                    author_id: evee.clone(),
                    body: "Pulled four statements. Two receipts are missing: Costco on Sep 12 and REI on Sep 21.".into(),
                    created_at: now - 3 * HOUR,
                },
                Comment {
                    id: comment_id + 2,
                    ticket_id: budget,
                    author_id: "owner".into(),
                    body: "The REI receipt is in the order email from Sep 21.".into(),
                    created_at: now - 2 * HOUR,
                },
            ]);
        });
        self.reload();
        self.loaded = true;
        cx.notify();
        budget
    }
    pub(crate) fn fixture_view(&mut self, list: bool, cx: &mut Context<Self>) {
        self.view = if list { View::List } else { View::Board };
        self.selected = None;
        cx.notify();
    }
    pub(crate) fn fixture_menu(&mut self, menu: Option<Menu>, cx: &mut Context<Self>) {
        self.menu = menu;
        cx.notify();
    }
    pub(crate) fn fixture_ticket_id(&self, title: &str) -> Option<i64> {
        self.state
            .tickets
            .iter()
            .find(|t| t.title == title)
            .map(|t| t.id)
    }
    pub(crate) fn fixture_status(&self, id: i64) -> Option<TicketStatus> {
        self.ticket(id).map(|t| t.status)
    }
    pub(crate) fn fixture_column(&self, status: TicketStatus) -> Vec<i64> {
        in_column(&self.state.tickets, status)
            .iter()
            .map(|t| t.id)
            .collect()
    }
    pub(crate) fn fixture_link(
        &mut self,
        id: i64,
        target: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.link_relation = Relation::BlockedBy;
        self.link_target = Some(target);
        let focus = self.link_search.focus_handle(cx);
        self.overlays
            .borrow_mut()
            .open(Overlay::LinkTicket(id), window, cx, Some(focus));
        cx.notify();
    }
    pub(crate) fn fixture_filter_label(&mut self, label: &str, cx: &mut Context<Self>) {
        self.filters.toggle_label(label);
        cx.notify();
    }
}
