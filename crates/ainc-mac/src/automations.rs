//! The Automations page: the saved rules that start agent work, and the form that edits them.
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
use ainc_client::types::{
    AssigneeKind, Automation, AutomationCommand, AutomationSnapshot, TicketProposal,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// The one dialog this page can have open: the rule editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
    Edit,
}

pub struct AutomationsPage {
    daemon: Arc<Daemon>,
    sync: Entity<Sync>,
    state: AutomationSnapshot,
    error: Option<String>,
    pending: Pending,
    overlays: PageOverlays<Dialog>,
    /// Reopen the editor at the next render: an update install closed it.
    reopen: bool,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    selected: Option<String>,
    editing_revision: Option<i64>,
    page_focus: FocusHandle,
    restore_focus: bool,
    agent: Option<String>,
    name: Entity<TextInput>,
    prompt: Entity<TextInput>,
    minutes: Entity<TextInput>,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Destination> for AutomationsPage {}
impl HoverHost for AutomationsPage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}
fn rule_state(rule: &Automation) -> (&'static str, Tone) {
    if rule.error.is_some() {
        ("Needs attention", Tone::Danger)
    } else if rule.revision != rule.applied_revision {
        ("Pending", Tone::Warning)
    } else if rule.paused {
        ("Paused", Tone::Neutral)
    } else {
        ("Active", Tone::Success)
    }
}
fn every(minutes: i64) -> String {
    format!(
        "Every {minutes} {}",
        if minutes == 1 { "minute" } else { "minutes" }
    )
}
impl AutomationsPage {
    pub fn new(
        daemon: Arc<Daemon>,
        sync: Entity<Sync>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let name =
            cx.new(|cx| TextInput::field("Rule name", false, cx).identified("automations.name"));
        let prompt = cx.new(|cx| {
            TextInput::field("What should the agent do?", false, cx)
                .identified("automations.prompt")
        });
        let minutes =
            cx.new(|cx| TextInput::field("30", false, cx).identified("automations.minutes"));
        let subscriptions = vec![
            cx.observe(&name, |_, _, cx| cx.notify()),
            cx.observe(&prompt, |_, _, cx| cx.notify()),
            cx.observe(&minutes, |_, _, cx| cx.notify()),
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, event: &SliceChanged, cx| {
                if *event == SliceChanged::Automations {
                    this.reload();
                    cx.notify();
                }
            }),
        ];
        let mut this = Self {
            daemon,
            sync,
            state: AutomationSnapshot {
                rules: vec![],
                occurrences: vec![],
                history: vec![],
            },
            error: None,
            pending: Pending::default(),
            overlays: PageOverlays::new(overlays, Route::Automations),
            reopen: false,
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            selected: None,
            editing_revision: None,
            page_focus: cx.focus_handle(),
            restore_focus: false,
            agent: None,
            name,
            prompt,
            minutes,
            hover: HoverFade::default(),
            _subscriptions: subscriptions,
        };
        this.reload();
        this
    }
    pub(crate) fn reload(&mut self) {
        self.state = self.daemon.automations();
    }
    fn command(&mut self, command: AutomationCommand, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        cx.run(
            &self.pending.clone(),
            move || Ok(daemon.send(command)?),
            |this, result, _| match result {
                Ok(id) => {
                    this.reload();
                    this.error = None;
                    this.overlays.close();
                    this.restore_focus = true;
                    if this.state.rules.iter().any(|r| r.id == id) {
                        this.selected = Some(id);
                    }
                }
                Err(failure) => this.error = Some(failure.message("The Automation change")),
            },
        );
    }
    fn edit(&mut self, rule: Option<Automation>, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.selected = rule.as_ref().map(|r| r.id.clone());
        self.editing_revision = rule.as_ref().map(|r| r.revision);
        self.agent = rule.as_ref().map(|r| r.agent_id.clone());
        self.name.update(cx, |input, cx| {
            input.set_text(rule.as_ref().map_or("", |r| r.name.as_str()), cx)
        });
        self.prompt.update(cx, |input, cx| {
            input.set_text(rule.as_ref().map_or("", |r| r.prompt.as_str()), cx)
        });
        self.minutes.update(cx, |input, cx| {
            input.set_text(
                &rule
                    .as_ref()
                    .map_or("30".to_owned(), |r| r.every_minutes.to_string()),
                cx,
            )
        });
        let focus = self.name.focus_handle(cx);
        self.overlays.open_dialog(Dialog::Edit, focus, window, cx);
        cx.notify();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        let Ok(every_minutes) = self.minutes.read(cx).content.trim().parse::<i64>() else {
            self.error = Some("Enter an interval in whole minutes.".into());
            cx.notify();
            return;
        };
        let Some(agent_id) = self.agent.clone() else {
            self.error = Some("Choose an agent. Register one on the Agents page first.".into());
            cx.notify();
            return;
        };
        self.command(
            AutomationCommand::Save {
                id: self.selected.clone(),
                revision: self.editing_revision,
                name: self.name.read(cx).content.to_string(),
                proposal: TicketProposal {
                    title: self.prompt.read(cx).content.to_string(),
                    agent_id,
                },
                every_minutes,
            },
            cx,
        );
    }
    fn dialog(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.overlays.active()?;
        let agents: Vec<_> = self
            .daemon
            .tickets()
            .assignees
            .into_iter()
            .filter(|a| a.kind == AssigneeKind::Agent)
            .collect();
        let enabled = !self.pending.busy();
        let valid = !self.name.read(cx).content.trim().is_empty()
            && !self.prompt.read(cx).content.trim().is_empty()
            && self
                .minutes
                .read(cx)
                .content
                .trim()
                .parse::<i64>()
                .is_ok_and(|m| m > 0)
            && self.agent.is_some();
        let close = |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            this.overlays.dismiss(window, cx);
            cx.notify();
        };
        let title = if self.selected.is_some() {
            "Edit Automation"
        } else {
            "New Automation"
        };
        let body =
            column_gap(FORM_STACK_GAP)
                .child(text_field("Name", self.name.clone(), window, cx))
                .child(
                    Field::new(self.prompt.clone())
                        .label("Ticket prompt")
                        .multiline()
                        .hint("Each firing creates a Ticket with this prompt and assigns it.")
                        .build(window, cx),
                )
                .child(
                    div().w(px(180.)).child(
                        Field::new(self.minutes.clone())
                            .label("Repeat every")
                            .selector("Every (minutes)")
                            .suffix("min")
                            .build(window, cx),
                    ),
                )
                .child(
                    column()
                        .gap(px(SPACE_2))
                        .child(
                            div()
                                .text_size(type_size(LABEL_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(TEXT_SECONDARY))
                                .child("Assign to"),
                        )
                        .when(agents.is_empty(), |s| {
                            s.child(
                                Select::new("automations.agent", vec![])
                                    .placeholder("No agents yet")
                                    .enabled(false)
                                    .width(240.)
                                    .build(&self.hover, |_, _, _| {}, |_, _, _, _| {}, cx),
                            )
                            .child(caption("Register an agent on the Agents page first."))
                        })
                        .child(row().gap(px(CHIP_GAP)).flex_wrap().children(
                            agents.into_iter().map(|a| {
                                let id = a.id.clone();
                                let selected = self.agent.as_ref() == Some(&a.id);
                                chip(
                                    SharedString::from(format!("automations.agent.{}", a.id)),
                                    a.name,
                                    selected,
                                    enabled,
                                    &self.hover,
                                    move |this, _, cx| {
                                        this.agent = Some(id.clone());
                                        cx.notify();
                                    },
                                    cx,
                                )
                            }),
                        )),
                )
                .when_some(self.error.clone(), |s, error| s.child(error_text(error)));
        let footer = DialogFooter::new(Verb::Save)
            .label("Save Automation")
            .ids("automations.cancel", "automations.save")
            .enabled(valid)
            .pending(!enabled)
            .focus(&self.cancel_focus, &self.submit_focus)
            .build(&self.hover, close, |this, _, cx| this.save(cx), cx);
        Some(dialog_shell(title, body, footer).into_any_element())
    }

    fn detail_header(&self, rule: &Automation, cx: &mut Context<Self>) -> PageHeader {
        let edit = rule.clone();
        let pause = rule.clone();
        let run = rule.clone();
        let (state, _) = rule_state(rule);
        let enabled = !self.pending.busy();
        PageHeader::new(rule.name.clone())
            .leading(
                Button::new("automations.back", "Automations")
                    .ghost()
                    .small()
                    .icon("chevronLeft")
                    .tint(TEXT_SECONDARY)
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
            .description(format!("{} · {state}", every(rule.every_minutes)))
            .actions(
                row_gap(CONTROL_GAP)
                    .child(
                        Button::new("automations.edit", "Edit")
                            .secondary()
                            .icon("edit")
                            .enabled(enabled)
                            .build(
                                &self.hover,
                                move |this, window, cx| this.edit(Some(edit.clone()), window, cx),
                                cx,
                            ),
                    )
                    .child(
                        Button::new(
                            "automations.pause",
                            if rule.paused { "Resume" } else { "Pause" },
                        )
                        .secondary()
                        .icon(if rule.paused { "play" } else { "pause" })
                        .enabled(enabled)
                        .build(
                            &self.hover,
                            move |this, _, cx| {
                                this.command(
                                    AutomationCommand::Pause {
                                        id: pause.id.clone(),
                                        revision: pause.revision,
                                        paused: !pause.paused,
                                    },
                                    cx,
                                )
                            },
                            cx,
                        ),
                    )
                    .child(
                        Button::new("automations.run", "Run now")
                            .primary()
                            .icon("play")
                            .enabled(enabled)
                            .build(
                                &self.hover,
                                move |this, _, cx| {
                                    this.command(
                                        AutomationCommand::RunNow {
                                            id: run.id.clone(),
                                            revision: run.revision,
                                        },
                                        cx,
                                    )
                                },
                                cx,
                            ),
                    ),
            )
    }

    fn detail(&self, rule: Automation, cx: &mut Context<Self>) -> Div {
        let (state, tone) = rule_state(&rule);
        column()
            .gap(px(SECTION_GAP))
            .child(
                card()
                    .gap(px(SPACE_4))
                    .child(
                        column()
                            .gap(px(SPACE_2))
                            .child(eyebrow("Ticket prompt"))
                            .child(div().text_size(type_size(BODY_SIZE)).child(rule.prompt.clone())),
                    )
                    .child(divider())
                    .child(
                        column()
                            .gap(px(SPACE_2))
                            .child(eyebrow("Schedule"))
                            .child(
                                row()
                                    .gap(px(SPACE_3))
                                    .child(status_pill(state, tone))
                                    .child(caption(every(rule.every_minutes))),
                            )
                            .when_some(rule.error.clone(), |s, error| s.child(error_text(error))),
                    ),
            )
            .child(
                column()
                    .gap(px(SPACE_3))
                    .child(heading("History"))
                    .child(caption(
                        "Run now deliberately replaces a missed firing; it does not replay all missed work.",
                    ))
                    .child(
                        card().p(px(SPACE_1)).gap_0().children(
                            self.state
                                .occurrences
                                .iter()
                                .filter(|o| o.automation_id == rule.id)
                                .map(|o| {
                                    let state = state_label(&o.state);
                                    let tone = state_tone(&o.state);
                                    row()
                                        .w_full()
                                        .min_h(px(LIST_ROW_HEIGHT))
                                        .px(px(SPACE_3))
                                        .gap(px(SPACE_3))
                                        .border_b_1()
                                        .border_color(rgb(BORDER_SUBTLE))
                                        .child(status_pill(state, tone))
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(type_size(LABEL_SIZE))
                                                .child(time::absolute(o.scheduled_at)),
                                        )
                                        .when_some(o.ticket_id, |s, id| {
                                            s.child(
                                                Button::new(
                                                    SharedString::from(format!(
                                                        "automations.ticket.{id}"
                                                    )),
                                                    format!("Ticket {id}"),
                                                )
                                                .ghost()
                                                .small()
                                                .trailing(icon("arrowRight", ICON_SIZE_SM))
                                                .build(
                                                    &self.hover,
                                                    move |_, _, cx| cx.emit(Destination::Ticket(id)),
                                                    cx,
                                                ),
                                            )
                                        })
                                }),
                        ),
                    )
                    .children(
                        self.state
                            .history
                            .iter()
                            .filter(|h| h.automation_id == rule.id)
                            .map(|h| {
                                caption(format!(
                                    "{} · {} {}",
                                    time::absolute(h.observed_at),
                                    h.count,
                                    h.kind.replace('_', " ")
                                ))
                            }),
                    ),
            )
    }

    fn list(&self, cx: &mut Context<Self>) -> Div {
        column()
            .gap(px(SPACE_HALF))
            .children(self.state.rules.iter().map(|r| {
                let id = r.id.clone();
                let (state, tone) = rule_state(r);
                ListRow::new(
                    SharedString::from(format!("automations.rule.{}", r.id)),
                    r.name.clone(),
                )
                .leading(icon("refresh", ICON_SIZE))
                .subtitle(every(r.every_minutes))
                .trailing(
                    row()
                        .gap(px(SPACE_2))
                        .child(badge(state, tone))
                        .child(icon("chevronRight", ICON_SIZE_SM)),
                )
                .build(
                    &self.hover,
                    move |this, _, cx| {
                        this.selected = Some(id.clone());
                        cx.notify();
                    },
                    cx,
                )
            }))
    }
}
impl Render for AutomationsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        if std::mem::take(&mut self.reopen) {
            let focus = self.name.focus_handle(cx);
            self.overlays.open_dialog(Dialog::Edit, focus, window, cx);
        }
        if self.restore_focus {
            window.focus(&self.page_focus, cx);
            self.restore_focus = false;
        }
        let selected = self
            .state
            .rules
            .iter()
            .find(|r| Some(&r.id) == self.selected.as_ref())
            .cloned();
        let create = |this: &Self, id: &'static str, cx: &mut Context<Self>| {
            Button::new(id, "New Automation")
                .primary()
                .icon("plus")
                .enabled(!this.pending.busy())
                .build(
                    &this.hover,
                    |this, window, cx| this.edit(None, window, cx),
                    cx,
                )
        };
        let sync = self.sync.read(cx);
        let (loaded, load_error, reconnecting, loading_started, fetching) = (
            sync.loaded,
            sync.message(),
            sync.reconnecting(),
            sync.loading_started,
            sync.fetching(),
        );
        let header = match &selected {
            Some(rule) => self.detail_header(rule, cx),
            None => PageHeader::new(self.title())
                .description("Recurring rules that create and assign Tickets on a schedule.")
                .actions(
                    row_gap(CONTROL_GAP)
                        .child(
                            Button::new("automations.refresh", "Refresh")
                                .icon("refresh")
                                .icon_only()
                                .secondary()
                                .enabled(!fetching)
                                .build(
                                    &self.hover,
                                    |this, _, cx| this.sync.update(cx, |sync, cx| sync.wake(cx)),
                                    cx,
                                ),
                        )
                        .child(
                            create(self, "automations.create", cx)
                                .debug_selector(|| "automations.create".into()),
                        ),
                ),
        };
        let mut content = column().gap(px(SECTION_GAP)).w_full();
        let dialog_open = self.overlays.active().is_some();
        if let Some(error) = load_error
            .as_ref()
            .or(self.error.as_ref().filter(|_| !dialog_open))
        {
            content = content.child(
                banner(Tone::Danger, error.clone())
                    .id("automations.error")
                    .accessibility_id("automations.error"),
            );
        }
        if reconnecting {
            content =
                content.child(LoadingFrame::new(loading_started, window).inline("Reconnecting…"));
        }
        if !loaded && load_error.is_none() {
            content = content.child(skeleton_rows("automations.loading", 3));
        }
        if let Some(rule) = selected {
            content = content.child(self.detail(rule, cx));
        } else {
            if loaded && self.state.rules.is_empty() && load_error.is_none() {
                content = content.child(
                    EmptyState::new("repeat", "No Automations yet")
                        .description(
                            "An Automation creates and assigns a Ticket to an agent on a schedule.",
                        )
                        .selector("automations.empty")
                        .action(
                            Button::new("automations.create.empty", "New Automation")
                                .secondary()
                                .icon("plus")
                                .enabled(!self.pending.busy())
                                .build(
                                    &self.hover,
                                    |this, window, cx| this.edit(None, window, cx),
                                    cx,
                                ),
                        )
                        .build(),
                );
            }
            content = content.child(self.list(cx));
        }
        PageFrame::document(header)
            .child(
                div()
                    .id("automations.page")
                    .debug_selector(|| "automations.page".into())
                    .track_focus(&self.page_focus)
                    .accessibility_id("automations.page")
                    .w_full()
                    .child(content),
            )
            .build()
    }
}

impl Page for AutomationsPage {
    const ROUTE: Route = Route::Automations;
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.dialog(window, cx)
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        vec![
            self.name.focus_handle(cx),
            self.prompt.focus_handle(cx),
            self.minutes.focus_handle(cx),
            self.cancel_focus.clone(),
            self.submit_focus.clone(),
        ]
    }
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        anyhow::ensure!(
            !self.pending.busy(),
            "Wait for the current change to finish before installing"
        );
        let mut drafts = Drafts::default();
        drafts.set("editing", self.overlays.active().is_some());
        drafts.set("selected", &self.selected);
        drafts.set("editing_revision", self.editing_revision);
        drafts.set("agent", &self.agent);
        drafts.text("name", &self.name, cx);
        drafts.text("prompt", &self.prompt, cx);
        drafts.text("minutes", &self.minutes, cx);
        Ok(drafts)
    }
    fn restore(&mut self, drafts: Drafts, cx: &mut Context<Self>) {
        self.reopen = drafts.get("editing") == Some(true);
        if let Some(selected) = drafts.get("selected") {
            self.selected = selected;
        }
        if let Some(revision) = drafts.get("editing_revision") {
            self.editing_revision = revision;
        }
        if let Some(agent) = drafts.get("agent") {
            self.agent = agent;
        }
        drafts.restore_text("name", &self.name, cx);
        drafts.restore_text("prompt", &self.prompt, cx);
        drafts.restore_text("minutes", &self.minutes, cx);
    }
}
