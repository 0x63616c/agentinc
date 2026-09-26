use crate::{
    input::{Submit, TextInput},
    model::Overlay,
    storage::{AssigneeKind, Store, Ticket, TicketCommand, TicketSnapshot, TicketStatus},
    ui::*,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};

fn status_tone(status: TicketStatus) -> Tone {
    match status {
        TicketStatus::Backlog => Tone::Neutral,
        TicketStatus::ToDo => Tone::Info,
        TicketStatus::InProgress => Tone::Warning,
        TicketStatus::Done => Tone::Success,
    }
}
fn status_name(status: TicketStatus) -> &'static str {
    match status {
        TicketStatus::Backlog => "Backlog",
        TicketStatus::ToDo => "To do",
        TicketStatus::InProgress => "In progress",
        TicketStatus::Done => "Done",
    }
}
const STATUSES: [TicketStatus; 4] = [
    TicketStatus::Backlog,
    TicketStatus::ToDo,
    TicketStatus::InProgress,
    TicketStatus::Done,
];
pub struct TicketsPage {
    store: Option<Arc<Store>>,
    overlays: Rc<RefCell<OverlayHost<Overlay>>>,
    state: TicketSnapshot,
    selected: Option<i64>,
    input: Entity<TextInput>,
    comment: Entity<TextInput>,
    agent_name: Entity<TextInput>,
    agent_instructions: Entity<TextInput>,
    agent_model: Entity<TextInput>,
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
impl TicketsPage {
    pub(crate) fn update_drafts(&self, cx: &App) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(
            !self.pending,
            "Wait for the current change to finish before installing"
        );
        Ok(
            serde_json::json!({"selected":self.selected,"input": self.input.read(cx).content.to_string(), "comment": self.comment.read(cx).content.to_string(), "agent_name": self.agent_name.read(cx).content.to_string(), "agent_instructions": self.agent_instructions.read(cx).content.to_string(), "agent_model": self.agent_model.read(cx).content.to_string()}),
        )
    }
    pub(crate) fn restore_update_drafts(
        &mut self,
        value: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if let Ok(value) = serde_json::from_value(value["selected"].clone()) {
            self.selected = value;
        }
        if let Some(text) = value["input"].as_str() {
            self.input.update(cx, |input, cx| input.set_text(text, cx));
        }
        if let Some(text) = value["comment"].as_str() {
            self.comment
                .update(cx, |input, cx| input.set_text(text, cx));
        }
        if let Some(text) = value["agent_name"].as_str() {
            self.agent_name
                .update(cx, |input, cx| input.set_text(text, cx));
        }
        if let Some(text) = value["agent_instructions"].as_str() {
            self.agent_instructions
                .update(cx, |input, cx| input.set_text(text, cx));
        }
        if let Some(text) = value["agent_model"].as_str() {
            self.agent_model
                .update(cx, |input, cx| input.set_text(text, cx));
        }
    }

    pub fn new(
        store: Option<Arc<Store>>,
        storage_error: Option<String>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx
            .new(|cx| TextInput::field("What needs doing?", false, cx).identified("tickets.title"));
        let comment = cx
            .new(|cx| TextInput::field("Add a Comment…", false, cx).identified("tickets.comment"));
        let agent_name = cx.new(|cx| TextInput::field("Name", false, cx).identified("agents.name"));
        let agent_instructions = cx.new(|cx| {
            TextInput::field("Instructions", false, cx).identified("agents.instructions")
        });
        let agent_model = cx
            .new(|cx| TextInput::field("Connection default", false, cx).identified("agents.model"));
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.add(cx)),
            cx.observe(&input, |this, input, cx| {
                this.form_error = Self::title_error(&input.read(cx).content).map(str::to_owned);
                cx.notify();
            }),
            cx.subscribe(&comment, |this, _, _: &Submit, cx| this.add_comment(cx)),
            cx.observe(&comment, |_, _, cx| cx.notify()),
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
            },
            selected: None,
            input,
            comment,
            agent_name,
            agent_instructions,
            agent_model,
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
    pub(crate) fn reload(&mut self) {
        if let Some(store) = &self.store {
            self.state = store.tickets();
        }
    }
    pub(crate) fn workspace_changed(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        for input in [
            &self.input,
            &self.comment,
            &self.agent_name,
            &self.agent_instructions,
            &self.agent_model,
        ] {
            input.update(cx, |input, _| input.reset());
        }
        self.form_error = None;
        self.reload();
    }
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
                    }
                    Err(error) => this.error = Some(format!("Tickets unavailable: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn select(&mut self, id: i64, cx: &mut Context<Self>) {
        self.selected = Some(id);
        cx.notify();
    }
    fn title_error(title: &str) -> Option<&'static str> {
        (title.trim().chars().count() > 500).then_some("Use 500 characters or fewer.")
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form_error = None;
        self.input.update(cx, |i, cx| {
            i.reset();
            cx.notify();
        });
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
            Some(Overlay::AddTicket) => vec![self.input.focus_handle(cx)],
            Some(Overlay::AddAgent) => vec![
                self.agent_name.focus_handle(cx),
                self.agent_instructions.focus_handle(cx),
                self.agent_model.focus_handle(cx),
            ],
            _ => vec![],
        };
        handles.extend([self.cancel_focus.clone(), self.submit_focus.clone()]);
        handles
    }
    fn add(&mut self, cx: &mut Context<Self>) {
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
        self.command(TicketCommand::Create { title }, cx);
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
                match result {
                    Ok(()) => {
                        this.overlays.borrow_mut().close();
                        this.form_error = None;
                        this.loaded = true;
                        this.error = None;
                        this.reload();
                        this.comment.update(cx, |input, cx| {
                            input.reset();
                            cx.notify();
                        });
                        if this
                            .selected
                            .is_some_and(|id| !this.state.tickets.iter().any(|t| t.id == id))
                        {
                            this.selected = None;
                        }
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
    pub fn overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let active = self.overlays.borrow().active()?;
        let (title, body, enabled) = match active {
            Overlay::AddTicket => (
                "New Ticket".into(),
                column().child(
                    Field::new(self.input.clone())
                        .label("Ticket title")
                        .hint("You can assign it to an agent afterwards.")
                        .error(self.form_error.clone())
                        .build(window, cx),
                ),
                !self.input.read(cx).content.trim().is_empty()
                    && Self::title_error(&self.input.read(cx).content).is_none(),
            ),
            Overlay::AddAgent => (
                "New Agent".into(),
                column_gap(FORM_STACK_GAP)
                    .child(text_field("Name", self.agent_name.clone(), window, cx))
                    .child(
                        Field::new(self.agent_instructions.clone())
                            .label("Instructions")
                            .multiline()
                            .build(window, cx),
                    )
                    .child(
                        Field::new(self.agent_model.clone())
                            .label("Model")
                            .hint("Leave empty to use the connection default.")
                            .build(window, cx),
                    )
                    .when_some(self.form_error.clone(), |s, error| {
                        s.child(error_text(error))
                    }),
                !self.agent_name.read(cx).content.trim().is_empty(),
            ),
            Overlay::DeleteTicket(id) => {
                let ticket = self.state.tickets.iter().find(|t| t.id == id)?;
                (
                    format!("Delete “{}”?", ticket.title),
                    column()
                        .gap(px(SPACE_2))
                        .child(caption("This Ticket and its Comments will be removed."))
                        .when_some(self.form_error.clone(), |s, error| {
                            s.child(error_text(error))
                        }),
                    true,
                )
            }
            _ => return None,
        };
        let deleting = matches!(active, Overlay::DeleteTicket(_));
        let submit_label = if self.pending {
            "Saving…"
        } else if deleting {
            "Delete"
        } else {
            "Create"
        };
        let footer = dialog_footer(
            Button::new("tickets.cancel", "Cancel")
                .secondary()
                .track_focus(&self.cancel_focus)
                .build(
                    &self.hover,
                    |this: &mut Self, window, cx| {
                        this.overlays.borrow_mut().dismiss(window, cx);
                        cx.notify();
                    },
                    cx,
                ),
            Button::new("tickets.submit", submit_label)
                .kind(if deleting {
                    ButtonKind::Destructive
                } else {
                    ButtonKind::Primary
                })
                .enabled(enabled && !self.pending)
                .track_focus(&self.submit_focus)
                .build(
                    &self.hover,
                    move |this: &mut Self, _, cx| match active {
                        Overlay::AddTicket => this.add(cx),
                        Overlay::AddAgent => {
                            let name = this.agent_name.read(cx).content.trim().to_owned();
                            let instructions =
                                this.agent_instructions.read(cx).content.trim().to_owned();
                            let model = this.agent_model.read(cx).content.trim().to_owned();
                            this.command(
                                TicketCommand::RegisterAgent {
                                    name,
                                    instructions,
                                    model: if model.is_empty() {
                                        "connection-default".into()
                                    } else {
                                        model
                                    },
                                },
                                cx,
                            );
                        }
                        Overlay::DeleteTicket(id) => {
                            if let Some(ticket) = this.state.tickets.iter().find(|t| t.id == id) {
                                this.command(
                                    TicketCommand::Delete {
                                        id,
                                        revision: ticket.revision,
                                    },
                                    cx,
                                );
                            }
                        }
                        _ => {}
                    },
                    cx,
                ),
        );
        Some(dialog_shell(title, body, footer).into_any_element())
    }
    pub fn agents(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.hover.animate(window);
        let agents: Vec<_> = self
            .state
            .assignees
            .iter()
            .filter(|a| a.kind == AssigneeKind::Agent)
            .collect();
        let add = |this: &Self, id: &'static str, cx: &mut Context<Self>| {
            Button::new(id, "New Agent")
                .primary()
                .icon("plus")
                .enabled(this.store.is_some() && !this.pending)
                .build(&this.hover, Self::open_agent, cx)
        };
        Page::document(
            PageHeader::new("Agents")
                .description("Agents pick up the Tickets you assign to them.")
                .actions(add(self, "agents.create", cx)),
        )
        .child(
            column()
                .gap(px(SPACE_4))
                .when_some(self.error.clone(), |s, error| s.child(banner(Tone::Danger, error)))
                .when(self.refreshing && self.error.is_some(), |s| {
                    s.child(LoadingFrame::new(self.loading_started, window).inline("Reconnecting…"))
                })
                .when(!self.loaded && self.error.is_none(), |s| {
                    s.child(skeleton_rows("agents.loading", 3))
                })
                .when(self.loaded && agents.is_empty() && self.error.is_none(), |s| {
                    s.child(
                        EmptyState::new("agents", "No Agents yet")
                            .description("Register an agent with instructions and a model, then assign it Tickets.")
                            .selector("agents.empty")
                            .action(
                                Button::new("agents.create.empty", "New Agent")
                                    .secondary()
                                    .icon("plus")
                                    .enabled(self.store.is_some() && !self.pending)
                                    .build(&self.hover, Self::open_agent, cx),
                            )
                            .build(),
                    )
                })
                .when(!agents.is_empty(), |s| {
                    s.child(card().p(px(SPACE_1)).gap_0().children(agents.iter().map(|agent| {
                        let name = agent.name.clone();
                        let assigned = self
                            .state
                            .tickets
                            .iter()
                            .filter(|t| t.assignee_id == agent.id && t.status != TicketStatus::Done)
                            .count();
                        ListRow::new(SharedString::from(format!("agent.{}", agent.id)), name.clone())
                            .leading(avatar(&name, None, AVATAR_SIZE))
                            .subtitle(match assigned {
                                0 => "No open Tickets".to_owned(),
                                1 => "1 open Ticket".to_owned(),
                                n => format!("{n} open Tickets"),
                            })
                            .trailing(badge("Agent", Tone::Neutral))
                            .build(&self.hover, |_, _, _| {}, cx)
                            .accessibility_id(format!("agent.{}", agent.id))
                    })))
                }),
        )
        .build()
        .into_any_element()
    }
    /// The header for one Ticket: a way back, the title as the page's one
    /// heading, its meta line, and the actions that apply to it.
    fn detail_header(&self, ticket: &Ticket, cx: &mut Context<Self>) -> PageHeader {
        let id = ticket.id;
        let revision = ticket.revision;
        let running = self
            .state
            .runs
            .iter()
            .any(|r| r.ticket_id == id && matches!(r.state.as_str(), "queued" | "running"));
        let has_runs = self.state.runs.iter().any(|r| r.ticket_id == id);
        let assignee = self
            .state
            .assignees
            .iter()
            .find(|a| a.id == ticket.assignee_id)
            .map_or("Unassigned", |a| a.name.as_str());
        let enabled = !self.pending;
        PageHeader::new(ticket.title.clone())
            .leading(
                Button::new("tickets.back", "Tickets")
                    .ghost()
                    .small()
                    .icon("chevronLeft")
                    .build(
                        &self.hover,
                        |this, _, cx| {
                            this.selected = None;
                            cx.notify();
                        },
                        cx,
                    )
                    .ml(px(-CONTROL_INSET_X_SM)),
            )
            .description(format!(
                "Ticket {id} · {} · {assignee}",
                status_name(ticket.status)
            ))
            .actions(
                row_gap(CONTROL_GAP)
                    .when(running, |s| {
                        s.child(
                            Button::new("tickets.stop", "Cancel work")
                                .secondary()
                                .icon("stop")
                                .enabled(enabled)
                                .build(
                                    &self.hover,
                                    move |this, _, cx| {
                                        this.command(TicketCommand::Cancel { id, revision }, cx)
                                    },
                                    cx,
                                ),
                        )
                    })
                    .when(!has_runs, |s| {
                        s.child(
                            Button::new("tickets.delete", "Delete")
                                .ghost()
                                .icon("trash")
                                .enabled(enabled)
                                .build(
                                    &self.hover,
                                    move |this, window, cx| {
                                        this.overlays.borrow_mut().open(
                                            Overlay::DeleteTicket(id),
                                            window,
                                            cx,
                                            Some(this.cancel_focus.clone()),
                                        );
                                        cx.notify();
                                    },
                                    cx,
                                ),
                        )
                    }),
            )
    }
    fn detail(&self, ticket: &Ticket, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let id = ticket.id;
        let revision = ticket.revision;
        let comments: Vec<_> = self
            .state
            .comments
            .iter()
            .filter(|c| c.ticket_id == id)
            .collect();
        let enabled = !self.pending;
        let section = |title: &'static str, body: Div| {
            column().gap(px(SPACE_2)).child(eyebrow(title)).child(body)
        };
        column()
            .gap(px(SECTION_GAP))
            .child(
                card()
                    .gap(px(SPACE_4))
                    .child(section(
                        "Status",
                        row()
                            .flex_wrap()
                            .gap(px(CHIP_GAP))
                            .children(STATUSES.into_iter().map(|status| {
                                let current = status == ticket.status;
                                chip(
                                    SharedString::from(format!("tickets.status.{status}")),
                                    status_name(status),
                                    current,
                                    enabled,
                                    &self.hover,
                                    move |this, _, cx| {
                                        if !current {
                                            this.command(
                                                TicketCommand::SetStatus {
                                                    id,
                                                    revision,
                                                    status,
                                                },
                                                cx,
                                            )
                                        }
                                    },
                                    cx,
                                )
                            })),
                    ))
                    .child(divider())
                    .child(section(
                        "Assignee",
                        row().flex_wrap().gap(px(CHIP_GAP)).children(
                            self.state.assignees.iter().map(|assignee| {
                                let assignee_id = assignee.id.clone();
                                let assignee_kind = assignee.kind;
                                let current = assignee.id == ticket.assignee_id;
                                chip(
                                    SharedString::from(format!("tickets.assign.{}", assignee.id)),
                                    assignee.name.clone(),
                                    current,
                                    enabled,
                                    &self.hover,
                                    move |this, _, cx| {
                                        if !current {
                                            this.command(
                                                TicketCommand::Assign {
                                                    id,
                                                    revision,
                                                    assignee_id: assignee_id.clone(),
                                                    assignee_kind,
                                                },
                                                cx,
                                            )
                                        }
                                    },
                                    cx,
                                )
                            }),
                        ),
                    ))
                    .children(
                        self.state
                            .runs
                            .iter()
                            .filter(|r| r.ticket_id == id && r.generation == ticket.generation)
                            .map(|run| {
                                let tone = match run.state.as_str() {
                                    "queued" => Tone::Neutral,
                                    "running" => Tone::Info,
                                    "done" | "completed" => Tone::Success,
                                    _ => Tone::Warning,
                                };
                                column().gap(px(SPACE_4)).child(divider()).child(section(
                                    "Work",
                                    row()
                                        .gap(px(SPACE_3))
                                        .child(status_pill(
                                            run.state.clone(),
                                            if run.error.is_some() {
                                                Tone::Danger
                                            } else {
                                                tone
                                            },
                                        ))
                                        .when_some(run.error.clone(), |s, error| {
                                            s.child(caption(error))
                                        }),
                                ))
                            }),
                    ),
            )
            .child(
                column()
                    .gap(px(SPACE_3))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .child(heading("Comments"))
                            .child(caption(format!("{}", comments.len()))),
                    )
                    .when(comments.is_empty(), |s| {
                        s.child(caption(
                            "Comments are the work log. Add context, decisions and evidence here.",
                        ))
                    })
                    .children(comments.iter().map(|comment| {
                        let author = self
                            .state
                            .assignees
                            .iter()
                            .find(|a| a.id == comment.author_id)
                            .map_or("Agent", |a| a.name.as_str())
                            .to_owned();
                        list_item(("comment", comment.id as u64), comment.body.clone())
                            .items_start()
                            .accessibility_id(format!("comment.{}", comment.id))
                            .gap(px(SPACE_3))
                            .py(px(SPACE_4))
                            .border_b_1()
                            .border_color(rgb(BORDER_SUBTLE))
                            .child(avatar(&author, None, AVATAR_SIZE))
                            .child(
                                column()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(SPACE_HALF))
                                    .child(
                                        div()
                                            .text_size(type_size(LABEL_SIZE))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(author),
                                    )
                                    .child(
                                        div()
                                            .text_size(type_size(BODY_SIZE))
                                            .text_color(rgb(TEXT_SECONDARY))
                                            .child(comment.body.clone()),
                                    ),
                            )
                    }))
                    .child(
                        row()
                            .mt(px(SPACE_2))
                            .items_start()
                            .gap(px(SPACE_3))
                            .child(avatar("You", None, AVATAR_SIZE))
                            .child(
                                column()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(SPACE_2))
                                    .child(
                                        Field::new(self.comment.clone())
                                            .selector("Add a Comment")
                                            .build(window, cx),
                                    )
                                    .child(
                                        row().justify_end().child(
                                            Button::new("tickets.post", "Post Comment")
                                                .primary()
                                                .enabled(
                                                    enabled
                                                        && !self
                                                            .comment
                                                            .read(cx)
                                                            .content
                                                            .trim()
                                                            .is_empty(),
                                                )
                                                .build(
                                                    &self.hover,
                                                    |this, _, cx| this.add_comment(cx),
                                                    cx,
                                                ),
                                        ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn list(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut groups = column().gap(px(SPACE_5));
        for status in STATUSES {
            let tickets: Vec<_> = self
                .state
                .tickets
                .iter()
                .filter(|t| t.status == status)
                .collect();
            if tickets.is_empty() {
                continue;
            }
            let count = tickets.len();
            groups = groups.child(
                column()
                    .gap(px(SPACE_2))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .child(eyebrow(status_name(status)))
                            .child(caption(count.to_string())),
                    )
                    .child(card().p(px(SPACE_1)).gap_0().children(
                        tickets.into_iter().enumerate().map(|(index, ticket)| {
                            let id = ticket.id;
                            let assignee = self
                                .state
                                .assignees
                                .iter()
                                .find(|a| a.id == ticket.assignee_id)
                                .map_or("Unassigned", |a| a.name.as_str())
                                .to_owned();
                            let running = self.state.runs.iter().any(|r| {
                                r.ticket_id == id
                                    && matches!(r.state.as_str(), "queued" | "running")
                            });
                            let comments = self
                                .state
                                .comments
                                .iter()
                                .filter(|c| c.ticket_id == id)
                                .count();
                            let mut subtitle = format!("T-{id} · {assignee}");
                            if comments > 0 {
                                subtitle.push_str(&format!(
                                    " · {comments} comment{}",
                                    if comments == 1 { "" } else { "s" }
                                ));
                            }
                            column()
                                .when(index + 1 < count, |s| {
                                    s.border_b_1().border_color(rgb(BORDER_SUBTLE))
                                })
                                .child(
                                    ListRow::new(("ticket", id as u64), ticket.title.clone())
                                        .leading(status_dot(status_tone(status)))
                                        .subtitle(subtitle)
                                        .trailing(
                                            row()
                                                .gap(px(SPACE_2))
                                                .when(running, |s| {
                                                    s.child(badge("Running", Tone::Info))
                                                })
                                                .child(icon("chevronRight", ICON_SIZE)),
                                        )
                                        .build(
                                            &self.hover,
                                            move |this, _, cx| this.select(id, cx),
                                            cx,
                                        ),
                                )
                        }),
                    )),
            );
        }
        groups.into_any_element()
    }
}
impl Render for TicketsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        if self.restore_focus && self.overlays.borrow().active().is_none() {
            self.restore_focus = false;
            window.focus(&self.page_focus, cx);
        }
        let selected = self
            .selected
            .and_then(|id| self.state.tickets.iter().find(|t| t.id == id));
        let header = match selected {
            Some(ticket) => self.detail_header(ticket, cx),
            None => PageHeader::new("Tickets")
                .description("Work for you and your agents, with Comments as the log.")
                .actions(
                    row_gap(CONTROL_GAP)
                        .child(
                            Button::new("tickets.refresh", "Refresh Tickets")
                                .icon("refresh")
                                .icon_only()
                                .secondary()
                                .enabled(!self.refreshing)
                                .build(&self.hover, |this, _, cx| this.refresh(cx), cx),
                        )
                        .child(
                            Button::new("tickets.create", "New Ticket")
                                .primary()
                                .icon("plus")
                                .enabled(self.store.is_some() && !self.pending)
                                .track_focus(&self.add_focus)
                                .build(&self.hover, Self::open_add, cx)
                                .debug_selector(|| "tickets.create".into()),
                        ),
                ),
        };
        let empty = self.loaded && self.state.tickets.is_empty() && self.error.is_none();
        Page::document(header)
            .child(
                column()
                    .id("tickets-page")
                    .track_focus(&self.page_focus)
                    .gap(px(SECTION_GAP))
                    .when_some(self.error.clone(), |s, error| {
                        s.child(banner(Tone::Danger, error))
                    })
                    .when_some(
                        self.form_error
                            .clone()
                            .filter(|_| self.overlays.borrow().active().is_none()),
                        |s, error| s.child(banner(Tone::Danger, error)),
                    )
                    .when(!self.loaded && self.error.is_none(), |s| {
                        s.child(skeleton_rows("tickets.loading", 4))
                    })
                    .when(empty && selected.is_none(), |s| {
                        s.child(
                            EmptyState::new("tasks", "No Tickets yet")
                                .description(
                                    "Create a Ticket and assign it to an agent to start work.",
                                )
                                .selector("tickets.empty")
                                .action(
                                    Button::new("tickets.create.empty", "New Ticket")
                                        .secondary()
                                        .icon("plus")
                                        .enabled(self.store.is_some() && !self.pending)
                                        .build(&self.hover, Self::open_add, cx),
                                )
                                .build(),
                        )
                    })
                    .child(match selected {
                        Some(ticket) => self.detail(ticket, window, cx),
                        None => self.list(cx),
                    }),
            )
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::TicketsPage;
    #[test]
    fn validates_titles_and_previews_real_open_tickets() {
        assert_eq!(
            TicketsPage::title_error(&"A".repeat(501)),
            Some("Use 500 characters or fewer.")
        );
        assert_eq!(TicketsPage::title_error(&"A".repeat(500)), None);
    }
}
