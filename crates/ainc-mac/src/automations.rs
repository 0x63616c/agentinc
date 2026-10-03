//! The Automations page: the saved Automations that start agent work, and the form that edits them.
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
    Assignee, AssigneeKind, Automation, AutomationCommand, AutomationSnapshot, TicketProposal,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// The one dialog this page can have open: the Automation editor.
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
    /// The registered agents the editor offers, read with the snapshot.
    agents: Vec<Assignee>,
    /// Validation of the editor's fields, shown under them.
    minutes_error: Option<&'static str>,
    agent_error: Option<&'static str>,
    name: Entity<TextInput>,
    prompt: Entity<TextInput>,
    minutes: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Destination> for AutomationsPage {}
fn automation_state(automation: &Automation) -> (&'static str, Tone) {
    if automation.error.is_some() {
        ("Needs attention", Tone::Danger)
    } else if automation.revision != automation.applied_revision {
        ("Pending", Tone::Warning)
    } else if automation.paused {
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
        let name = cx.new(|cx| {
            TextInput::field("Automation name", false, cx).identified("automations.name")
        });
        let prompt = cx.new(|cx| {
            TextInput::field("Ticket prompt", false, cx).identified("automations.prompt")
        });
        let minutes =
            cx.new(|cx| TextInput::field("Minutes", false, cx).identified("automations.minutes"));
        let subscriptions = vec![
            cx.observe(&name, |_, _, cx| cx.notify()),
            cx.observe(&prompt, |_, _, cx| cx.notify()),
            cx.observe(&minutes, |_, _, cx| cx.notify()),
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, event: &SliceChanged, cx| {
                if matches!(*event, SliceChanged::Automations | SliceChanged::Tickets) {
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
            agents: vec![],
            minutes_error: None,
            agent_error: None,
            name,
            prompt,
            minutes,
            _subscriptions: subscriptions,
        };
        this.reload();
        this
    }
    pub(crate) fn reload(&mut self) {
        self.state = self.daemon.automations();
        self.agents = self
            .daemon
            .tickets()
            .assignees
            .iter()
            .filter(|a| a.kind == AssigneeKind::Agent)
            .cloned()
            .collect();
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
    fn edit(
        &mut self,
        automation: Option<Automation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        self.minutes_error = None;
        self.agent_error = None;
        self.selected = automation.as_ref().map(|r| r.id.clone());
        self.editing_revision = automation.as_ref().map(|r| r.revision);
        self.agent = automation.as_ref().map(|r| r.agent_id.clone());
        self.name.update(cx, |input, cx| {
            input.set_text(automation.as_ref().map_or("", |r| r.name.as_str()), cx)
        });
        self.prompt.update(cx, |input, cx| {
            input.set_text(automation.as_ref().map_or("", |r| r.prompt.as_str()), cx)
        });
        self.minutes.update(cx, |input, cx| {
            input.set_text(
                &automation
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
        let every_minutes = self.minutes.read(cx).content.trim().parse::<i64>();
        self.minutes_error = (!every_minutes.as_ref().is_ok_and(|m| *m > 0))
            .then_some("Enter a whole number of minutes.");
        self.agent_error = self
            .agent
            .is_none()
            .then_some("Choose an agent. Register one on the Agents page first.");
        let (Ok(every_minutes), None, None, Some(agent_id)) = (
            every_minutes,
            self.minutes_error,
            self.agent_error,
            self.agent.clone(),
        ) else {
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
    fn dialog(&self, ui: &mut Ui<Self>) -> Option<AnyElement> {
        self.overlays.active()?;
        let agents = self.agents.clone();
        let enabled = !self.pending.busy();
        let valid = !self.name.read(ui.cx).content.trim().is_empty()
            && !self.prompt.read(ui.cx).content.trim().is_empty()
            && self
                .minutes
                .read(ui.cx)
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
                .child(text_field("Name", self.name.clone(), ui))
                .child(
                    Field::new(self.prompt.clone())
                        .label("Ticket prompt")
                        .multiline()
                        .hint("Each firing creates a Ticket with this prompt and assigns it.")
                        .build(ui),
                )
                .child(
                    div().w(px(SHORT_FIELD_WIDTH)).child(
                        Field::new(self.minutes.clone())
                            .label("Repeat every")
                            .selector("Every (minutes)")
                            .suffix("min")
                            .error(self.minutes_error)
                            .build(ui),
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
                                    .width(SELECT_WIDTH)
                                    .build(ui, |_, _, _| {}, |_, _, _, _| {}),
                            )
                            .child(caption("Register an agent on the Agents page first."))
                        })
                        .child(row().gap(px(CHIP_GAP)).flex_wrap().children(
                            agents.into_iter().map(|a| {
                                let id = a.id.clone();
                                let selected = self.agent.as_ref() == Some(&a.id);
                                Chip::new(
                                    SharedString::from(format!("automations.agent.{}", a.id)),
                                    a.name,
                                )
                                .selected(selected)
                                .enabled(enabled)
                                .build(ui, move |this, _, cx| {
                                    this.agent = Some(id.clone());
                                    cx.notify();
                                })
                            }),
                        ))
                        .when_some(self.agent_error, |s, error| s.child(error_text(error))),
                )
                .when_some(self.error.clone(), |s, error| s.child(error_text(error)));
        let footer = DialogFooter::new(if self.selected.is_some() {
            Verb::Save
        } else {
            Verb::Create
        })
        .ids("automations.cancel", "automations.save")
        .enabled(valid)
        .pending(!enabled)
        .focus(&self.cancel_focus, &self.submit_focus)
        .build(ui, close, |this, _, cx| this.save(cx));
        Some(dialog_shell(title, body, footer).into_any_element())
    }

    fn detail_header(&self, automation: &Automation, ui: &mut Ui<Self>) -> PageHeader {
        let edit = automation.clone();
        let pause = automation.clone();
        let run = automation.clone();
        let (state, _) = automation_state(automation);
        let enabled = !self.pending.busy();
        PageHeader::new(automation.name.clone())
            .leading(
                Button::new("automations.back", "Automations")
                    .ghost()
                    .small()
                    .icon(Icon::ChevronLeft)
                    .tint(TEXT_SECONDARY)
                    .build(ui, |this, _, cx| {
                        this.selected = None;
                        cx.notify();
                    })
                    .ml(px(-CONTROL_INSET_X_SM)),
            )
            .description(format!("{} · {state}", every(automation.every_minutes)))
            .actions(
                row_gap(CONTROL_GAP)
                    .child(
                        Button::new("automations.edit", "Edit")
                            .secondary()
                            .icon(Icon::Edit)
                            .enabled(enabled)
                            .build(ui, move |this, window, cx| {
                                this.edit(Some(edit.clone()), window, cx)
                            }),
                    )
                    .child(
                        Button::new(
                            "automations.pause",
                            if automation.paused { "Resume" } else { "Pause" },
                        )
                        .secondary()
                        .icon(if automation.paused {
                            Icon::Play
                        } else {
                            Icon::Pause
                        })
                        .enabled(enabled)
                        .build(ui, move |this, _, cx| {
                            this.command(
                                AutomationCommand::Pause {
                                    id: pause.id.clone(),
                                    revision: pause.revision,
                                    paused: !pause.paused,
                                },
                                cx,
                            )
                        }),
                    )
                    .child(
                        Button::new("automations.run", "Run Now")
                            .primary()
                            .icon(Icon::Play)
                            .enabled(enabled)
                            .build(ui, move |this, _, cx| {
                                this.command(
                                    AutomationCommand::RunNow {
                                        id: run.id.clone(),
                                        revision: run.revision,
                                    },
                                    cx,
                                )
                            }),
                    ),
            )
    }

    fn detail(&self, automation: Automation, ui: &mut Ui<Self>) -> Div {
        let (state, tone) = automation_state(&automation);
        column()
            .gap(px(SECTION_GAP))
            .child(
                card()
                    .gap(px(SPACE_4))
                    .child(
                        column()
                            .gap(px(SPACE_2))
                            .child(eyebrow("Ticket prompt"))
                            .child(div().text_size(type_size(BODY_SIZE)).child(automation.prompt.clone())),
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
                                    .child(caption(every(automation.every_minutes))),
                            )
                            .when_some(automation.error.clone(), |s, error| s.child(error_text(error))),
                    ),
            )
            .child(
                column()
                    .gap(px(SPACE_3))
                    .child(heading("History"))
                    .child(caption(
                        "Run Now creates one Occurrence in place of a missed firing; it does not replay every missed one.",
                    ))
                    .child(
                        card().p(px(SPACE_1)).gap_0().children(
                            self.state
                                .occurrences
                                .iter()
                                .filter(|o| o.automation_id == automation.id)
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
                                                    crate::tickets::model::ticket_key(id),
                                                )
                                                .ghost()
                                                .small()
                                                .trailing(icon(Icon::ArrowRight, ICON_SIZE_SM))
                                                .build(ui, move |_, _, cx| cx.emit(Destination::Ticket(id))),
                                            )
                                        })
                                }),
                        ),
                    )
                    .children(
                        self.state
                            .history
                            .iter()
                            .filter(|h| h.automation_id == automation.id)
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

    fn list(&self, ui: &mut Ui<Self>) -> Div {
        column()
            .gap(px(SPACE_HALF))
            .children(self.state.rules.iter().map(|r| {
                let id = r.id.clone();
                let (state, tone) = automation_state(r);
                ListRow::new(
                    SharedString::from(format!("automation.{}", r.id)),
                    r.name.clone(),
                )
                .leading(icon(Route::Automations.icon(), ICON_SIZE))
                .subtitle(every(r.every_minutes))
                .trailing(
                    row()
                        .gap(px(SPACE_2))
                        .child(badge(state, tone))
                        .child(icon(Icon::ChevronRight, ICON_SIZE_SM)),
                )
                .build(ui, move |this, _, cx| {
                    this.selected = Some(id.clone());
                    cx.notify();
                })
            }))
    }
}
impl Render for AutomationsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let create = |this: &Self, id: &'static str, ui: &mut Ui<Self>| {
            Button::new(id, "New Automation")
                .primary()
                .icon(Icon::Plus)
                .enabled(!this.pending.busy())
                .build(ui, |this, window, cx| this.edit(None, window, cx))
        };
        let sync = self.sync.read(cx);
        let (mut state, fetching) = (sync.load_state(), sync.fetching());
        let ui = &mut Ui::new(window, cx);
        let header = match &selected {
            Some(automation) => self.detail_header(automation, ui),
            None => PageHeader::new(self.title())
                .description("Automations that create and assign Tickets on a schedule.")
                .actions(
                    row_gap(CONTROL_GAP)
                        .child(
                            Button::new("automations.refresh", "Refresh")
                                .icon(Icon::Refresh)
                                .icon_only()
                                .secondary()
                                .enabled(!fetching)
                                .build(ui, |this, _, cx| {
                                    this.sync.update(cx, |sync, cx| sync.wake(cx))
                                }),
                        )
                        .child(
                            create(self, "automations.create", ui)
                                .debug_selector(|| "automations.create".into()),
                        ),
                ),
        };
        // A change that failed shows in the same banner as a load that did.
        let load_failed = state.error.is_some();
        state.error = state.error.take().or(self
            .error
            .clone()
            .filter(|_| self.overlays.active().is_none()));
        let content = column().gap(px(SECTION_GAP)).w_full().children(page_frame(
            "automations",
            &state,
            SKELETON_ROWS,
            ui,
            |ui| {
                if let Some(automation) = selected {
                    return self.detail(automation, ui).into_any_element();
                }
                column()
                    .gap(px(SECTION_GAP))
                    .when(self.state.rules.is_empty() && !load_failed, |s| {
                        s.child(
                            EmptyState::new(Icon::Repeat, "No Automations yet.")
                                .description(
                                    "An Automation creates and assigns a Ticket to an agent on a schedule.",
                                )
                                .selector("automations.empty")
                                .action(
                                    Button::new("automations.create.empty", "New Automation")
                                        .secondary()
                                        .icon(Icon::Plus)
                                        .enabled(!self.pending.busy())
                                        .build(ui, |this, window, cx| this.edit(None, window, cx)),
                                )
                                .build(),
                        )
                    })
                    .child(self.list(ui))
                    .into_any_element()
            },
        ));
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
        self.dialog(&mut Ui::new(window, cx))
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

#[cfg(test)]
mod tests {
    use super::{automation_state, every};
    use ainc_client::types::Automation;

    fn automation() -> Automation {
        Automation {
            agent_id: "agent".into(),
            applied_revision: 1,
            error: None,
            every_minutes: 5,
            id: "a".into(),
            missed: 0,
            name: "Nightly".into(),
            overlap_skipped: 0,
            paused: false,
            prompt: "Go".into(),
            revision: 1,
        }
    }

    #[test]
    fn state_prefers_errors_then_pending_then_paused() {
        let mut rule = automation();
        assert_eq!(automation_state(&rule).0, "Active");
        rule.paused = true;
        assert_eq!(automation_state(&rule).0, "Paused");
        rule.revision = 2;
        assert_eq!(automation_state(&rule).0, "Pending");
        rule.error = Some("boom".into());
        assert_eq!(automation_state(&rule).0, "Needs attention");
    }

    #[test]
    fn every_pluralises_minutes() {
        assert_eq!(every(1), "Every 1 minute");
        assert_eq!(every(15), "Every 15 minutes");
    }
}
