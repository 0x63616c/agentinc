use crate::{
    input::TextInput,
    storage::{
        AssigneeKind, Automation, AutomationCommand, AutomationSnapshot, Store, TicketProposal,
    },
    style::*,
};
use gpui::{prelude::*, *};
use std::sync::Arc;

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
    fn reload(&mut self) {
        if let Some(store) = &self.store {
            self.state = store.automations();
        }
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.pending || self.refreshing {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
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
    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        enabled: bool,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = id.into();
        let hover_id = id.clone();
        let enabled = enabled && !self.pending;
        let background = self.hover.color(&id);
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
                kind: ButtonKind::Secondary,
                enabled,
            },
            |button| {
                button
                    .h(px(CONTROL_HEIGHT))
                    .px(px(12.))
                    .bg(background)
                    .border_1()
                    .border_color(rgb(BORDER))
                    .on_hover(on_hover)
            },
            f,
            cx,
        )
    }
    fn field(label: &str, input: Entity<TextInput>) -> impl IntoElement {
        column()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(LABEL_SIZE))
                    .text_color(rgb(MUTED))
                    .child(label.to_owned()),
            )
            .child(
                row()
                    .h(px(FIELD_HEIGHT))
                    .px(px(12.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .rounded(px(FIELD_RADIUS))
                    .child(input),
            )
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
        let mut content=column().gap(px(24.)).w_full().max_w(px(880.)).child(row().justify_between().child(div().text_size(px(24.)).child("Automations")).child(row().gap(px(8.)).child(self.button("automations.refresh","Refresh",true,|this,_,cx|this.refresh(cx),cx).child("Refresh")).child(self.button("automations.create","Create Automation",true,|this,_,cx|this.edit(None,cx),cx).child("Create Automation"))))
            .child(div().text_color(rgb(MUTED)).text_size(px(LABEL_SIZE)).child("Recurring Tickets for your agents. Overlapping work is skipped; missed firings stay in history."));
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .id("automations.error")
                    .accessibility_id("automations.error")
                    .text_color(rgb(DESTRUCTIVE_TEXT))
                    .child(error.clone()),
            );
        }
        if !self.loaded {
            content = content.child("Loading Automations…");
        }
        if self.editing {
            let agents = self
                .store
                .as_ref()
                .map(|s| s.tickets().assignees)
                .unwrap_or_default();
            content=content.child(column().gap(px(16.)).child(Self::field("Name",self.name.clone())).child(Self::field("Ticket prompt",self.prompt.clone())).child(Self::field("Every (minutes)",self.minutes.clone()))
                .child(column().gap(px(8.)).child(div().text_color(rgb(MUTED)).child("Assign to"))
                    .when(!agents.iter().any(|a|a.kind==AssigneeKind::Agent),|s|s.child("Register an agent on the Agents page first."))
                    .child(row().gap(px(8.)).flex_wrap().children(agents.into_iter().filter(|a|a.kind==AssigneeKind::Agent).map(|a|{let id=a.id.clone();self.button(SharedString::from(format!("automations.agent.{}",a.id)),a.name.clone(),true,move|this,_,cx|{this.agent=Some(id.clone());cx.notify();},cx).border_color(rgb(if self.agent.as_ref()==Some(&a.id){FOCUS}else{BORDER})).child(a.name)}))))
                .child(div().text_color(rgb(MUTED)).text_size(px(CAPTION_SIZE)).child("Saving authorizes this rule to create and assign a new Ticket on each firing."))
                .child(row().gap(px(8.)).child(self.button("automations.save","Save rule",true,|this,_,cx|this.save(cx),cx).child("Save rule")).child(self.button("automations.cancel","Cancel",true,|this,_,cx|{this.editing=false;cx.notify();},cx).child("Cancel"))));
        } else if let Some(rule) = selected {
            let edit = rule.clone();
            let pause = rule.clone();
            let run = rule.clone();
            content=content.child(column().gap(px(16.)).child(row().gap(px(8.)).child(self.button("automations.back","All rules",true,|this,_,cx|{this.selected=None;cx.notify();},cx).child("← All rules")))
                .child(div().text_size(px(20.)).child(rule.name.clone())).child(rule.prompt.clone())
                .child(div().text_color(rgb(MUTED)).child(format!("Every {} {} · {}",rule.every_minutes,if rule.every_minutes == 1 {"minute"} else {"minutes"},if rule.revision!=rule.applied_revision {"Pending application"}else if rule.paused{"Paused"}else{"Active"})))
                .when_some(rule.error.clone(),|s,e|s.child(div().text_color(rgb(DESTRUCTIVE_TEXT)).child(e)))
                .child(row().gap(px(8.)).child(self.button("automations.edit","Edit rule",true,move|this,_,cx|this.edit(Some(edit.clone()),cx),cx).child("Edit rule"))
                    .child(self.button("automations.pause",if rule.paused{"Resume"}else{"Pause"},true,move|this,_,cx|this.command(AutomationCommand::Pause{id:pause.id.clone(),revision:pause.revision,paused:!pause.paused},cx),cx).child(if rule.paused{"Resume"}else{"Pause"}))
                    .child(self.button("automations.run","Run now",true,move|this,_,cx|this.command(AutomationCommand::RunNow{id:run.id.clone(),revision:run.revision},cx),cx).child("Run now")))
                .child(div().text_color(rgb(MUTED)).child("History · Run now deliberately replaces a missed firing; it does not replay all missed work."))
                .children(self.state.occurrences.iter().filter(|o|o.automation_id==rule.id).map(|o|{
                    let label=format!("{} · {}",display_time(o.scheduled_at),o.state.replace('_'," "));
                    row().w_full().justify_between().py(px(8.)).border_b_1().border_color(rgb(BORDER)).child(label)
                        .when_some(o.ticket_id,|s,id|s.child(self.button(SharedString::from(format!("automations.ticket.{id}")),format!("Open Ticket {id}"),true,move|_,_,cx|cx.emit(OpenTicket(id)),cx).child(format!("Ticket {id} →"))))
                }))
                .children(self.state.history.iter().filter(|h|h.automation_id==rule.id).map(|h|div().text_color(rgb(MUTED)).child(format!("{} · {} {}",display_time(h.observed_at),h.count,h.kind.replace('_'," "))))));
        } else {
            if self.loaded && self.state.rules.is_empty() && self.error.is_none() {
                content =
                    content.child("No Automations yet. Create your first recurring Ticket rule.");
            }
            content = content.children(self.state.rules.iter().map(|r| {
                let id = r.id.clone();
                self.button(
                    SharedString::from(format!("automations.rule.{}", r.id)),
                    r.name.clone(),
                    true,
                    move |this, _, cx| {
                        this.selected = Some(id.clone());
                        cx.notify();
                    },
                    cx,
                )
                .h_auto()
                .min_h(px(64.))
                .w_full()
                .justify_between()
                .child(r.name.clone())
                .child(div().text_color(rgb(MUTED)).child(format!(
                    "Every {} min · {}",
                    r.every_minutes,
                    if r.error.is_some() {
                        "Needs attention"
                    } else if r.revision != r.applied_revision {
                        "Pending"
                    } else if r.paused {
                        "Paused"
                    } else {
                        "Active"
                    }
                )))
            }));
        }
        div()
            .id("automations.page")
            .track_focus(&self.page_focus)
            .accessibility_id("automations.page")
            .size_full()
            .overflow_y_scroll()
            .p(px(32.))
            .child(content)
    }
}
