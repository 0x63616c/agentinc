use crate::{
    input::TextInput,
    storage::{
        AssigneeKind, Automation, AutomationCommand, AutomationSnapshot, Store, TicketProposal,
    },
    ui::*,
};
use gpui::{prelude::*, *};
use std::{sync::Arc, time::Instant};

fn display_time(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%b %-d, %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "Time unavailable".into())
}
pub struct OpenTicket(pub i64);
pub struct AutomationsPage {
    store: Option<Arc<Store>>,
    state: AutomationSnapshot,
    error: Option<String>,
    pending: bool,
    refreshing: bool,
    loaded: bool,
    loading_started: Instant,
    editing: bool,
    selected: Option<String>,
    editing_revision: Option<i64>,
    page_focus: FocusHandle,
    restore_focus: bool,
    focus_editor: bool,
    agent: Option<String>,
    name: Entity<TextInput>,
    prompt: Entity<TextInput>,
    minutes: Entity<TextInput>,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<OpenTicket> for AutomationsPage {}
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
    pub(crate) fn update_drafts(&self, cx: &App) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(
            !self.pending,
            "Wait for the current change to finish before installing"
        );
        Ok(
            serde_json::json!({"editing":self.editing,"selected":self.selected,"editing_revision":self.editing_revision,"agent":self.agent,"name": self.name.read(cx).content.to_string(), "prompt": self.prompt.read(cx).content.to_string(), "minutes": self.minutes.read(cx).content.to_string()}),
        )
    }
    pub(crate) fn restore_update_drafts(
        &mut self,
        value: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if let Ok(value) = serde_json::from_value(value["editing"].clone()) {
            self.editing = value;
        }
        if let Ok(value) = serde_json::from_value(value["selected"].clone()) {
            self.selected = value;
        }
        if let Ok(value) = serde_json::from_value(value["editing_revision"].clone()) {
            self.editing_revision = value;
        }
        if let Ok(value) = serde_json::from_value(value["agent"].clone()) {
            self.agent = value;
        }
        if let Some(text) = value["name"].as_str() {
            self.name.update(cx, |input, cx| input.set_text(text, cx));
        }
        if let Some(text) = value["prompt"].as_str() {
            self.prompt.update(cx, |input, cx| input.set_text(text, cx));
        }
        if let Some(text) = value["minutes"].as_str() {
            self.minutes
                .update(cx, |input, cx| input.set_text(text, cx));
        }
    }

    pub fn new(store: Option<Arc<Store>>, error: Option<String>, cx: &mut Context<Self>) -> Self {
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
        ];
        let mut this = Self {
            store,
            state: AutomationSnapshot {
                rules: vec![],
                occurrences: vec![],
                history: vec![],
            },
            error,
            pending: false,
            refreshing: false,
            loaded: cfg!(test),
            loading_started: Instant::now(),
            editing: false,
            selected: None,
            editing_revision: None,
            page_focus: cx.focus_handle(),
            restore_focus: false,
            focus_editor: false,
            agent: None,
            name,
            prompt,
            minutes,
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
            self.state = store.automations();
        }
    }
    pub(crate) fn workspace_changed(&mut self, cx: &mut Context<Self>) {
        self.editing = false;
        self.selected = None;
        self.editing_revision = None;
        self.agent = None;
        for input in [&self.name, &self.prompt, &self.minutes] {
            input.update(cx, |input, _| input.reset());
        }
        self.reload();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.pending || self.refreshing {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        if self.error.is_some() {
            self.loading_started = Instant::now();
        }
        self.refreshing = true;
        let work = cx
            .background_executor()
            .spawn(async move { store.refresh() });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.refreshing = false;
                match result {
                    Ok(()) => {
                        this.reload();
                        this.loaded = true;
                        this.error = None;
                    }
                    Err(e) => this.error = Some(format!("Automations unavailable: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn command(&mut self, command: AutomationCommand, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        self.pending = true;
        let work = cx
            .background_executor()
            .spawn(async move { store.automation_command(command) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                match result {
                    Ok(id) => {
                        this.reload();
                        this.error = None;
                        this.editing = false;
                        this.restore_focus = true;
                        if this.state.rules.iter().any(|r| r.id == id) {
                            this.selected = Some(id);
                        }
                    }
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn edit(&mut self, rule: Option<Automation>, cx: &mut Context<Self>) {
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
        self.editing = true;
        self.focus_editor = true;
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
    fn editor(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let agents = self
            .store
            .as_ref()
            .map(|s| s.tickets().assignees)
            .unwrap_or_default();
        let agents: Vec<_> = agents
            .into_iter()
            .filter(|a| a.kind == AssigneeKind::Agent)
            .collect();
        let enabled = !self.pending;
        card()
            .p(px(SPACE_5))
            .gap(px(FORM_STACK_GAP))
            .child(heading(if self.selected.is_some() {
                "Edit rule"
            } else {
                "New rule"
            }))
            .child(text_field("Name", self.name.clone(), window, cx))
            .child(
                Field::new(self.prompt.clone())
                    .label("Ticket prompt")
                    .multiline()
                    .hint("Each firing creates a Ticket with this prompt.")
                    .build(window, cx),
            )
            .child(
                div().w(px(180.)).child(
                    Field::new(self.minutes.clone())
                        .label("Every (minutes)")
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
                        s.child(caption("Register an agent on the Agents page first."))
                    })
                    .child(
                        row()
                            .gap(px(CHIP_GAP))
                            .flex_wrap()
                            .children(agents.into_iter().map(|a| {
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
                            })),
                    ),
            )
            .child(caption(
                "Saving authorizes this rule to create and assign a new Ticket on each firing.",
            ))
            .child(
                row_gap(CONTROL_GAP)
                    .child(
                        Button::new("automations.save", "Save rule")
                            .primary()
                            .enabled(enabled)
                            .build(&self.hover, |this, _, cx| this.save(cx), cx),
                    )
                    .child(
                        Button::new("automations.cancel", "Cancel")
                            .secondary()
                            .build(
                                &self.hover,
                                |this, _, cx| {
                                    this.editing = false;
                                    cx.notify();
                                },
                                cx,
                            ),
                    ),
            )
    }

    fn detail(&self, rule: Automation, cx: &mut Context<Self>) -> Div {
        let edit = rule.clone();
        let pause = rule.clone();
        let run = rule.clone();
        let (state, tone) = rule_state(&rule);
        let enabled = !self.pending;
        column()
            .gap(px(SECTION_GAP))
            .child(
                row().child(
                    Button::new("automations.back", "All rules")
                        .ghost()
                        .icon("chevronLeft")
                        .build(
                            &self.hover,
                            |this, _, cx| {
                                this.selected = None;
                                cx.notify();
                            },
                            cx,
                        ),
                ),
            )
            .child(
                column()
                    .gap(px(SPACE_3))
                    .child(
                        row()
                            .gap(px(SPACE_3))
                            .child(badge(state, tone))
                            .child(caption(every(rule.every_minutes))),
                    )
                    .child(
                        div()
                            .text_size(type_size(TITLE_SIZE))
                            .line_height(relative(TITLE_LINE_HEIGHT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(rule.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(type_size(BODY_SIZE))
                            .text_color(rgb(TEXT_SECONDARY))
                            .child(rule.prompt.clone()),
                    )
                    .when_some(rule.error.clone(), |s, error| {
                        s.child(caption(error).text_color(rgb(DESTRUCTIVE_TEXT)))
                    }),
            )
            .child(
                row_gap(CONTROL_GAP)
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
                        Button::new("automations.edit", "Edit rule")
                            .secondary()
                            .icon("edit")
                            .enabled(enabled)
                            .build(
                                &self.hover,
                                move |this, _, cx| this.edit(Some(edit.clone()), cx),
                                cx,
                            ),
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
                                    let state = o.state.replace('_', " ");
                                    let tone = match o.state.as_str() {
                                        "completed" | "done" => Tone::Success,
                                        "running" | "dispatched" => Tone::Info,
                                        "skipped" | "missed" => Tone::Warning,
                                        "failed" => Tone::Danger,
                                        _ => Tone::Neutral,
                                    };
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
                                                .child(display_time(o.scheduled_at)),
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
                                                .on_surface(SURFACE_RAISED)
                                                .trailing(icon("arrowRight", ICON_SIZE_SM))
                                                .build(
                                                    &self.hover,
                                                    move |_, _, cx| cx.emit(OpenTicket(id)),
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
                                    display_time(h.observed_at),
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
        if self.focus_editor {
            window.focus(&self.name.read(cx).focus_handle(cx), cx);
            self.focus_editor = false;
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
            Button::new(id, "New rule")
                .primary()
                .icon("plus")
                .enabled(!this.pending)
                .build(&this.hover, |this, _, cx| this.edit(None, cx), cx)
        };
        let header = PageHeader::new("Automations")
            .description("Recurring rules that create and assign Tickets on a schedule.")
            .actions(
                row_gap(CONTROL_GAP)
                    .child(
                        Button::icon_only("automations.refresh", "refresh", "Refresh")
                            .secondary()
                            .enabled(!self.refreshing)
                            .build(&self.hover, |this, _, cx| this.refresh(cx), cx),
                    )
                    .child(
                        create(self, "automations.create", cx)
                            .debug_selector(|| "automations.create".into()),
                    ),
            );
        let mut content = column().gap(px(SECTION_GAP)).w_full();
        if let Some(error) = &self.error {
            content = content.child(
                row()
                    .id("automations.error")
                    .accessibility_id("automations.error")
                    .gap(px(SPACE_3))
                    .px(px(SPACE_4))
                    .py(px(SPACE_3))
                    .rounded(px(RADIUS_MD))
                    .border_1()
                    .border_color(rgb(ERROR_BORDER))
                    .bg(rgb(SURFACE_ERROR))
                    .text_size(type_size(LABEL_SIZE))
                    .text_color(rgb(DESTRUCTIVE_TEXT))
                    .child(icon("warning", ICON_SIZE).text_color(rgb(DESTRUCTIVE_TEXT)))
                    .child(error.clone()),
            );
        }
        if self.refreshing && self.error.is_some() {
            content = content
                .child(LoadingFrame::new(self.loading_started, window).inline("Reconnecting…"));
        }
        if !self.loaded && self.error.is_none() {
            content = content.child(skeleton_rows("automations.loading", 3));
        }
        if self.editing {
            content = content.child(self.editor(window, cx));
        } else if let Some(rule) = selected {
            content = content.child(self.detail(rule, cx));
        } else {
            if self.loaded && self.state.rules.is_empty() && self.error.is_none() {
                content = content.child(
                    EmptyState::new("refresh", "No Automations yet")
                        .description("Create a rule to have an agent pick up a fresh Ticket on a schedule. Overlapping work is skipped; missed firings stay in history.")
                        .selector("automations.empty")
                        .action(create(self, "automations.create.empty", cx))
                        .build(),
                );
            }
            content = content.child(self.list(cx));
        }
        Page::document(header)
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
