//! Tickets: a Kanban board and a grouped list over the same filtered Tickets,
//! one Ticket's detail, and the dialogs that create, rename and relate them.
#[path = "tickets/board.rs"]
mod board;
#[path = "tickets/detail.rs"]
mod detail;
#[path = "tickets/dialogs.rs"]
mod dialogs;
#[cfg(all(test, feature = "rendered-tests"))]
#[path = "tickets/fixture.rs"]
mod fixture;
#[path = "tickets/labels.rs"]
mod labels;
#[path = "tickets/list.rs"]
mod list;
#[path = "tickets/model.rs"]
pub(crate) mod model;
#[path = "tickets/parts.rs"]
mod parts;

use crate::{
    action::{Pending, Run},
    daemon::Daemon,
    input::{Submit, TextInput},
    overlay::Overlay,
    page::{Drafts, Page, PageOverlays},
    routes::{Destination, Route},
    sync::{SliceChanged, Sync},
    ui::*,
};
use ainc_client::types::{
    Assignee, AssigneeKind, Ticket, TicketActivity, TicketCommand, TicketPriority, TicketSnapshot,
    TicketStatus,
};
use gpui::{prelude::*, *};
use model::*;
use parts::*;
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};

/// How the Tickets page lays out the filtered Tickets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Board,
    List,
}

/// The one dialog this page can have open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialog {
    Add,
    Delete(i64),
    Rename(i64),
    Link(i64),
}

/// What a Ticket's history depends on: its edits, Comments and runs. Polling
/// reads the history again only when this changes.
type ActivityStamp = (i64, i64, usize, Vec<String>);

/// The fields of the create dialog that are not text inputs.
#[derive(Clone, Debug)]
struct Draft {
    status: TicketStatus,
    priority: TicketPriority,
    assignee: Option<String>,
    labels: Vec<String>,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            status: TicketStatus::ToDo,
            priority: TicketPriority::None,
            assignee: None,
            labels: vec![],
        }
    }
}

pub struct TicketsPage {
    daemon: Arc<Daemon>,
    sync: Entity<Sync>,
    overlays: PageOverlays<Dialog>,
    state: TicketSnapshot,
    view: View,
    selected: Option<i64>,
    filters: Filters,
    owner: (SharedString, Option<Arc<Image>>),
    search: Entity<TextInput>,
    input: Entity<TextInput>,
    draft_description: Entity<TextInput>,
    draft: Draft,
    comment: Entity<TextInput>,
    description: Entity<TextInput>,
    editing_description: bool,
    rename: Entity<TextInput>,
    label_input: Entity<TextInput>,
    link_search: Entity<TextInput>,
    link_relation: Relation,
    link_target: Option<i64>,
    activity: Vec<TicketActivity>,
    /// Conversation titles by id, for the "From" row; read with the snapshot.
    conversation_titles: Vec<(i64, String)>,
    /// The open Ticket's history was read at this stamp.
    activity_for: Option<ActivityStamp>,
    drag: board::DragState,
    form_error: Option<String>,
    pending: Pending,
    page_focus: FocusHandle,
    restore_focus: bool,
    add_focus: FocusHandle,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Destination> for TicketsPage {}

impl TicketsPage {
    /// Every text the person may be typing on this page, by a stable key.
    fn inputs(&self) -> [(&'static str, &Entity<TextInput>); 8] {
        [
            ("input", &self.input),
            ("draft_description", &self.draft_description),
            ("comment", &self.comment),
            ("description", &self.description),
            ("rename", &self.rename),
            ("label", &self.label_input),
            ("link_search", &self.link_search),
            ("search", &self.search),
        ]
    }
    pub fn new(
        daemon: Arc<Daemon>,
        sync: Entity<Sync>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let overlays = PageOverlays::new(overlays, Route::Tickets);
        let field = |placeholder: &str, id: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| TextInput::field(placeholder, false, cx).identified(id))
        };
        let input = field("Ticket title", "tickets.title", cx);
        let search = field("Search Tickets", "tickets.search", cx);
        let draft_description = field("Description", "tickets.draft.description", cx);
        let comment = field("Comment", "tickets.comment", cx);
        let description = field("Description", "tickets.description", cx);
        let rename = field("Ticket title", "tickets.rename", cx);
        let label_input = field("Label", "tickets.label", cx);
        let link_search = field("Search Tickets", "tickets.link.search", cx);
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.create(cx)),
            cx.observe(&input, |this, input, cx| {
                this.form_error = Self::title_error(&input.read(cx).content).map(str::to_owned);
                cx.notify();
            }),
            cx.observe(&search, |this, search, cx| {
                this.filters.query = search.read(cx).content.to_string();
                cx.notify();
            }),
            cx.subscribe(&comment, |this, _, _: &Submit, cx| this.add_comment(cx)),
            cx.observe(&comment, |_, _, cx| cx.notify()),
            cx.observe(&description, |_, _, cx| cx.notify()),
            cx.subscribe(&rename, |this, _, _: &Submit, cx| this.rename_ticket(cx)),
            cx.observe(&rename, |_, _, cx| cx.notify()),
            cx.subscribe(&label_input, |this, _, _: &Submit, cx| {
                let label = this.label_input.read(cx).content.trim().to_owned();
                this.toggle_label(label, cx);
            }),
            cx.observe(&label_input, |_, _, cx| cx.notify()),
            cx.observe(&link_search, |this, _, cx| {
                this.link_target = None;
                cx.notify();
            }),
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, event: &SliceChanged, cx| {
                if matches!(*event, SliceChanged::Tickets | SliceChanged::Product) {
                    this.reload();
                    this.load_activity(cx);
                    cx.notify();
                }
            }),
        ];
        let mut this = Self {
            daemon,
            sync,
            overlays,
            state: TicketSnapshot {
                tickets: vec![],
                comments: vec![],
                assignees: vec![],
                runs: vec![],
                links: vec![],
            },
            view: View::Board,
            selected: None,
            filters: Filters::default(),
            owner: ("You".into(), None),
            search,
            input,
            draft_description,
            draft: Draft::default(),
            comment,
            description,
            editing_description: false,
            rename,
            label_input,
            link_search,
            link_relation: Relation::BlockedBy,
            link_target: None,
            activity: vec![],
            conversation_titles: vec![],
            activity_for: None,
            drag: board::DragState::default(),
            form_error: None,
            pending: Pending::default(),
            page_focus: cx.focus_handle(),
            restore_focus: false,
            add_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        this.reload();
        this
    }
    /// The local person the `owner` principal stands for.
    pub fn set_owner(&mut self, name: &str, photo: Option<Arc<Image>>, cx: &mut Context<Self>) {
        self.owner = (name.to_owned().into(), photo);
        cx.notify();
    }
    pub(crate) fn reload(&mut self) {
        self.state = (*self.daemon.tickets()).clone();
        self.conversation_titles = self
            .daemon
            .product()
            .conversations
            .into_iter()
            .map(|c| (c.id, c.title))
            .collect();
        if self
            .selected
            .is_some_and(|id| !self.state.tickets.iter().any(|t| t.id == id))
        {
            self.selected = None;
        }
    }
    fn activity_stamp(&self) -> Option<ActivityStamp> {
        let ticket = self.selected.and_then(|id| self.ticket(id))?;
        let runs = self
            .state
            .runs
            .iter()
            .filter(|run| run.ticket_id == ticket.id)
            .map(|run| format!("{}/{}", run.run_id, run.state))
            .collect();
        Some((
            ticket.id,
            ticket.updated_at,
            self.comment_count(ticket.id),
            runs,
        ))
    }
    /// Read the open Ticket's history in the background, unless nothing it
    /// depends on has changed since the last read.
    fn load_activity(&mut self, cx: &mut Context<Self>) {
        let Some(stamp) = self.activity_stamp() else {
            return;
        };
        let daemon = self.daemon.clone();
        if self.activity_for.as_ref() == Some(&stamp) {
            return;
        }
        let id = stamp.0;
        // Reads of different Tickets may overlap; the stamp keeps them straight.
        cx.run(
            &Pending::default(),
            move || Ok(daemon.fetch(crate::daemon::Activity(id))?),
            move |this, result, _| {
                if this.selected == Some(id)
                    && let Ok(activity) = result
                {
                    this.activity = activity;
                    this.activity_for = Some(stamp);
                }
            },
        );
    }
    pub fn select(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.selected != Some(id) {
            self.activity.clear();
            self.activity_for = None;
            self.editing_description = false;
        }
        self.selected = Some(id);
        self.overlays.close_popover();
        self.load_activity(cx);
        cx.notify();
    }
    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.overlays.close_popover();
        self.editing_description = false;
        cx.notify();
    }
    pub(crate) fn ticket(&self, id: i64) -> Option<&Ticket> {
        self.state.tickets.iter().find(|t| t.id == id)
    }
    /// Tickets the toolbar's search and filters let through.
    fn visible(&self) -> Vec<Ticket> {
        self.state
            .tickets
            .iter()
            .filter(|ticket| self.filters.matches(ticket))
            .cloned()
            .collect()
    }
    fn title_error(title: &str) -> Option<&'static str> {
        (title.trim().chars().count() > 500).then_some("Use 500 characters or fewer.")
    }
    /// Open the select or menu with this id, or close it when it is the open one.
    fn toggle_menu(&mut self, id: &'static str, cx: &mut Context<Self>) {
        self.overlays.toggle_popover(id);
        if id == "tickets.labels" {
            self.label_input.update(cx, |input, _| input.reset());
        }
        cx.notify();
    }

    /// Open the create dialog, starting in `status` when a board column asked.
    pub fn open_create(
        &mut self,
        status: Option<TicketStatus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.form_error = None;
        self.overlays.close_popover();
        self.draft = Draft {
            status: status.unwrap_or(TicketStatus::ToDo),
            ..Draft::default()
        };
        self.label_input.update(cx, |input, _| input.reset());
        for input in [&self.input, &self.draft_description] {
            input.update(cx, |input, cx| {
                input.reset();
                cx.notify();
            });
        }
        let focus = self.input.focus_handle(cx);
        self.overlays.open_dialog(Dialog::Add, focus, window, cx);
        cx.notify();
    }
    fn create(&mut self, cx: &mut Context<Self>) {
        if self.overlays.active() != Some(Dialog::Add) {
            return;
        }
        let title = self.input.read(cx).content.trim().to_owned();
        if title.is_empty() {
            return;
        }
        if let Some(error) = Self::title_error(&title) {
            self.form_error = Some(error.into());
            cx.notify();
            return;
        }
        let description = self.draft_description.read(cx).content.trim().to_owned();
        let labels = self.draft.labels.clone();
        let draft = self.draft.clone();
        self.command(
            TicketCommand::CreateDetailed {
                title,
                description: (!description.is_empty()).then_some(description),
                status: Some(draft.status),
                priority: Some(draft.priority),
                labels,
                assignee_id: draft.assignee,
            },
            cx,
        );
    }
    fn add_comment(&mut self, cx: &mut Context<Self>) {
        let Some(ticket_id) = self.selected else {
            return;
        };
        let body = self.comment.read(cx).content.trim().to_owned();
        if body.is_empty() {
            return;
        }
        self.command(TicketCommand::AddComment { ticket_id, body }, cx);
    }
    fn rename_ticket(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Rename(id)) = self.overlays.active() else {
            return;
        };
        let title = self.rename.read(cx).content.trim().to_owned();
        let Some(ticket) = self.ticket(id) else {
            return;
        };
        if title.is_empty() || Self::title_error(&title).is_some() {
            return;
        }
        let revision = ticket.revision;
        self.command(
            TicketCommand::Rename {
                id,
                revision,
                title,
            },
            cx,
        );
    }
    fn command(&mut self, command: TicketCommand, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        self.mutate(move || Ok(daemon.send(command).map(|_| ())?), cx);
    }
    fn mutate(
        &mut self,
        operation: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.run(&self.pending.clone(), operation, |this, result, cx| {
            this.restore_focus = true;
            // The Daemon holds the daemon's answer either way; an optimistic
            // board move that was refused snaps back here.
            this.reload();
            match result {
                Ok(()) => {
                    this.overlays.close();
                    this.form_error = None;
                    this.editing_description = false;
                    this.comment.update(cx, |input, cx| {
                        input.reset();
                        cx.notify();
                    });
                    this.load_activity(cx);
                }
                Err(failure) => this.form_error = Some(failure.message("The Ticket change")),
            }
        });
    }

    // Shared pieces for the board, list and detail.

    fn assignee(&self, id: &str) -> Option<&Assignee> {
        self.state.assignees.iter().find(|a| a.id == id)
    }
    /// The person or agent's display name; the owner is the local person.
    fn assignee_name(&self, id: &str) -> SharedString {
        if id == "owner" {
            return self.owner.0.clone();
        }
        self.assignee(id)
            .map_or_else(|| "Agent".into(), |a| a.name.clone().into())
    }
    fn assignee_face(&self, id: &str) -> AssigneeFace {
        let name = self.assignee_name(id);
        if self.is_agent(id) {
            AssigneeFace::agent(name)
        } else {
            AssigneeFace::person(
                name,
                (id == "owner").then(|| self.owner.1.clone()).flatten(),
            )
        }
    }
    fn assignee_option(&self, id: &str) -> SelectOption {
        SelectOption::new(self.assignee_name(id)).avatar(self.assignee_face(id))
    }
    fn is_agent(&self, id: &str) -> bool {
        self.assignee(id)
            .is_some_and(|a| a.kind == AssigneeKind::Agent)
    }
    fn running(&self, ticket: &Ticket) -> bool {
        self.state.runs.iter().any(|run| {
            run.ticket_id == ticket.id
                && run.generation == ticket.generation
                && is_active(&run.state)
        })
    }
    fn comment_count(&self, id: i64) -> usize {
        self.state
            .comments
            .iter()
            .filter(|c| c.ticket_id == id)
            .count()
    }
    /// Open blockers read the same on a board card and a list row: a red
    /// glyph, then the blockers' keys in quiet text so the title leads.
    fn blockers_marker(&self, blockers: &[i64]) -> Div {
        let keys = blockers
            .iter()
            .map(|id| ticket_key(*id))
            .collect::<Vec<_>>()
            .join(", ");
        row()
            .gap(px(SPACE_1))
            .text_size(type_size(CAPTION_SIZE))
            .text_color(rgb(TEXT_SECONDARY))
            .child(icon(Icon::StatusBlocked, ICON_SIZE_XS).text_color(rgb(STATUS_RED)))
            .child(keys)
    }
    /// The header's icon-only Refresh: every page that polls the daemon has one.
    fn refresh_button(&self, ui: &mut Ui<Self>) -> Stateful<Div> {
        let fetching = self.sync.read(ui.cx).fetching();
        Button::new("tickets.refresh", "Refresh")
            .icon(Icon::Refresh)
            .icon_only()
            .secondary()
            .enabled(!fetching)
            .build(ui, |this: &mut Self, _, cx| {
                this.sync.update(cx, |sync, cx| sync.wake(cx))
            })
    }

    /// A filter's menu button; the page decides which menu is open.
    #[allow(clippy::too_many_arguments)]
    fn dropdown(
        &self,
        id: &'static str,
        label: SharedString,
        icon_name: Icon,
        active: bool,
        width: f32,
        items: Vec<AnyElement>,
        ui: &mut Ui<Self>,
    ) -> Div {
        MenuButton::new(id, label)
            .icon(icon_name)
            .active(active)
            .open(self.overlays.popover_open(id))
            .width(width)
            .build(ui, items, move |this: &mut Self, _, cx| {
                this.toggle_menu(id, cx)
            })
    }

    fn filter_bar(&self, ui: &mut Ui<Self>) -> Div {
        let tickets = &self.state.tickets;
        let tally = |n: usize| hint(n.to_string());
        let priority_items = PRIORITIES
            .into_iter()
            .map(|priority| {
                MenuEntry::new(
                    SharedString::from(format!(
                        "tickets.filter.priority.{}",
                        priority_key(priority)
                    )),
                    priority_name(priority),
                )
                .glyph(priority_icon(priority), priority_color(priority))
                .trailing(tally(
                    tickets.iter().filter(|t| t.priority == priority).count(),
                ))
                .checked(self.filters.priorities.contains(&priority))
                .build(ui, move |this: &mut Self, _, cx| {
                    this.filters.toggle_priority(priority);
                    cx.notify();
                })
                .into_any_element()
            })
            .collect();
        let labels = all_labels(&self.state.tickets);
        let label_items = if labels.is_empty() {
            vec![menu_label("No labels yet").into_any_element()]
        } else {
            labels
                .into_iter()
                .map(|label| {
                    let checked = self
                        .filters
                        .labels
                        .iter()
                        .any(|chosen| chosen.eq_ignore_ascii_case(&label));
                    MenuEntry::new(
                        SharedString::from(format!("tickets.filter.label.{label}")),
                        label.clone(),
                    )
                    .glyph(Icon::Dot, label_color(&label))
                    .trailing(tally(
                        tickets
                            .iter()
                            .filter(|t| t.labels.iter().any(|l| l.eq_ignore_ascii_case(&label)))
                            .count(),
                    ))
                    .checked(checked)
                    .build(ui, move |this: &mut Self, _, cx| {
                        this.filters.toggle_label(&label);
                        cx.notify();
                    })
                    .into_any_element()
                })
                .collect()
        };
        let assignee_items = self
            .state
            .assignees
            .iter()
            .map(|assignee| {
                let id = assignee.id.clone();
                MenuEntry::new(
                    SharedString::from(format!("tickets.filter.assignee.{id}")),
                    self.assignee_name(&id),
                )
                .icon(if assignee.kind == AssigneeKind::Agent {
                    Icon::Agents
                } else {
                    Icon::User
                })
                .trailing(tally(
                    tickets.iter().filter(|t| t.assignee_id == id).count(),
                ))
                .checked(self.filters.assignees.contains(&id))
                .build(ui, move |this: &mut Self, _, cx| {
                    this.filters.toggle_assignee(&id);
                    cx.notify();
                })
                .into_any_element()
            })
            .collect();
        let count = |label: &str, n: usize| -> SharedString {
            if n == 0 {
                label.to_owned().into()
            } else {
                format!("{label} · {n}").into()
            }
        };
        row()
            .w_full()
            .gap(px(CONTROL_GAP))
            .child(
                div().w(px(TOOLBAR_SEARCH_WIDTH)).child(
                    Field::new(self.search.clone())
                        .leading_icon(Icon::Search)
                        .selector("tickets.search")
                        .build(ui),
                ),
            )
            .child(self.dropdown(
                "tickets.filter.priority",
                count("Priority", self.filters.priorities.len()),
                Icon::Filter,
                !self.filters.priorities.is_empty(),
                MENU_WIDTH,
                priority_items,
                ui,
            ))
            .child(self.dropdown(
                "tickets.filter.label",
                count("Labels", self.filters.labels.len()),
                Icon::Tag,
                !self.filters.labels.is_empty(),
                MENU_WIDTH,
                label_items,
                ui,
            ))
            .child(self.dropdown(
                "tickets.filter.assignee",
                count("Assignee", self.filters.assignees.len()),
                Icon::User,
                !self.filters.assignees.is_empty(),
                MENU_WIDTH,
                assignee_items,
                ui,
            ))
            .when(self.filters.active(), |s| {
                s.child(
                    Button::new("tickets.filter.clear", "Clear")
                        .ghost()
                        .tint(TEXT_SECONDARY)
                        .build(ui, |this: &mut Self, _, cx| {
                            this.filters = Filters::default();
                            this.search.update(cx, |input, cx| {
                                input.reset();
                                cx.notify();
                            });
                            cx.notify();
                        }),
                )
            })
            .child(div().flex_1())
            .child(
                div().flex_shrink_0().child(
                    Segmented::new("tickets.view", ["Board", "List"])
                        .selected(usize::from(self.view == View::List))
                        .build(ui, |this: &mut Self, index, _, cx| {
                            this.view = if index == 0 { View::Board } else { View::List };
                            this.overlays.close_popover();
                            cx.notify();
                        }),
                ),
            )
    }

    fn summary(&self) -> String {
        if self.filters.active() {
            let shown = self.visible().len();
            return format!("Showing {shown} of {} Tickets", self.state.tickets.len());
        }
        let count = |status| {
            self.state
                .tickets
                .iter()
                .filter(|t| t.status == status)
                .count()
        };
        let open = self
            .state
            .tickets
            .iter()
            .filter(|t| !matches!(t.status, TicketStatus::Done | TicketStatus::Cancelled))
            .count();
        let mut parts = vec![format!("{open} open")];
        for (status, name) in [
            (TicketStatus::InProgress, "in progress"),
            (TicketStatus::Blocked, "blocked"),
        ] {
            let n = count(status);
            if n > 0 {
                parts.push(format!("{n} {name}"));
            }
        }
        parts.join(" · ")
    }

    fn overview(&mut self, ui: &mut Ui<Self>) -> Stateful<Div> {
        let header = PageHeader::new(self.title())
            .description(self.summary())
            .actions(
                row_gap(CONTROL_GAP).child(self.refresh_button(ui)).child(
                    Button::new("tickets.create", "New Ticket")
                        .primary()
                        .icon(Icon::Plus)
                        .enabled(!self.pending.busy())
                        .track_focus(&self.add_focus)
                        .build(ui, |this: &mut Self, window, cx| {
                            this.open_create(None, window, cx)
                        })
                        .debug_selector(|| "tickets.create".into()),
                ),
            );
        let sync = self.sync.read(ui.cx);
        let state = sync.load_state();
        let notices: Vec<String> = self
            .form_error
            .iter()
            .filter(|_| self.overlays.active().is_none())
            .cloned()
            .collect();
        let visible = self.visible();
        let body = |ui: &mut Ui<Self>| {
            if self.view == View::Board {
                self.board(&visible, ui).into_any_element()
            } else if self.state.tickets.is_empty() {
                EmptyState::new(Icon::Tasks, "No Tickets yet.")
                    .description("Create a Ticket and assign it to an agent to start work.")
                    .selector("tickets.empty")
                    .action(
                        Button::new("tickets.create.empty", "New Ticket")
                            .secondary()
                            .icon(Icon::Plus)
                            .enabled(!self.pending.busy())
                            .build(ui, |this: &mut Self, window, cx| {
                                this.open_create(None, window, cx)
                            }),
                    )
                    .build()
                    .into_any_element()
            } else {
                self.list(&visible, ui).into_any_element()
            }
        };
        let page = match self.view {
            View::Board => PageFrame::fill(header),
            View::List => PageFrame::document(header),
        };
        page.child(
            column()
                .id("tickets-page")
                .track_focus(&self.page_focus)
                .w_full()
                .when(self.view == View::Board, |s| s.flex_1().min_h_0())
                .gap(px(SPACE_4))
                .child(self.filter_bar(ui))
                .children(
                    notices
                        .into_iter()
                        .map(|notice| banner(Tone::Danger, notice)),
                )
                .children(page_frame("tickets", &state, SKELETON_ROWS, ui, body)),
        )
        .build()
    }
}

impl Render for TicketsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drag.settle(window, cx);
        if self.restore_focus && self.overlays.active().is_none() {
            self.restore_focus = false;
            window.focus(&self.page_focus, cx);
        }
        let ui = &mut Ui::new(window, cx);
        match self.selected.and_then(|id| self.ticket(id)).cloned() {
            Some(ticket) => self.detail(&ticket, ui).into_any_element(),
            None => self.overview(ui).into_any_element(),
        }
    }
}

impl Page for TicketsPage {
    const ROUTE: Route = Route::Tickets;
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.dialog(&mut Ui::new(window, cx))
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        self.dialog_focus(cx)
    }
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        anyhow::ensure!(
            !self.pending.busy(),
            "Wait for the current change to finish before installing"
        );
        let mut drafts = Drafts::default();
        drafts.set("selected", self.selected);
        drafts.set("list", self.view == View::List);
        drafts.set("editing_description", self.editing_description);
        for (key, input) in self.inputs() {
            drafts.text(key, input, cx);
        }
        Ok(drafts)
    }
    fn restore(&mut self, drafts: Drafts, cx: &mut Context<Self>) {
        if let Some(selected) = drafts.get::<Option<i64>>("selected") {
            self.selected = selected;
        }
        if drafts.get::<bool>("list") == Some(true) {
            self.view = View::List;
        }
        self.editing_description = drafts.get::<bool>("editing_description") == Some(true);
        for (key, input) in self.inputs() {
            drafts.restore_text(key, input, cx);
        }
    }
    fn open(&mut self, to: &Destination, window: &mut Window, cx: &mut Context<Self>) {
        match to {
            Destination::Ticket(id) => self.select(*id, cx),
            Destination::NewTicket => self.open_create(None, window, cx),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TicketsPage;
    #[test]
    fn validates_titles() {
        assert_eq!(
            TicketsPage::title_error(&"A".repeat(501)),
            Some("Use 500 characters or fewer.")
        );
        assert_eq!(TicketsPage::title_error(&"A".repeat(500)), None);
    }
}
