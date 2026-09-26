//! Tickets: a Kanban board and a grouped list over the same filtered Tickets,
//! one Ticket's detail, and the dialogs that create, rename and relate them.
//! The Agents page shares this entity because it reads the same snapshot.
#[path = "tickets/agents.rs"]
mod agents;
#[path = "tickets/board.rs"]
mod board;
#[path = "tickets/detail.rs"]
mod detail;
#[path = "tickets/dialogs.rs"]
mod dialogs;
#[cfg(all(test, feature = "rendered-tests"))]
#[path = "tickets/fixture.rs"]
mod fixture;
#[path = "tickets/list.rs"]
mod list;
#[path = "tickets/model.rs"]
pub(crate) mod model;

use crate::{
    input::{Submit, TextInput},
    model::Overlay,
    storage::{
        Assignee, AssigneeKind, Store, Ticket, TicketActivity, TicketCommand, TicketPriority,
        TicketSnapshot, TicketStatus,
    },
    ui::*,
};
use gpui::{prelude::*, *};
use model::*;
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};

/// How the Tickets page lays out the filtered Tickets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Board,
    List,
}

/// The one floating menu open on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Menu {
    FilterPriority,
    FilterLabel,
    FilterAssignee,
    Status,
    Priority,
    Assignee,
    Labels,
    DraftStatus,
    DraftPriority,
    DraftAssignee,
    Relation,
}

/// Where the shell should take the person next.
pub enum TicketsEvent {
    /// The durable runs behind a Ticket's work.
    OpenRuns,
    /// The Conversation a Ticket came from.
    OpenConversation(i64),
}

/// The fields of the create dialog that are not text inputs.
#[derive(Clone, Debug)]
struct Draft {
    status: TicketStatus,
    priority: TicketPriority,
    assignee: Option<String>,
    /// Existing labels chosen as chips; new ones are typed.
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
    store: Option<Arc<Store>>,
    overlays: Rc<RefCell<OverlayHost<Overlay>>>,
    state: TicketSnapshot,
    view: View,
    selected: Option<i64>,
    filters: Filters,
    menu: Option<Menu>,
    owner: (SharedString, Option<Arc<Image>>),
    search: Entity<TextInput>,
    input: Entity<TextInput>,
    draft_description: Entity<TextInput>,
    draft_labels: Entity<TextInput>,
    draft: Draft,
    comment: Entity<TextInput>,
    description: Entity<TextInput>,
    editing_description: bool,
    rename: Entity<TextInput>,
    label_input: Entity<TextInput>,
    link_search: Entity<TextInput>,
    link_relation: Relation,
    link_target: Option<i64>,
    agent_name: Entity<TextInput>,
    agent_instructions: Entity<TextInput>,
    agent_model: Entity<TextInput>,
    activity: Vec<TicketActivity>,
    activity_for: Option<i64>,
    drag: board::DragState,
    error: Option<String>,
    form_error: Option<String>,
    pending: bool,
    refreshing: bool,
    loaded: bool,
    loading_started: Instant,
    page_focus: FocusHandle,
    restore_focus: bool,
    add_focus: FocusHandle,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl HoverHost for TicketsPage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}
impl EventEmitter<TicketsEvent> for TicketsPage {}

impl TicketsPage {
    pub(crate) fn update_drafts(&self, cx: &App) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(
            !self.pending,
            "Wait for the current change to finish before installing"
        );
        Ok(serde_json::json!({
            "selected": self.selected,
            "list": self.view == View::List,
            "input": self.input.read(cx).content.to_string(),
            "comment": self.comment.read(cx).content.to_string(),
            "search": self.search.read(cx).content.to_string(),
            "agent_name": self.agent_name.read(cx).content.to_string(),
            "agent_instructions": self.agent_instructions.read(cx).content.to_string(),
            "agent_model": self.agent_model.read(cx).content.to_string(),
        }))
    }
    pub(crate) fn restore_update_drafts(
        &mut self,
        value: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if let Ok(value) = serde_json::from_value(value["selected"].clone()) {
            self.selected = value;
        }
        if value["list"].as_bool() == Some(true) {
            self.view = View::List;
        }
        for (key, input) in [
            ("input", &self.input),
            ("comment", &self.comment),
            ("search", &self.search),
            ("agent_name", &self.agent_name),
            ("agent_instructions", &self.agent_instructions),
            ("agent_model", &self.agent_model),
        ] {
            if let Some(text) = value[key].as_str() {
                input.update(cx, |input, cx| input.set_text(text, cx));
            }
        }
    }

    pub fn new(
        store: Option<Arc<Store>>,
        storage_error: Option<String>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = |placeholder: &str, id: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| TextInput::field(placeholder, false, cx).identified(id))
        };
        let input = field("What needs doing?", "tickets.title", cx);
        let search = field("Search Tickets", "tickets.search", cx);
        let draft_description = field("Add details (optional)", "tickets.draft.description", cx);
        let draft_labels = field("Home, Money", "tickets.draft.labels", cx);
        let comment = field("Add a Comment…", "tickets.comment", cx);
        let description = field("Describe the work", "tickets.description", cx);
        let rename = field("Ticket title", "tickets.rename", cx);
        let label_input = field("Add or find a label", "tickets.label", cx);
        let link_search = field("Find a Ticket by title or ID", "tickets.link.search", cx);
        let agent_name = field("Name", "agents.name", cx);
        let agent_instructions = field("Instructions", "agents.instructions", cx);
        let agent_model = field("Connection default", "agents.model", cx);
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
                this.add_label(label, cx);
            }),
            cx.observe(&label_input, |_, _, cx| cx.notify()),
            cx.observe(&link_search, |this, _, cx| {
                this.link_target = None;
                cx.notify();
            }),
            cx.observe(&agent_name, |_, _, cx| cx.notify()),
        ];
        let mut this = Self {
            store,
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
            menu: None,
            owner: ("You".into(), None),
            search,
            input,
            draft_description,
            draft_labels,
            draft: Draft::default(),
            comment,
            description,
            editing_description: false,
            rename,
            label_input,
            link_search,
            link_relation: Relation::BlockedBy,
            link_target: None,
            agent_name,
            agent_instructions,
            agent_model,
            activity: vec![],
            activity_for: None,
            drag: board::DragState::default(),
            error: storage_error,
            form_error: None,
            pending: false,
            refreshing: false,
            loaded: cfg!(test),
            loading_started: Instant::now(),
            page_focus: cx.focus_handle(),
            restore_focus: false,
            add_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            hover: HoverFade::default(),
            _subscriptions: subscriptions,
        };
        this.reload();
        #[cfg(not(test))]
        {
            this.refresh(cx);
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        this
    }
    /// The local person the `owner` principal stands for.
    pub fn set_owner(&mut self, name: &str, photo: Option<Arc<Image>>, cx: &mut Context<Self>) {
        self.owner = (name.to_owned().into(), photo);
        cx.notify();
    }
    pub(crate) fn reload(&mut self) {
        if let Some(store) = &self.store {
            self.state = store.tickets();
        }
        if self
            .selected
            .is_some_and(|id| !self.state.tickets.iter().any(|t| t.id == id))
        {
            self.selected = None;
        }
    }
    pub(crate) fn workspace_changed(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.filters = Filters::default();
        self.menu = None;
        for input in [
            &self.input,
            &self.comment,
            &self.search,
            &self.agent_name,
            &self.agent_instructions,
            &self.agent_model,
        ] {
            input.update(cx, |input, _| input.reset());
        }
        self.form_error = None;
        self.reload();
    }
    #[cfg_attr(test, allow(dead_code))]
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing || self.pending {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        if self.error.is_some() {
            self.loading_started = Instant::now();
        }
        self.refreshing = true;
        let request = cx
            .background_executor()
            .spawn(async move { store.refresh() });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.refreshing = false;
                match result {
                    Ok(()) => {
                        this.reload();
                        this.loaded = true;
                        this.error = None;
                        this.load_activity(cx);
                    }
                    Err(error) => this.error = Some(format!("Tickets unavailable: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    /// Read the open Ticket's history in the background.
    fn load_activity(&mut self, cx: &mut Context<Self>) {
        let (Some(store), Some(id)) = (self.store.clone(), self.selected) else {
            return;
        };
        let request = cx
            .background_executor()
            .spawn(async move { store.ticket_activity(id) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                if this.selected == Some(id)
                    && let Ok(activity) = result
                {
                    this.activity = activity;
                    this.activity_for = Some(id);
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn select(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.selected != Some(id) {
            self.activity.clear();
            self.activity_for = None;
            self.editing_description = false;
        }
        self.selected = Some(id);
        self.menu = None;
        self.load_activity(cx);
        cx.notify();
    }
    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.menu = None;
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
    /// Close any open menu; returns whether one was open.
    pub fn dismiss_menus(&mut self, cx: &mut Context<Self>) -> bool {
        let was_open = self.menu.take().is_some();
        if was_open {
            cx.notify();
        }
        was_open
    }
    fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = if self.menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
        if menu == Menu::Labels {
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
        self.menu = None;
        self.draft = Draft {
            status: status.unwrap_or(TicketStatus::ToDo),
            ..Draft::default()
        };
        for input in [&self.input, &self.draft_description, &self.draft_labels] {
            input.update(cx, |input, cx| {
                input.reset();
                cx.notify();
            });
        }
        let focus = self.input.focus_handle(cx);
        self.overlays
            .borrow_mut()
            .open(Overlay::AddTicket, window, cx, Some(focus));
        cx.notify();
    }
    fn open_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form_error = None;
        for input in [
            &self.agent_name,
            &self.agent_instructions,
            &self.agent_model,
        ] {
            input.update(cx, |i, cx| {
                i.reset();
                cx.notify();
            });
        }
        let focus = self.agent_name.focus_handle(cx);
        self.overlays
            .borrow_mut()
            .open(Overlay::AddAgent, window, cx, Some(focus));
        cx.notify();
    }
    pub fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let mut handles = match self.overlays.borrow().active() {
            Some(Overlay::AddTicket) => vec![
                self.input.focus_handle(cx),
                self.draft_description.focus_handle(cx),
                self.draft_labels.focus_handle(cx),
            ],
            Some(Overlay::AddAgent) => vec![
                self.agent_name.focus_handle(cx),
                self.agent_instructions.focus_handle(cx),
                self.agent_model.focus_handle(cx),
            ],
            Some(Overlay::RenameTicket(_)) => vec![self.rename.focus_handle(cx)],
            Some(Overlay::LinkTicket(_)) => vec![self.link_search.focus_handle(cx)],
            _ => vec![],
        };
        handles.extend([self.cancel_focus.clone(), self.submit_focus.clone()]);
        handles
    }
    fn create(&mut self, cx: &mut Context<Self>) {
        if self.overlays.borrow().active() != Some(Overlay::AddTicket) {
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
        let mut labels = self.draft.labels.clone();
        for typed in self.draft_labels.read(cx).content.split(',') {
            let typed = typed.trim();
            if !typed.is_empty() && !labels.iter().any(|l| l.eq_ignore_ascii_case(typed)) {
                labels.push(typed.to_owned());
            }
        }
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
        let Some(Overlay::RenameTicket(id)) = self.overlays.borrow().active() else {
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
    /// Remove an applied label, or apply one that is not.
    fn toggle_label(&mut self, label: String, cx: &mut Context<Self>) {
        let Some(ticket) = self.selected.and_then(|id| self.ticket(id)) else {
            return;
        };
        if !ticket
            .labels
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&label))
        {
            return self.add_label(label, cx);
        }
        let labels = ticket
            .labels
            .iter()
            .filter(|existing| !existing.eq_ignore_ascii_case(&label))
            .cloned()
            .collect();
        let (id, revision) = (ticket.id, ticket.revision);
        self.menu = None;
        self.command(
            TicketCommand::SetLabels {
                id,
                revision,
                labels,
            },
            cx,
        );
    }
    fn add_label(&mut self, label: String, cx: &mut Context<Self>) {
        let Some(ticket) = self.selected.and_then(|id| self.ticket(id)) else {
            return;
        };
        if label.is_empty() || label.contains(',') || label.chars().count() > 32 {
            return;
        }
        if ticket
            .labels
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&label))
        {
            return;
        }
        let mut labels = ticket.labels.clone();
        labels.push(label);
        let (id, revision) = (ticket.id, ticket.revision);
        self.label_input.update(cx, |input, _| input.reset());
        self.menu = None;
        self.command(
            TicketCommand::SetLabels {
                id,
                revision,
                labels,
            },
            cx,
        );
    }
    fn command(&mut self, command: TicketCommand, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        self.mutate(move || store.ticket_command(command).map(|_| ()), cx);
    }
    fn mutate(
        &mut self,
        operation: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.pending {
            return;
        }
        self.pending = true;
        let request = cx.background_executor().spawn(async move { operation() });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                this.restore_focus = true;
                // The store holds the daemon's answer either way; an optimistic
                // board move that was refused snaps back here.
                this.reload();
                match result {
                    Ok(()) => {
                        this.overlays.borrow_mut().close();
                        this.form_error = None;
                        this.loaded = true;
                        this.error = None;
                        this.editing_description = false;
                        this.comment.update(cx, |input, cx| {
                            input.reset();
                            cx.notify();
                        });
                        this.load_activity(cx);
                    }
                    Err(error) => {
                        this.form_error = Some(format!("Change was not acknowledged: {error}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
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
    fn assignee_avatar(&self, id: &str, size: f32) -> AnyElement {
        if self.is_agent(id) {
            return agent_avatar(&self.assignee_name(id), size);
        }
        let photo = (id == "owner").then(|| self.owner.1.clone()).flatten();
        avatar(&self.assignee_name(id), photo, size)
    }
    fn is_agent(&self, id: &str) -> bool {
        self.assignee(id)
            .is_some_and(|a| a.kind == AssigneeKind::Agent)
    }
    fn running(&self, ticket: &Ticket) -> bool {
        self.state.runs.iter().any(|run| {
            run.ticket_id == ticket.id
                && run.generation == ticket.generation
                && matches!(run.state.as_str(), "queued" | "running")
        })
    }
    fn comment_count(&self, id: i64) -> usize {
        self.state
            .comments
            .iter()
            .filter(|c| c.ticket_id == id)
            .count()
    }

    /// A dropdown trigger with a floating menu below it.
    #[allow(clippy::too_many_arguments)]
    fn dropdown(
        &self,
        id: &'static str,
        label: SharedString,
        icon_name: &'static str,
        active: bool,
        menu: Menu,
        width: f32,
        items: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Div {
        let open = self.menu == Some(menu);
        column()
            .relative()
            .child(
                Button::new(id, label)
                    .secondary()
                    .icon(icon_name)
                    .selected(open || active)
                    .trailing(icon("chevronDown", ICON_SIZE_SM))
                    .build(
                        &self.hover,
                        move |this: &mut Self, _, cx| this.toggle_menu(menu, cx),
                        cx,
                    ),
            )
            .when(open, |s| {
                s.child(floating(
                    menu_shell(width)
                        .debug_selector(move || format!("{id}.menu"))
                        .children(items),
                    Anchor::TopLeft,
                    point(px(0.), px(CONTROL_HEIGHT + SPACE_1)),
                ))
            })
    }

    fn filter_bar(&self, window: &Window, cx: &mut Context<Self>) -> Div {
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
                .build(
                    &self.hover,
                    move |this: &mut Self, _, cx| {
                        this.filters.toggle_priority(priority);
                        cx.notify();
                    },
                    cx,
                )
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
                    .glyph("dot", label_color(&label))
                    .trailing(tally(
                        tickets
                            .iter()
                            .filter(|t| t.labels.iter().any(|l| l.eq_ignore_ascii_case(&label)))
                            .count(),
                    ))
                    .checked(checked)
                    .build(
                        &self.hover,
                        move |this: &mut Self, _, cx| {
                            this.filters.toggle_label(&label);
                            cx.notify();
                        },
                        cx,
                    )
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
                    "agents"
                } else {
                    "user"
                })
                .trailing(tally(
                    tickets.iter().filter(|t| t.assignee_id == id).count(),
                ))
                .checked(self.filters.assignees.contains(&id))
                .build(
                    &self.hover,
                    move |this: &mut Self, _, cx| {
                        this.filters.toggle_assignee(&id);
                        cx.notify();
                    },
                    cx,
                )
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
                        .leading_icon("search")
                        .selector("tickets.search")
                        .build(window, cx),
                ),
            )
            .child(self.dropdown(
                "tickets.filter.priority",
                count("Priority", self.filters.priorities.len()),
                "filter",
                !self.filters.priorities.is_empty(),
                Menu::FilterPriority,
                MENU_WIDTH,
                priority_items,
                cx,
            ))
            .child(self.dropdown(
                "tickets.filter.label",
                count("Labels", self.filters.labels.len()),
                "tag",
                !self.filters.labels.is_empty(),
                Menu::FilterLabel,
                MENU_WIDTH,
                label_items,
                cx,
            ))
            .child(self.dropdown(
                "tickets.filter.assignee",
                count("Assignee", self.filters.assignees.len()),
                "user",
                !self.filters.assignees.is_empty(),
                Menu::FilterAssignee,
                MENU_WIDTH,
                assignee_items,
                cx,
            ))
            .when(self.filters.active(), |s| {
                s.child(
                    Button::new("tickets.filter.clear", "Clear")
                        .ghost()
                        .tint(TEXT_SECONDARY)
                        .build(
                            &self.hover,
                            |this: &mut Self, _, cx| {
                                this.filters = Filters::default();
                                this.search.update(cx, |input, cx| {
                                    input.reset();
                                    cx.notify();
                                });
                                cx.notify();
                            },
                            cx,
                        ),
                )
            })
            .child(div().flex_1())
            .child(div().flex_shrink_0().child(segmented(
                "tickets.view",
                ["Board", "List"],
                usize::from(self.view == View::List),
                true,
                &self.hover,
                |this: &mut Self, index, _, cx| {
                    this.view = if index == 0 { View::Board } else { View::List };
                    this.menu = None;
                    cx.notify();
                },
                cx,
            )))
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

    fn overview(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let header = PageHeader::new("Tickets")
            .description(self.summary())
            .actions(
                Button::new("tickets.create", "New Ticket")
                    .primary()
                    .icon("plus")
                    .enabled(self.store.is_some() && !self.pending)
                    .track_focus(&self.add_focus)
                    .build(
                        &self.hover,
                        |this: &mut Self, window, cx| this.open_create(None, window, cx),
                        cx,
                    )
                    .debug_selector(|| "tickets.create".into()),
            );
        let notices: Vec<String> = self
            .error
            .iter()
            .chain(
                self.form_error
                    .iter()
                    .filter(|_| self.overlays.borrow().active().is_none()),
            )
            .cloned()
            .collect();
        let visible = self.visible();
        let body = if !self.loaded && self.error.is_none() {
            skeleton_rows("tickets.loading", 4).into_any_element()
        } else if self.view == View::Board {
            self.board(&visible, window, cx).into_any_element()
        } else if self.state.tickets.is_empty() {
            EmptyState::new("tasks", "No Tickets yet")
                .description("Create a Ticket and assign it to an agent to start work.")
                .selector("tickets.empty")
                .action(
                    Button::new("tickets.create.empty", "New Ticket")
                        .secondary()
                        .icon("plus")
                        .enabled(self.store.is_some() && !self.pending)
                        .build(
                            &self.hover,
                            |this: &mut Self, window, cx| this.open_create(None, window, cx),
                            cx,
                        ),
                )
                .build()
                .into_any_element()
        } else {
            self.list(&visible, cx).into_any_element()
        };
        let page = match self.view {
            View::Board => Page::fill(header),
            View::List => Page::document(header),
        };
        page.child(
            column()
                .id("tickets-page")
                .track_focus(&self.page_focus)
                .w_full()
                .when(self.view == View::Board, |s| s.flex_1().min_h_0())
                .gap(px(SPACE_4))
                .child(self.filter_bar(window, cx))
                .children(
                    notices
                        .into_iter()
                        .map(|notice| banner(Tone::Danger, notice)),
                )
                .child(body),
        )
        .build()
    }
}

impl Render for TicketsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        self.drag.settle(window, cx);
        if self.restore_focus && self.overlays.borrow().active().is_none() {
            self.restore_focus = false;
            window.focus(&self.page_focus, cx);
        }
        match self.selected.and_then(|id| self.ticket(id)).cloned() {
            Some(ticket) => self.detail(&ticket, window, cx).into_any_element(),
            None => self.overview(window, cx).into_any_element(),
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
