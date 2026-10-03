//! Agents: the registered agents with their open Tickets and live work, and
//! the dialog that registers one.
use crate::{
    action::{Pending, Run},
    daemon::Daemon,
    input::TextInput,
    overlay::Overlay,
    page::{Drafts, Page, PageOverlays},
    routes::{Destination, Route},
    sync::{SliceChanged, Sync},
    ui::*,
};
use ainc_client::types::{AssigneeKind, TicketCommand, TicketSnapshot, TicketStatus};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
    Add,
}

pub struct AgentsPage {
    daemon: Arc<Daemon>,
    sync: Entity<Sync>,
    overlays: PageOverlays<Dialog>,
    state: TicketSnapshot,
    name: Entity<TextInput>,
    instructions: Entity<TextInput>,
    model: Entity<TextInput>,
    form_error: Option<String>,
    pending: Pending,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Destination> for AgentsPage {}

impl AgentsPage {
    pub fn new(
        daemon: Arc<Daemon>,
        sync: Entity<Sync>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = |placeholder: &str, id: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| TextInput::field(placeholder, false, cx).identified(id))
        };
        let name = field("Name", "agents.name", cx);
        let instructions = field("Instructions", "agents.instructions", cx);
        let model = field("Model", "agents.model", cx);
        let subscriptions = vec![
            cx.observe(&name, |_, _, cx| cx.notify()),
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, event: &SliceChanged, cx| {
                if *event == SliceChanged::Tickets {
                    this.reload();
                    cx.notify();
                }
            }),
        ];
        let mut this = Self {
            daemon,
            sync,
            overlays: PageOverlays::new(overlays, Route::Agents),
            state: TicketSnapshot {
                tickets: vec![],
                comments: vec![],
                assignees: vec![],
                runs: vec![],
                links: vec![],
            },
            name,
            instructions,
            model,
            form_error: None,
            pending: Pending::default(),
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        this.reload();
        this
    }
    fn reload(&mut self) {
        self.state = (*self.daemon.tickets()).clone();
    }
    fn inputs(&self) -> [(&'static str, &Entity<TextInput>); 3] {
        [
            ("name", &self.name),
            ("instructions", &self.instructions),
            ("model", &self.model),
        ]
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form_error = None;
        for (_, input) in self.inputs() {
            input.update(cx, |input, cx| {
                input.reset();
                cx.notify();
            });
        }
        let focus = self.name.focus_handle(cx);
        self.overlays.open_dialog(Dialog::Add, focus, window, cx);
        cx.notify();
    }
    fn register(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).content.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let instructions = self.instructions.read(cx).content.trim().to_owned();
        let model = self.model.read(cx).content.trim().to_owned();
        let daemon = self.daemon.clone();
        cx.run(
            &self.pending.clone(),
            move || {
                daemon.send(TicketCommand::RegisterAgent {
                    name,
                    instructions,
                    model: if model.is_empty() {
                        "connection-default".into()
                    } else {
                        model
                    },
                })?;
                Ok(())
            },
            |this, result, _| {
                this.reload();
                match result {
                    Ok(()) => {
                        this.overlays.close();
                        this.form_error = None;
                    }
                    Err(failure) => this.form_error = Some(failure.message("The agent")),
                }
            },
        );
    }
    fn add_button(&self, id: &'static str, secondary: bool, ui: &mut Ui<Self>) -> Stateful<Div> {
        let button = Button::new(id, "New Agent")
            .icon(Icon::Plus)
            .enabled(!self.pending.busy());
        if secondary {
            button.secondary()
        } else {
            button.primary()
        }
        .build(ui, Self::open_add)
    }
    fn agent_row(
        &self,
        index: usize,
        count: usize,
        agent: &ainc_client::types::Assignee,
    ) -> Stateful<Div> {
        let name = agent.name.clone();
        let assigned = self
            .state
            .tickets
            .iter()
            .filter(|t| {
                t.assignee_id == agent.id
                    && !matches!(t.status, TicketStatus::Done | TicketStatus::Cancelled)
            })
            .count();
        let running = self.state.runs.iter().any(|r| {
            is_active(&r.state)
                && self
                    .state
                    .tickets
                    .iter()
                    .any(|t| t.id == r.ticket_id && t.assignee_id == agent.id)
        });
        let subtitle = copy::pluralize(assigned, "open Ticket", "open Tickets");
        list_item(
            SharedString::from(format!("agent.{}", agent.id)),
            name.clone(),
        )
        .accessibility_id(format!("agent.{}", agent.id))
        .min_h(px(LIST_ROW_HEIGHT))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .gap(px(SPACE_3))
        .when(index + 1 < count, |s| {
            s.border_b_1().border_color(rgb(BORDER_SUBTLE))
        })
        .child(assignee_avatar(
            &AssigneeFace::agent(name.clone()),
            AVATAR_SIZE,
        ))
        .child(
            column()
                .flex_1()
                .min_w_0()
                .gap(px(SPACE_HALF))
                .child(div().text_size(type_size(BODY_SIZE)).child(name.clone()))
                .child(caption(subtitle)),
        )
        .child(if running {
            badge("Running", Tone::Info)
        } else {
            badge("Idle", Tone::Neutral)
        })
    }
}

impl Page for AgentsPage {
    const ROUTE: Route = Route::Agents;
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.overlays.active()?;
        let ui = &mut Ui::new(window, cx);
        let body = column_gap(FORM_STACK_GAP)
            .child(text_field("Name", self.name.clone(), ui))
            .child(
                Field::new(self.instructions.clone())
                    .label("Instructions")
                    .multiline()
                    .build(ui),
            )
            .child(
                Field::new(self.model.clone())
                    .label("Model")
                    .hint("Leave empty to use the connection default.")
                    .build(ui),
            )
            .when_some(self.form_error.clone(), |s, error| {
                s.child(error_text(error))
            });
        let footer = DialogFooter::new(Verb::Create)
            .ids("agents.cancel", "agents.submit")
            .enabled(!self.name.read(ui.cx).content.trim().is_empty())
            .pending(self.pending.busy())
            .focus(&self.cancel_focus, &self.submit_focus)
            .build(
                ui,
                |this: &mut Self, window, cx| {
                    this.overlays.dismiss(window, cx);
                    cx.notify();
                },
                |this: &mut Self, _, cx| this.register(cx),
            );
        Some(dialog_shell("New Agent", body, footer).into_any_element())
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let mut handles: Vec<FocusHandle> = self
            .inputs()
            .into_iter()
            .map(|(_, input)| input.focus_handle(cx))
            .collect();
        handles.extend([self.cancel_focus.clone(), self.submit_focus.clone()]);
        handles
    }
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        anyhow::ensure!(
            !self.pending.busy(),
            "Wait for the current change to finish before installing"
        );
        let mut drafts = Drafts::default();
        for (key, input) in self.inputs() {
            drafts.text(key, input, cx);
        }
        Ok(drafts)
    }
    fn restore(&mut self, drafts: Drafts, cx: &mut Context<Self>) {
        for (key, input) in self.inputs() {
            drafts.restore_text(key, input, cx);
        }
    }
}

impl Render for AgentsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let agents: Vec<_> = self
            .state
            .assignees
            .iter()
            .filter(|a| a.kind == AssigneeKind::Agent)
            .cloned()
            .collect();
        let sync = self.sync.read(cx);
        let state = sync.load_state();
        let fetching = sync.fetching();
        let count = agents.len();
        let ui = &mut Ui::new(window, cx);
        PageFrame::document(
            PageHeader::new(self.title())
                .description("Agents pick up the Tickets you assign to them.")
                .actions(
                    row_gap(CONTROL_GAP)
                        .child(
                            Button::new("agents.refresh", "Refresh")
                                .icon(Icon::Refresh)
                                .icon_only()
                                .secondary()
                                .enabled(!fetching)
                                .build(ui, |this, _, cx| {
                                    this.sync.update(cx, |sync, cx| sync.wake(cx))
                                }),
                        )
                        .child(self.add_button("agents.create", false, ui)),
                ),
        )
        .child(column().gap(px(SPACE_4)).children(page_frame(
            "agents",
            &state,
            SKELETON_ROWS,
            ui,
            |ui| {
                if agents.is_empty() {
                    return match state.error {
                        Some(_) => div().into_any_element(),
                        None => EmptyState::new(Icon::Agents, "No Agents yet.")
                            .description("Register an agent with instructions and a model, then assign Tickets to it.")
                            .selector("agents.empty")
                            .action(self.add_button("agents.create.empty", true, ui))
                            .build()
                            .into_any_element(),
                    };
                }
                card()
                    .p(px(SPACE_1))
                    .gap_0()
                    .children(
                        agents
                            .iter()
                            .enumerate()
                            .map(|(index, agent)| self.agent_row(index, count, agent)),
                    )
                    .into_any_element()
            },
        )))
        .build()
    }
}
