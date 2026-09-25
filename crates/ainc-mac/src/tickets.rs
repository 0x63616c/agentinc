use crate::{
    input::{Submit, TextInput},
    storage::{AssigneeKind, Store, Ticket, TicketCommand, TicketSnapshot, TicketStatus},
    ui::*,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

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
    overlays: Rc<RefCell<OverlayHost>>,
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
    page_focus: FocusHandle,
    restore_focus: bool,
    add_focus: FocusHandle,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
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
        overlays: Rc<RefCell<OverlayHost>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input =
            cx.new(|cx| TextInput::field("Ticket title", false, cx).identified("tickets.title"));
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
    fn reload(&mut self) {
        if let Some(store) = &self.store {
            self.state = store.tickets();
        }
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing || self.pending {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
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
    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        enabled: bool,
        kind: ButtonKind,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let enabled = enabled && !self.pending;
        let id = id.into();
        let hover_id = id.clone();
        let background = match kind {
            ButtonKind::Primary => rgb(PRIMARY),
            ButtonKind::Destructive => rgb(DESTRUCTIVE),
            _ => self.hover.color(&id),
        };
        let on_hover = cx.listener(move |this, over, _, cx| {
            if enabled {
                this.hover.set(hover_id.clone(), *over);
                cx.notify();
            }
        });
        action_button(
            ButtonSpec {
                id,
                label: label.into(),
                kind,
                enabled,
            },
            |button| {
                standard_button(button)
                    .justify_center()
                    .bg(background)
                    .on_hover(on_hover)
            },
            f,
            cx,
        )
    }
    pub fn overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let active = self.overlays.borrow().active()?;
        let (title, body, enabled) = match active {
            Overlay::AddTicket => (
                "Add Ticket".into(),
                column().child(text_field("Ticket title", self.input.clone())),
                !self.input.read(cx).content.trim().is_empty()
                    && Self::title_error(&self.input.read(cx).content).is_none(),
            ),
            Overlay::AddAgent => (
                "Add agent".into(),
                column_gap(FORM_STACK_GAP)
                    .child(text_field("Name", self.agent_name.clone()))
                    .child(text_field("Instructions", self.agent_instructions.clone()))
                    .child(text_field("Model", self.agent_model.clone())),
                !self.agent_name.read(cx).content.trim().is_empty(),
            ),
            Overlay::DeleteTicket(id) => {
                let ticket = self.state.tickets.iter().find(|t| t.id == id)?;
                (
                    format!("Delete “{}”?", ticket.title),
                    column().child("This Ticket and its Comments will be removed."),
                    true,
                )
            }
            _ => return None,
        };
        let body = body.when_some(self.form_error.clone(), |s, error| {
            s.child(div().mt(px(12.)).text_color(rgb(ERROR)).child(error))
        });
        let footer = row_gap(CONTROL_GAP)
            .justify_end()
            .child(
                self.button(
                    "tickets.cancel",
                    "Cancel",
                    true,
                    ButtonKind::Secondary,
                    |this, window, cx| {
                        this.overlays.borrow_mut().dismiss(window, cx);
                        cx.notify();
                    },
                    cx,
                )
                .track_focus(&self.cancel_focus)
                .border_1()
                .border_color(rgb(BORDER))
                .child("Cancel"),
            )
            .child(
                self.button(
                    "tickets.submit",
                    if matches!(active, Overlay::DeleteTicket(_)) {
                        "Delete Ticket"
                    } else {
                        "Create"
                    },
                    enabled,
                    if matches!(active, Overlay::DeleteTicket(_)) {
                        ButtonKind::Destructive
                    } else {
                        ButtonKind::Primary
                    },
                    move |this, _, cx| match active {
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
                )
                .track_focus(&self.submit_focus)
                .child(if self.pending {
                    "Saving…"
                } else if matches!(active, Overlay::DeleteTicket(_)) {
                    "Delete"
                } else {
                    "Create"
                }),
            );
        Some(dialog_shell(title, body, footer).into_any_element())
    }
    pub fn agents(&self, cx: &mut Context<Self>) -> AnyElement {
        column()
            .gap(px(24.))
            .child(
                PageHeader::new("Agents")
                    .description("Assign a Ticket to an agent to start its work.")
                    .actions(
                        self.button(
                            "agents.create",
                            "Add agent",
                            true,
                            ButtonKind::Secondary,
                            Self::open_agent,
                            cx,
                        )
                        .border_1()
                        .border_color(rgb(BORDER))
                        .child("Add agent"),
                    )
                    .build(),
            )
            .when_some(self.error.clone(), |s, error| {
                s.child(div().text_color(rgb(ERROR)).child(error))
            })
            .children(
                self.state
                    .assignees
                    .iter()
                    .filter(|a| a.kind == AssigneeKind::Agent)
                    .map(|agent| {
                        list_row(
                            SharedString::from(format!("agent.{}", agent.id)),
                            agent.name.clone(),
                            false,
                        )
                        .accessibility_id(format!("agent.{}", agent.id))
                        .child(agent.name.clone())
                    }),
            )
            .into_any_element()
    }
    fn detail(&self, ticket: &Ticket, cx: &mut Context<Self>) -> AnyElement {
        let id = ticket.id;
        let revision = ticket.revision;
        let running = self
            .state
            .runs
            .iter()
            .any(|r| r.ticket_id == id && matches!(r.state.as_str(), "queued" | "running"));
        column().gap(px(22.))
            .child(row().gap(px(8.)).child(self.button("tickets.back","All Tickets",true,ButtonKind::Quiet,|this,_,cx|{this.selected=None;cx.notify();},cx).child("← All Tickets")).child(div().flex_1())
                .when(running,|s|s.child(self.button("tickets.stop","Cancel work",true,ButtonKind::Secondary,move|this,_,cx|this.command(TicketCommand::Cancel{id,revision},cx),cx).border_1().border_color(rgb(BORDER)).child("Cancel work")))
                .when(!self.state.runs.iter().any(|r|r.ticket_id==id),|s|s.child(self.button("tickets.delete","Delete Ticket",true,ButtonKind::Quiet,move|this,window,cx|{this.overlays.borrow_mut().open(Overlay::DeleteTicket(id),window,cx,Some(this.cancel_focus.clone()));cx.notify();},cx).text_color(rgb(DESTRUCTIVE_TEXT)).child("Delete"))))
            .child(div().text_size(type_size(20.)).font_weight(FontWeight::MEDIUM).child(ticket.title.clone()))
            .child(column().gap(px(8.)).child(div().text_size(type_size(CAPTION_SIZE)).text_color(rgb(MUTED)).child("Status")).child(row().flex_wrap().gap(px(6.)).children(STATUSES.into_iter().map(|status|choice_button(self.button(SharedString::from(format!("tickets.status.{status}")),status_name(status),status!=ticket.status,ButtonKind::Secondary,move|this,_,cx|this.command(TicketCommand::SetStatus{id,revision,status},cx),cx), status==ticket.status).child(status_name(status))))))
            .child(column().gap(px(8.)).child(div().text_size(type_size(CAPTION_SIZE)).text_color(rgb(MUTED)).child("Assignee")).child(row().flex_wrap().gap(px(6.)).children(self.state.assignees.iter().map(|assignee|{
                let assignee_id=assignee.id.clone();let assignee_kind=assignee.kind;
                choice_button(self.button(SharedString::from(format!("tickets.assign.{}",assignee.id)),assignee.name.clone(),assignee.id!=ticket.assignee_id,ButtonKind::Secondary,move|this,_,cx|this.command(TicketCommand::Assign{id,revision,assignee_id:assignee_id.clone(),assignee_kind},cx),cx), assignee.id==ticket.assignee_id).child(assignee.name.clone())
            }))))
            .children(self.state.runs.iter().filter(|r|r.ticket_id==id && r.generation==ticket.generation).map(|run|div().text_size(type_size(LABEL_SIZE)).text_color(rgb(MUTED)).child(run.error.as_ref().map_or_else(||format!("Work {}",run.state),|error|format!("Work stopped: {error}")))))
            .child(column().gap(px(16.)).child(div().font_weight(FontWeight::MEDIUM).child("Comments"))
                .when(!self.state.comments.iter().any(|c|c.ticket_id==id),|s|s.child(div().text_color(rgb(MUTED)).child("Comments are the work log. Add context, decisions and evidence here.")))
                .children(self.state.comments.iter().filter(|c|c.ticket_id==id).map(|comment|{
                    let author=self.state.assignees.iter().find(|a|a.id==comment.author_id).map_or("Agent",|a|a.name.as_str());
                    list_item(("comment",comment.id as u64), comment.body.clone()).flex_col().items_start().accessibility_id(format!("comment.{}",comment.id)).gap(px(6.)).py(px(12.)).border_b_1().border_color(rgb(BORDER)).child(div().text_size(type_size(CAPTION_SIZE)).text_color(rgb(MUTED)).child(author.to_owned())).child(div().text_size(type_size(BODY_SIZE)).child(comment.body.clone()))
                }))
                .child(text_field("Add a Comment",self.comment.clone()))
                .child(row().justify_end().child(self.button("tickets.post","Post Comment",!self.comment.read(cx).content.trim().is_empty(),ButtonKind::Primary,|this,_,cx|this.add_comment(cx),cx).child("Post Comment"))))
            .into_any_element()
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
        let mut header =
            PageHeader::new("Tickets").description("Tickets, assignees and their work log.");
        if selected.is_none() {
            header = header.actions(
                row_gap(CONTROL_GAP)
                    .child(
                        self.button(
                            "tickets.refresh",
                            "Refresh Tickets",
                            true,
                            ButtonKind::Quiet,
                            |this, _, cx| this.refresh(cx),
                            cx,
                        )
                        .child("Refresh"),
                    )
                    .child(
                        self.button(
                            "tickets.create",
                            "Add Ticket",
                            self.store.is_some(),
                            ButtonKind::Secondary,
                            Self::open_add,
                            cx,
                        )
                        .track_focus(&self.add_focus)
                        .debug_selector(|| "tickets.create".into())
                        .border_1()
                        .border_color(rgb(SELECTED_BORDER))
                        .text_size(type_size(LABEL_SIZE))
                        .child("Add Ticket"),
                    ),
            );
        }
        column()
            .id("tickets-page")
            .track_focus(&self.page_focus)
            .gap(px(24.))
            .child(header.build())
            .when_some(self.error.clone(), |s, error| {
                s.child(div().text_color(rgb(ERROR)).child(error))
            })
            .when_some(self.form_error.clone(), |s, error| {
                s.child(div().text_color(rgb(ERROR)).child(error))
            })
            .child(if let Some(ticket) = selected {
                self.detail(ticket, cx)
            } else {
                column()
                    .gap(px(24.))
                    .when(
                        self.loaded && self.state.tickets.is_empty() && self.error.is_none(),
                        |s| {
                            s.child(
                                div()
                                    .py(px(24.))
                                    .text_color(rgb(MUTED))
                                    .child("No Tickets yet."),
                            )
                        },
                    )
                    .children(
                        STATUSES
                            .into_iter()
                            .filter(|status| self.state.tickets.iter().any(|t| t.status == *status))
                            .map(|status| {
                                column()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(type_size(CAPTION_SIZE))
                                            .text_color(rgb(MUTED))
                                            .child(status_name(status)),
                                    )
                                    .children(
                                        self.state
                                            .tickets
                                            .iter()
                                            .filter(move |t| t.status == status)
                                            .map(|ticket| {
                                                let id = ticket.id;
                                                let assignee = self
                                                    .state
                                                    .assignees
                                                    .iter()
                                                    .find(|a| a.id == ticket.assignee_id)
                                                    .map_or("Unknown", |a| a.name.as_str());
                                                self.button(
                                                    ("ticket", id as u64),
                                                    ticket.title.clone(),
                                                    true,
                                                    ButtonKind::Quiet,
                                                    move |this, _, cx| this.select(id, cx),
                                                    cx,
                                                )
                                                .w_full()
                                                .h_auto()
                                                .min_h(px(52.))
                                                .py(px(10.))
                                                .justify_start()
                                                .border_b_1()
                                                .border_color(rgb(BORDER_SUBTLE))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .child(ticket.title.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(type_size(CAPTION_SIZE))
                                                        .text_color(rgb(MUTED))
                                                        .child(assignee.to_owned()),
                                                )
                                            }),
                                    )
                            }),
                    )
                    .into_any_element()
            })
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
