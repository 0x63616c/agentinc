use crate::{
    commands::{self, Parsed},
    input::{Submit, TextInput},
    markdown,
    providers::{self, ProviderId, ProviderModel, ProviderStatus, ProviderTest, ProvidersState},
    storage::{Command, Conversation, Store, Turn},
    ui::*,
};
use ainc_client::types::{HttpPolicy, TurnStep};
use gpui::{prelude::*, *};
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn conversation_date(updated: &str, updated_at: i64, now: i64) -> String {
    let updated = chrono::DateTime::from_timestamp(updated_at, 0)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| updated.to_owned());
    let age = now.saturating_sub(updated_at);
    if age < 86_400 {
        format!("Today, {}", updated.get(11..16).unwrap_or_default())
    } else if age < 172_800 {
        "Yesterday".to_owned()
    } else {
        let year = updated.get(2..4).unwrap_or_default();
        let month_day = updated.get(5..10).unwrap_or_default().replace('-', "/");
        format!("{month_day}/{year}")
    }
}

pub struct AssistantPage {
    store: Option<Arc<Store>>,
    overlays: Rc<RefCell<OverlayHost>>,
    turns: Vec<Turn>,
    input: Entity<TextInput>,
    conversations: Vec<Conversation>,
    conversation: Option<i64>,
    show_chat: bool,
    rename_input: Entity<TextInput>,
    form_error: Option<String>,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    account: Option<String>,
    providers: Option<ProvidersState>,
    model: Option<String>,
    model_menu_open: bool,
    model_menu_closing: bool,
    model_menu_generation: u64,
    credentials_busy: bool,
    busy_provider: Option<ProviderId>,
    connection_error: Option<String>,
    provider_test: Option<(ProviderId, ProviderTest)>,
    api_key_input: Entity<TextInput>,
    code_input: Entity<TextInput>,
    allow_input: Entity<TextInput>,
    deny_input: Entity<TextInput>,
    expanded_steps: HashSet<i64>,
    palette_index: usize,
    help_open: bool,
    notice: Option<String>,
    active: Option<i64>,
    error: Option<String>,
    pending: bool,
    scroll: ScrollHandle,
    appearance: Option<Instant>,
    loading_started: Instant,
    reduced_motion: bool,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
pub enum Navigation {
    Settings,
    Chat,
    List,
}
impl EventEmitter<Navigation> for AssistantPage {}
impl AssistantPage {
    pub(crate) fn update_drafts(&self, cx: &App) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(
            !self.pending,
            "Wait for the current change to finish before installing"
        );
        Ok(
            serde_json::json!({"conversation":self.conversation,"input": self.input.read(cx).content.to_string(), "rename_input": self.rename_input.read(cx).content.to_string()}),
        )
    }
    pub(crate) fn restore_update_drafts(
        &mut self,
        value: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if let Ok(value) = serde_json::from_value(value["conversation"].clone()) {
            self.conversation = value;
        }
        if let Some(text) = value["input"].as_str() {
            self.input.update(cx, |input, cx| input.set_text(text, cx));
        }
        if let Some(text) = value["rename_input"].as_str() {
            self.rename_input
                .update(cx, |input, cx| input.set_text(text, cx));
        }
    }

    pub fn new(
        store: Option<Arc<Store>>,
        storage_error: Option<String>,
        overlays: Rc<RefCell<OverlayHost>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::composer(cx).identified("evee.composer"));
        let rename_input = cx.new(|cx| {
            TextInput::field("Conversation title", false, cx).identified("evee.conversation.title")
        });
        let api_key_input =
            cx.new(|cx| TextInput::field("sk-or-…", true, cx).identified("evee.openrouter.key"));
        let code_input = cx
            .new(|cx| TextInput::field("Paste the code", false, cx).identified("evee.claude.code"));
        let allow_input =
            cx.new(|cx| TextInput::field("*", false, cx).identified("evee.policy.allow"));
        let deny_input =
            cx.new(|cx| TextInput::field("none", false, cx).identified("evee.policy.deny"));
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.send(cx)),
            cx.observe(&input, |_, _, cx| cx.notify()),
            cx.subscribe(&api_key_input, |this, _, _: &Submit, cx| {
                this.connect(ProviderId::Openrouter, cx)
            }),
            cx.subscribe(&code_input, |this, _, _: &Submit, cx| this.submit_code(cx)),
            cx.subscribe(&allow_input, |this, _, _: &Submit, cx| this.save_policy(cx)),
            cx.subscribe(&deny_input, |this, _, _: &Submit, cx| this.save_policy(cx)),
            cx.subscribe(&rename_input, |this, _, _: &Submit, cx| this.rename(cx)),
            cx.observe(&rename_input, |this, input, cx| {
                let title = input.read(cx).content.trim().to_owned();
                this.form_error =
                    (title.chars().count() > 120).then(|| "Use 120 characters or fewer.".into());
                cx.notify();
            }),
        ];
        let conversations = store
            .as_ref()
            .map(|db| db.snapshot().conversations)
            .unwrap_or_default();
        let conversation = conversations.first().map(|c| c.id);
        let turns = vec![];
        let error = storage_error;
        let model = None;
        #[cfg(not(test))]
        if let Some(db) = store.clone() {
            let request = cx.background_executor().spawn(async move { db.refresh() });
            cx.spawn(async move |this, cx| {
                let result = request.await;
                let _ = this.update(cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            this.reload_snapshot();
                            this.load_policy(cx);
                            if let Some(id) = this.active {
                                this.watch_turn(id, cx);
                            }
                        }
                        Err(error) => this.error = Some(format!("Data unavailable: {error}")),
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        #[cfg(not(test))]
        {
            let request = cx
                .background_executor()
                .spawn(async { providers::state(false) });
            cx.spawn(async move |this, cx| {
                let result = request.await;
                let _ = this.update(cx, |this, cx| {
                    this.apply_providers(result);
                    cx.notify();
                });
            })
            .detach();
        }
        Self {
            store,
            overlays,
            turns,
            input,
            conversations,
            conversation,
            show_chat: false,
            rename_input,
            form_error: None,
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            account: None,
            providers: None,
            model,
            model_menu_open: false,
            model_menu_closing: false,
            model_menu_generation: 0,
            credentials_busy: !cfg!(test),
            busy_provider: None,
            connection_error: None,
            provider_test: None,
            api_key_input,
            code_input,
            allow_input,
            deny_input,
            expanded_steps: HashSet::new(),
            palette_index: 0,
            help_open: false,
            notice: None,
            active: None,
            error,
            pending: false,
            scroll: ScrollHandle::new(),
            appearance: None,
            loading_started: Instant::now(),
            reduced_motion: reduced_motion(),
            hover: HoverFade::default(),
            _subscriptions: subscriptions,
        }
    }
    fn apply_providers(&mut self, result: anyhow::Result<ProvidersState>) {
        self.credentials_busy = false;
        self.busy_provider = None;
        match result {
            Ok(state) => {
                self.account = state
                    .providers
                    .iter()
                    .find(|provider| provider.connected)
                    .map(|provider| {
                        provider
                            .account
                            .clone()
                            .unwrap_or_else(|| provider.name.clone())
                    });
                self.connection_error = None;
                self.providers = Some(state);
            }
            Err(error) => self.connection_error = Some(error.to_string()),
        }
    }
    fn provider(&self, id: ProviderId) -> Option<&ProviderStatus> {
        self.providers
            .as_ref()
            .and_then(|state| state.providers.iter().find(|provider| provider.id == id))
    }
    fn models(&self) -> Vec<(&ProviderStatus, &ProviderModel)> {
        self.providers
            .iter()
            .flat_map(|state| state.providers.iter())
            .flat_map(|provider| provider.models.iter().map(move |model| (provider, model)))
            .collect()
    }
    fn model_label(&self) -> String {
        self.model
            .as_ref()
            .and_then(|id| {
                self.models()
                    .into_iter()
                    .find(|(_, model)| &model.id == id)
                    .map(|(provider, model)| format!("{} · {}", model.name, provider.name))
            })
            .unwrap_or_else(|| "Choose a model".into())
    }
    fn find_model(&self, needle: &str) -> Option<String> {
        let needle = needle.trim().to_lowercase();
        self.models()
            .into_iter()
            .filter(|(provider, _)| provider.connected)
            .find(|(_, model)| {
                model.name.to_lowercase().contains(&needle)
                    || model.id.to_lowercase().contains(&needle)
            })
            .map(|(_, model)| model.id.clone())
    }
    fn run_provider(
        &mut self,
        id: Option<ProviderId>,
        operation: impl FnOnce() -> anyhow::Result<ProvidersState> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.credentials_busy {
            return;
        }
        self.credentials_busy = true;
        self.busy_provider = id;
        self.connection_error = None;
        let request = cx.background_executor().spawn(async move { operation() });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.apply_providers(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn refresh_providers(&mut self, refresh: bool, cx: &mut Context<Self>) {
        self.run_provider(None, move || providers::state(refresh), cx);
    }
    fn connect(&mut self, id: ProviderId, cx: &mut Context<Self>) {
        match id {
            ProviderId::Openrouter => {
                let key = self.api_key_input.read(cx).content.trim().to_owned();
                if key.is_empty() {
                    self.connection_error = Some("Paste your OpenRouter API key first.".into());
                    cx.notify();
                    return;
                }
                self.api_key_input.update(cx, |input, cx| {
                    input.reset();
                    cx.notify();
                });
                self.run_provider(
                    Some(id),
                    move || providers::connect(id, Some(key), None),
                    cx,
                );
            }
            ProviderId::Claude | ProviderId::Codex => {
                // Follow the daemon's sign-in: open the browser link once, then wait
                // until it finishes (ChatGPT) or asks for the pasted code (Claude).
                self.run_provider(
                    Some(id),
                    move || {
                        let mut state = providers::connect(id, None, None)?;
                        let mut opened = false;
                        loop {
                            let Some(status) = state.providers.iter().find(|p| p.id == id).cloned()
                            else {
                                return Ok(state);
                            };
                            if let Some(url) = &status.auth_url
                                && !opened
                            {
                                let _ =
                                    std::process::Command::new("/usr/bin/open").arg(url).spawn();
                                opened = true;
                            }
                            if !status.signing_in || status.awaiting_code {
                                return Ok(state);
                            }
                            std::thread::sleep(Duration::from_millis(250));
                            state = providers::state(false)?;
                        }
                    },
                    cx,
                );
            }
        }
    }
    fn submit_code(&mut self, cx: &mut Context<Self>) {
        let code = self.code_input.read(cx).content.trim().to_owned();
        if code.is_empty() {
            self.connection_error = Some("Paste the code shown after signing in.".into());
            cx.notify();
            return;
        }
        self.code_input.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
        self.run_provider(
            Some(ProviderId::Claude),
            move || providers::connect(ProviderId::Claude, None, Some(code)),
            cx,
        );
    }
    fn cancel_sign_in(&mut self, id: ProviderId, cx: &mut Context<Self>) {
        // Cancellation must work while the sign-in follower is still busy.
        let request = cx
            .background_executor()
            .spawn(async move { providers::cancel(id) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(state) = result {
                    this.providers = Some(state);
                }
                this.credentials_busy = false;
                this.busy_provider = None;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn disconnect(&mut self, id: ProviderId, cx: &mut Context<Self>) {
        if self.active.is_some() {
            return;
        }
        self.provider_test = None;
        self.run_provider(Some(id), move || providers::disconnect(id), cx);
    }
    fn test_provider(&mut self, id: ProviderId, cx: &mut Context<Self>) {
        if self.credentials_busy {
            return;
        }
        self.credentials_busy = true;
        self.busy_provider = Some(id);
        self.provider_test = None;
        let prefix = format!("{id}:");
        let model = self
            .model
            .clone()
            .filter(|model| model.starts_with(&prefix));
        let request = cx
            .background_executor()
            .spawn(async move { providers::test(id, model) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.credentials_busy = false;
                this.busy_provider = None;
                match result {
                    Ok(test) => this.provider_test = Some((id, test)),
                    Err(error) => this.connection_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save_policy(&mut self, cx: &mut Context<Self>) {
        let split = |text: &str| -> Vec<String> {
            text.split([',', ' ', '\n'])
                .map(|host| host.trim().to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect()
        };
        let policy = HttpPolicy {
            allow: split(&self.allow_input.read(cx).content),
            deny: split(&self.deny_input.read(cx).content),
        };
        self.mutate(
            move |db| db.command(Command::SetHttpPolicy { policy }),
            |this, _, _| this.notice = Some("HTTP policy saved.".into()),
            cx,
        );
    }
    fn load_policy(&mut self, cx: &mut Context<Self>) {
        let Some(db) = &self.store else { return };
        let policy = db.snapshot().settings.http_policy;
        self.allow_input
            .update(cx, |input, cx| input.set_text(&policy.allow.join(", "), cx));
        self.deny_input
            .update(cx, |input, cx| input.set_text(&policy.deny.join(", "), cx));
    }

    pub fn providers_view(&self, cx: &mut Context<Self>) -> Div {
        column()
            .child(self.provider_row(ProviderId::Claude, cx))
            .child(settings_divider())
            .child(self.provider_row(ProviderId::Codex, cx))
            .child(settings_divider())
            .child(self.provider_row(ProviderId::Openrouter, cx))
            .child(settings_divider())
            .child(settings_row(
                "Model",
                "Used for new replies. Jev on OpenRouter is the inexpensive default for everyday work.",
                self.model_menu(true, cx),
            ))
            .child(settings_divider())
            .child(settings_row(
                "Connection status",
                match &self.connection_error {
                    Some(error) => error.clone(),
                    None if self.credentials_busy => "Checking providers…".to_owned(),
                    None => "Re-check every sign-in, key and model list.".to_owned(),
                },
                settings_button(
                    "providers-refresh",
                    "Refresh",
                    !self.credentials_busy,
                    |this: &mut Self, _, cx| this.refresh_providers(true, cx),
                    cx,
                ),
            ))
    }
    pub fn agent_view(&self, cx: &mut Context<Self>) -> Div {
        let enabled = !self.pending && self.store.is_some();
        column()
            .child(settings_row(
                "Allowed hosts",
                "Hosts Evee may call with http_request, separated by commas. `*` allows every public host; private networks are always refused.",
                self.policy_field("policy-allow", self.allow_input.clone()),
            ))
            .child(settings_divider())
            .child(settings_row(
                "Denied hosts",
                "Hosts Evee must never call, even when allowed above. `*.example.com` covers subdomains.",
                row()
                    .gap(px(CONTROL_GAP))
                    .items_center()
                    .child(self.policy_field("policy-deny", self.deny_input.clone()))
                    .child(settings_button(
                        "policy-save",
                        "Save",
                        enabled,
                        |this: &mut Self, _, cx| this.save_policy(cx),
                        cx,
                    )),
            ))
    }
    fn policy_field(&self, selector: &'static str, input: Entity<TextInput>) -> Div {
        row()
            .debug_selector(move || selector.into())
            .w(px(300.))
            .h(px(CONTROL_HEIGHT))
            .px(px(FIELD_INSET_X))
            .items_center()
            .border_1()
            .border_color(rgb(BORDER))
            .rounded(px(FIELD_RADIUS))
            .bg(rgb(SURFACE_SEARCH))
            .child(input)
    }
    fn provider_title(id: ProviderId) -> &'static str {
        match id {
            ProviderId::Claude => "Claude",
            ProviderId::Codex => "ChatGPT",
            ProviderId::Openrouter => "OpenRouter",
        }
    }
    fn provider_row(&self, id: ProviderId, cx: &mut Context<Self>) -> Div {
        let status = self.provider(id).cloned();
        let busy = self.credentials_busy && self.busy_provider == Some(id);
        let enabled = !self.credentials_busy && self.active.is_none();
        let state = if busy {
            "Working…".to_owned()
        } else if let Some(status) = &status {
            if let Some(error) = &status.error {
                error.clone()
            } else if status.awaiting_code {
                "Paste the code from your browser to finish signing in.".to_owned()
            } else if status.signing_in {
                "Complete the sign-in in your browser.".to_owned()
            } else if status.connected {
                format!(
                    "Connected · {}",
                    status.account.clone().unwrap_or_else(|| "Ready".into())
                )
            } else {
                status.description.clone()
            }
        } else if self.credentials_busy {
            "Checking…".to_owned()
        } else {
            "Not checked yet.".to_owned()
        };
        let mut controls = row().gap(px(CONTROL_GAP)).items_center().justify_end();
        match status.as_ref() {
            Some(status) if status.connected => {
                controls = controls.child(settings_button(
                    ("provider-test", id as usize),
                    "Test",
                    enabled,
                    move |this: &mut Self, _, cx| this.test_provider(id, cx),
                    cx,
                ));
                match id {
                    ProviderId::Codex => {
                        controls = controls.child(settings_button(
                            "codex-sign-in",
                            "Sign out",
                            enabled,
                            move |this: &mut Self, _, cx| this.disconnect(id, cx),
                            cx,
                        ))
                    }
                    ProviderId::Openrouter => {
                        controls = controls.child(settings_button(
                            "openrouter-remove",
                            "Remove key",
                            enabled,
                            move |this: &mut Self, _, cx| this.disconnect(id, cx),
                            cx,
                        ))
                    }
                    ProviderId::Claude => {}
                }
            }
            Some(status) if status.awaiting_code => {
                controls = controls
                    .child(self.policy_field("claude-code", self.code_input.clone()))
                    .child(settings_button(
                        "claude-finish",
                        "Finish",
                        !self.credentials_busy,
                        |this: &mut Self, _, cx| this.submit_code(cx),
                        cx,
                    ))
                    .child(settings_button(
                        "claude-cancel",
                        "Cancel",
                        true,
                        move |this: &mut Self, _, cx| this.cancel_sign_in(id, cx),
                        cx,
                    ));
            }
            Some(status) if status.signing_in => {
                controls = controls.child(settings_button(
                    "cancel-sign-in",
                    "Cancel",
                    true,
                    move |this: &mut Self, _, cx| this.cancel_sign_in(id, cx),
                    cx,
                ));
            }
            _ => match id {
                ProviderId::Claude => {
                    controls = controls.child(settings_button(
                        "claude-sign-in",
                        "Sign in with Claude",
                        enabled,
                        move |this: &mut Self, _, cx| this.connect(id, cx),
                        cx,
                    ))
                }
                ProviderId::Codex => {
                    controls = controls.child(settings_icon_button(
                        "codex-sign-in",
                        "Sign in with ChatGPT",
                        "openai",
                        enabled,
                        move |this: &mut Self, _, cx| this.connect(id, cx),
                        cx,
                    ))
                }
                ProviderId::Openrouter => {
                    controls = controls
                        .child(self.policy_field("openrouter-key", self.api_key_input.clone()))
                        .child(settings_button(
                            "openrouter-save",
                            "Save key",
                            enabled,
                            move |this: &mut Self, _, cx| this.connect(id, cx),
                            cx,
                        ))
                }
            },
        }
        let test = self
            .provider_test
            .as_ref()
            .filter(|(tested, _)| *tested == id)
            .map(|(_, test)| {
                if test.ok {
                    (
                        format!(
                            "Replied “{}” in {:.1} s",
                            test.reply.clone().unwrap_or_default().trim(),
                            test.elapsed_ms as f32 / 1000.
                        ),
                        STATUS_GREEN,
                    )
                } else {
                    (
                        format!(
                            "Test failed: {}",
                            test.error.clone().unwrap_or_else(|| "no reply".into())
                        ),
                        ERROR,
                    )
                }
            });
        settings_row(
            Self::provider_title(id),
            state,
            column().gap(px(6.)).items_end().child(controls).when_some(
                test,
                |control, (text, color)| {
                    control.child(
                        div()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(color))
                            .child(text),
                    )
                },
            ),
        )
    }
    fn toggle_model_menu(&mut self, cx: &mut Context<Self>) {
        if self.model_menu_open {
            self.close_model_menu(cx);
        } else {
            self.model_menu_generation += 1;
            self.model_menu_open = true;
            self.model_menu_closing = false;
            cx.notify();
        }
    }
    /// The model picker: grouped by provider, featured models first, with the
    /// same element IDs in Settings and in the composer.
    fn model_menu(&self, settings: bool, cx: &mut Context<Self>) -> Div {
        let enabled = self.store.is_some() && !self.pending;
        let label = self.model_label();
        let trigger = if settings {
            settings_button(
                "model-select",
                label,
                enabled,
                |this: &mut Self, _, cx| this.toggle_model_menu(cx),
                cx,
            )
            .w_full()
            .justify_between()
            .child("⌄")
        } else {
            self.action("model-select", &label, enabled, Self::toggle_model_menu, cx)
                .border_1()
                .border_color(rgb(BORDER))
                .child("⌄")
        };
        let menu_open = self.model_menu_open;
        column()
            .relative()
            .when(settings, |anchor| anchor.w(px(260.)).h(px(CONTROL_HEIGHT)))
            .child(trigger.debug_selector(|| "model-select".into()))
            .when(self.model_menu_open || self.model_menu_closing, |anchor| {
                let mut index = 0usize;
                let mut menu = column()
                    .id("model-menu")
                    .debug_selector(|| "model-menu".into())
                    .absolute()
                    .when(settings, |menu| menu.right(px(0.)).top(px(36.)))
                    .when(!settings, |menu| menu.left(px(0.)).bottom(px(34.)))
                    .w(px(300.))
                    .max_h(px(340.))
                    .overflow_y_scroll()
                    .p(px(4.))
                    .rounded(px(CONTROL_RADIUS))
                    .border_1()
                    .border_color(rgb(BORDER_OVERLAY))
                    .bg(rgb(SURFACE_MENU))
                    .shadow(vec![
                        BoxShadow::new(px(0.), px(8.), rgba(SCRIM).into()).blur_radius(px(24.)),
                    ]);
                for provider in self
                    .providers
                    .iter()
                    .flat_map(|state| state.providers.iter())
                {
                    menu = menu.child(
                        row()
                            .px(px(10.))
                            .pt(px(8.))
                            .pb(px(4.))
                            .gap(px(6.))
                            .items_center()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(provider.name.clone()),
                            )
                            .when(!provider.connected, |header| {
                                header.child("· not connected")
                            }),
                    );
                    for model in &provider.models {
                        menu = menu.child(self.model_option(
                            ("model", index),
                            model.name.clone(),
                            Some(model.id.clone()),
                            model.featured,
                            enabled && menu_open && provider.connected,
                            cx,
                        ));
                        index += 1;
                    }
                }
                if index == 0 {
                    menu = menu.child(
                        div()
                            .px(px(10.))
                            .py(px(8.))
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
                            .child("Connect a provider in Settings to choose a model."),
                    );
                }
                anchor.child(
                    deferred(menu.with_animation(
                        if menu_open {
                            "model-menu-open"
                        } else {
                            "model-menu-close"
                        },
                        Animation::new(Duration::from_millis(PANEL_MS)),
                        move |menu, progress| {
                            menu.opacity(if menu_open { progress } else { 1. - progress })
                        },
                    ))
                    .with_priority(1),
                )
            })
    }
    fn model_option(
        &self,
        id: impl Into<ElementId>,
        name: String,
        model: Option<String>,
        featured: bool,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.model == model;
        let choice = model.clone();
        let selector = format!(
            "settings.model-option.{}",
            name.to_lowercase().replace(' ', "-")
        );
        action_button(
            ButtonSpec {
                id: id.into(),
                label: name.clone().into(),
                kind: ButtonKind::Quiet,
                enabled,
            },
            |button| {
                button
                    .debug_selector(move || selector.clone())
                    .w_full()
                    .min_h(px(34.))
                    .px(px(10.))
                    .gap(px(10.))
                    .rounded(px(CONTROL_RADIUS))
                    .bg(rgb(if selected { SELECTED } else { SURFACE_MENU }))
                    .hover(|item| item.bg(rgb(HOVER_CONTROL)))
                    .text_size(type_size(LABEL_SIZE))
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .when(featured, |item| {
                        item.child(
                            div()
                                .px(px(6.))
                                .py(px(1.))
                                .rounded_full()
                                .bg(rgb(SURFACE_SEGMENT))
                                .text_size(type_size(CAPTION_SIZE - 1.))
                                .text_color(rgb(TEXT_ACCENT))
                                .child("Featured"),
                        )
                    })
                    .when(selected, |item| item.child("✓"))
            },
            move |this: &mut Self, _, cx| this.select_model(choice.clone(), cx),
            cx,
        )
    }
    fn mutate<R: Send + 'static>(
        &mut self,
        operation: impl FnOnce(Arc<Store>) -> anyhow::Result<R> + Send + 'static,
        apply: impl FnOnce(&mut Self, R, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.pending {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        self.pending = true;
        self.error = None;
        let request = cx
            .background_executor()
            .spawn(async move { operation(store) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                this.reload_snapshot();
                match result {
                    Ok(value) => {
                        apply(this, value, cx);
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        this.form_error = this.error.clone();
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn reload_snapshot(&mut self) {
        let Some(db) = &self.store else { return };
        let snapshot = db.snapshot();
        self.conversations = snapshot.conversations;
        if self
            .conversation
            .is_none_or(|id| !self.conversations.iter().any(|c| c.id == id))
        {
            self.conversation = snapshot
                .settings
                .selected_conversation
                .filter(|id| self.conversations.iter().any(|c| c.id == *id))
                .or_else(|| self.conversations.first().map(|c| c.id));
        }
        self.turns = snapshot
            .turns
            .into_iter()
            .filter(|t| Some(t.conversation_id) == self.conversation)
            .collect();
        self.active = self
            .turns
            .iter()
            .find(|t| t.state == "queued" || t.state == "running")
            .map(|t| t.id);
        self.model = snapshot.settings.model.filter(|s| !s.is_empty());
    }
    pub(crate) fn workspace_changed(&mut self, cx: &mut Context<Self>) {
        self.conversation = None;
        self.input.update(cx, |input, _| input.reset());
        self.rename_input.update(cx, |input, _| input.reset());
        self.form_error = None;
        self.help_open = false;
        self.notice = None;
        self.reload_snapshot();
        self.load_policy(cx);
    }
    fn select_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.close_model_menu(cx);
        self.mutate(
            move |db| {
                db.command(Command::SelectModel {
                    model: model.unwrap_or_default(),
                })
            },
            |_, _, _| {},
            cx,
        );
    }
    fn close_model_menu(&mut self, cx: &mut Context<Self>) {
        if !self.model_menu_open {
            return;
        }
        self.model_menu_open = false;
        self.model_menu_closing = true;
        self.model_menu_generation += 1;
        let generation = self.model_menu_generation;
        let timer = cx
            .background_executor()
            .timer(Duration::from_millis(PANEL_MS));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                if this.model_menu_generation == generation {
                    this.model_menu_closing = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn open_conversation(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.mutate(
            move |db| db.command(Command::SelectConversation { id }),
            move |this, _, cx| {
                this.conversation = Some(id);
                this.show_chat = true;
                this.reload_snapshot();
                this.overlays.borrow_mut().close();
                this.input.update(cx, |i, cx| {
                    i.reset();
                    cx.notify();
                });
                this.scroll.scroll_to_bottom();
                if let Some(id) = this.active {
                    this.watch_turn(id, cx);
                }
                cx.emit(Navigation::Chat);
            },
            cx,
        );
    }
    fn new_conversation(&mut self, cx: &mut Context<Self>) {
        self.mutate(
            |db| {
                let id = db.new_conversation()?;
                db.command(Command::SelectConversation { id })?;
                Ok(id)
            },
            |this, id, cx| {
                this.conversation = Some(id);
                this.show_chat = true;
                this.reload_snapshot();
                this.input.update(cx, |i, cx| {
                    i.reset();
                    cx.notify();
                });
                cx.emit(Navigation::Chat);
            },
            cx,
        );
    }
    fn rename(&mut self, cx: &mut Context<Self>) {
        let active = self.overlays.borrow().active();
        if let Some(Overlay::RenameConversation(id)) = active {
            let title = self.rename_input.read(cx).content.trim().to_owned();
            if title.is_empty() || self.form_error.is_some() {
                return;
            }
            self.mutate(
                move |db| db.command(Command::RenameConversation { id, title }),
                |this, _, _| {
                    this.overlays.borrow_mut().close();
                    this.form_error = None;
                },
                cx,
            );
        }
    }
    fn delete(&mut self, cx: &mut Context<Self>) {
        let active = self.overlays.borrow().active();
        if let Some(Overlay::DeleteConversation(id)) = active {
            self.mutate(
                move |db| db.command(Command::DeleteConversation { id }),
                |this, _, _| {
                    this.overlays.borrow_mut().close();
                    this.form_error = None;
                    this.show_chat = false;
                },
                cx,
            );
        }
    }
    pub fn conversations_view(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let enabled = self.active.is_none() && !self.pending && self.store.is_some();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_secs() as i64);
        Page::document(
            PageHeader::new("Assistant")
                .description("Your conversations with Evee.")
                .actions(
                    self.action(
                        "new-chat",
                        "New conversation",
                        enabled,
                        Self::new_conversation,
                        cx,
                    )
                    .border_1()
                    .border_color(rgb(BORDER)),
                ),
        )
        .child(
            column()
                .gap(px(16.))
                .when_some(self.error.clone(), |s, e| {
                    s.child(
                        div()
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(ERROR))
                            .child(e),
                    )
                })
                .when(self.conversations.is_empty(), |s| {
                    s.child(
                        div()
                            .py(px(28.))
                            .text_color(rgb(MUTED))
                            .child("Your conversations will appear here."),
                    )
                })
                .children(self.conversations.iter().map(|conversation| {
                    let id = conversation.id;
                    let selected = self.conversation == Some(id);
                    let snippet = if conversation.snippet.trim().is_empty() {
                        "No messages yet".to_owned()
                    } else {
                        conversation
                            .snippet
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                    };
                    column()
                        .relative()
                        .rounded(px(7.))
                        .bg(rgb(if selected { SELECTED } else { SURFACE }))
                        .child(
                            row()
                                .gap(px(8.))
                                .px(px(12.))
                                .py(px(10.))
                                .child(
                                    self.action(
                                        ("conversation", id as u64),
                                        "",
                                        enabled,
                                        move |this, cx| this.open_conversation(id, cx),
                                        cx,
                                    )
                                    .flex_1()
                                    .min_w_0()
                                    .justify_start()
                                    .bg(rgb(if selected { SELECTED } else { SURFACE }))
                                    .child(
                                        column()
                                            .gap(px(3.))
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_size(type_size(BODY_SIZE))
                                                    .text_color(rgb(TEXT))
                                                    .truncate()
                                                    .child(conversation.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(type_size(CAPTION_SIZE))
                                                    .text_color(rgb(MUTED))
                                                    .truncate()
                                                    .child(snippet),
                                            ),
                                    ),
                                )
                                .child(
                                    div()
                                        .text_size(type_size(10.))
                                        .text_color(rgb(MUTED))
                                        .child(conversation_date(
                                            &conversation.updated,
                                            conversation.updated_at,
                                            now,
                                        )),
                                )
                                .child(self.action_window(
                                    ("chat-menu", id as u64),
                                    "…",
                                    Some("Conversation actions"),
                                    enabled,
                                    move |this, window, cx| {
                                        let mut host = this.overlays.borrow_mut();
                                        if host.active() == Some(Overlay::ConversationMenu(id)) {
                                            host.dismiss(window, cx);
                                        } else {
                                            host.open(
                                                Overlay::ConversationMenu(id),
                                                window,
                                                cx,
                                                None,
                                            );
                                        }
                                        cx.notify();
                                    },
                                    cx,
                                )),
                        )
                        .when(
                            self.overlays.borrow().active() == Some(Overlay::ConversationMenu(id)),
                            |s| {
                                s.child(
                                    menu_shell(
                                        column()
                                            .child(
                                                column()
                                                    .px(px(10.))
                                                    .py(px(5.))
                                                    .text_size(type_size(10.))
                                                    .text_color(rgb(MUTED))
                                                    .child("Updated")
                                                    .child(conversation.updated.clone()),
                                            )
                                            .child(
                                                self.action_window(
                                                    ("rename-chat", id as u64),
                                                    "Rename",
                                                    None,
                                                    enabled,
                                                    move |this, window, cx| {
                                                        if let Some(c) = this
                                                            .conversations
                                                            .iter()
                                                            .find(|c| c.id == id)
                                                        {
                                                            this.rename_input.update(
                                                                cx,
                                                                |input, cx| {
                                                                    input.set_text(&c.title, cx)
                                                                },
                                                            );
                                                        }
                                                        this.form_error = None;
                                                        let initial_focus =
                                                            this.rename_input.focus_handle(cx);
                                                        this.overlays.borrow_mut().open(
                                                            Overlay::RenameConversation(id),
                                                            window,
                                                            cx,
                                                            Some(initial_focus),
                                                        );
                                                        cx.notify();
                                                    },
                                                    cx,
                                                )
                                                .w_full()
                                                .justify_start(),
                                            )
                                            .child(
                                                self.action_window(
                                                    ("delete-chat", id as u64),
                                                    "Delete",
                                                    None,
                                                    enabled,
                                                    move |this, window, cx| {
                                                        this.form_error = None;
                                                        this.overlays.borrow_mut().open(
                                                            Overlay::DeleteConversation(id),
                                                            window,
                                                            cx,
                                                            Some(this.cancel_focus.clone()),
                                                        );
                                                        cx.notify();
                                                    },
                                                    cx,
                                                )
                                                .w_full()
                                                .justify_start()
                                                .text_color(rgb(DESTRUCTIVE_TEXT)),
                                            ),
                                    )
                                    .absolute()
                                    .right(px(0.))
                                    .top(px(48.)),
                                )
                            },
                        )
                })),
        )
        .build()
    }
    pub fn show_list(&mut self, cx: &mut Context<Self>) {
        self.show_chat = false;
        cx.emit(Navigation::List);
        cx.notify();
    }
    pub fn composer_focus(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_chat(&mut self, populated: bool, cx: &mut Context<Self>) {
        self.fixture_models(cx);
        self.conversation = Some(1);
        self.conversations = vec![Conversation {
            id: 1,
            title: "Planning the day".into(),
            snippet: "Let's prioritize the work.".into(),
            updated: "2026-09-24 09:00".into(),
            updated_at: 1_790_249_400,
        }];
        self.turns = if populated {
            vec![
                fixture_turn(
                    1,
                    "What should I focus on today?",
                    None,
                    "completed",
                    vec![fixture_step(
                        1,
                        1,
                        "text",
                        serde_json::json!({"text":"Start with the **blocking Ticket**, then the agent run.\n\n1. Review `ticket-142` (blocked on the API contract)\n2. Kick off the nightly automation\n3. Reply to the design thread"}),
                    )],
                ),
                fixture_turn(
                    2,
                    "List my Tickets grouped by state.",
                    Some("/tickets"),
                    "completed",
                    vec![
                        fixture_step(
                            2,
                            2,
                            "tool_use",
                            serde_json::json!({"id":"call_1","name":"list_tickets","input":{}}),
                        ),
                        fixture_step(
                            3,
                            2,
                            "tool_result",
                            serde_json::json!({"tool_use_id":"call_1","content":{"tickets":[{"id":142,"title":"Ship the API contract","status":"in_progress"}]},"is_error":false}),
                        ),
                        fixture_step(
                            4,
                            2,
                            "text",
                            serde_json::json!({"text":"You have **one Ticket in progress**: *Ship the API contract*. Nothing is blocked."}),
                        ),
                    ],
                ),
            ]
        } else {
            vec![]
        };
        self.show_chat = true;
        cx.notify();
    }
    /// A turn mid-flight: a tool call still running and reply text streaming in.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_streaming(&mut self, cx: &mut Context<Self>) {
        self.fixture_chat(true, cx);
        let mut turn = fixture_turn(
            3,
            "Fetch https://example.test/status and summarize it.",
            Some("/http https://example.test/status"),
            "running",
            vec![fixture_step(
                5,
                3,
                "tool_use",
                serde_json::json!({"id":"call_2","name":"http_request","input":{"method":"GET","url":"https://example.test/status"}}),
            )],
        );
        turn.draft = Some("Checking the status page now. So far the service reports".into());
        self.turns.push(turn);
        self.active = Some(3);
        self.expanded_steps.insert(2);
        cx.notify();
    }
    /// The slash palette open with a partial command typed.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_palette(&mut self, cx: &mut Context<Self>) {
        self.fixture_chat(true, cx);
        self.input.update(cx, |input, cx| input.set_text("/ti", cx));
        self.palette_index = 1;
        cx.notify();
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_models(&mut self, cx: &mut Context<Self>) {
        use ainc_client::types::ConnectMethod;
        let provider = |id: ProviderId,
                        name: &str,
                        description: &str,
                        connect: ConnectMethod,
                        connected: bool,
                        account: Option<&str>,
                        models: Vec<(&str, &str, bool)>| ProviderStatus {
            id,
            name: name.into(),
            description: description.into(),
            connect,
            connected,
            account: account.map(str::to_owned),
            models: models
                .into_iter()
                .map(|(id, name, featured)| ProviderModel {
                    id: id.into(),
                    name: name.into(),
                    featured,
                })
                .collect(),
            error: None,
            signing_in: false,
            auth_url: None,
            awaiting_code: false,
        };
        self.providers = Some(ProvidersState {
            default_model: Some("codex:model-one".into()),
            providers: vec![
                provider(
                    ProviderId::Claude,
                    "Claude",
                    "Your Claude subscription through Claude Code's sign-in.",
                    ConnectMethod::BrowserCode,
                    true,
                    Some("fixture@example.test · Claude Max"),
                    vec![
                        ("claude:claude-fable-5-1", "Fable 5.1", true),
                        ("claude:claude-sonnet-5", "Sonnet 5", false),
                    ],
                ),
                provider(
                    ProviderId::Codex,
                    "ChatGPT",
                    "Your ChatGPT subscription through Codex's sign-in.",
                    ConnectMethod::Browser,
                    true,
                    Some("fixture@example.test · Plus"),
                    vec![
                        ("codex:model-one", "Codex One", true),
                        ("codex:model-two", "Codex Two", false),
                    ],
                ),
                provider(
                    ProviderId::Openrouter,
                    "OpenRouter",
                    "Hundreds of models with one API key, including Jev.",
                    ConnectMethod::ApiKey,
                    false,
                    None,
                    vec![("openrouter:typesafe/jev-router", "Jev Router", true)],
                ),
            ],
        });
        self.model = Some("codex:model-one".into());
        self.account = Some("Fixture account".into());
        self.credentials_busy = false;
        cx.notify();
    }
    pub fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        if matches!(
            self.overlays.borrow().active(),
            Some(Overlay::RenameConversation(_))
        ) {
            vec![
                self.rename_input.focus_handle(cx),
                self.cancel_focus.clone(),
                self.submit_focus.clone(),
            ]
        } else {
            vec![self.cancel_focus.clone(), self.submit_focus.clone()]
        }
    }
    pub fn overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let active = self.overlays.borrow().active()?;
        let (rename, id) = match active {
            Overlay::RenameConversation(id) => (true, id),
            Overlay::DeleteConversation(id) => (false, id),
            _ => return None,
        };
        let conversation = self.conversations.iter().find(|c| c.id == id)?;
        let title = if rename {
            "Rename conversation".to_owned()
        } else {
            format!("Delete “{}”?", conversation.title)
        };
        let body = if rename {
            column()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(MUTED))
                        .child("Title"),
                )
                .child(
                    row()
                        .min_h(type_size(FIELD_HEIGHT))
                        .px(px(12.))
                        .border_1()
                        .border_color(rgb(if self.form_error.is_some() {
                            ERROR_BORDER
                        } else {
                            BORDER
                        }))
                        .rounded(px(FIELD_RADIUS))
                        .child(self.rename_input.clone()),
                )
                .when_some(self.form_error.clone(), |s, error| {
                    s.child(
                        div()
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(ERROR))
                            .child(error),
                    )
                })
                .into_any_element()
        } else {
            column()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(MUTED))
                        .child("This permanently removes its messages from this Mac."),
                )
                .when_some(self.form_error.clone(), |s, error| {
                    s.child(
                        div()
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(ERROR))
                            .child(error),
                    )
                })
                .into_any_element()
        };
        let enabled = self.store.is_some()
            && self.active.is_none()
            && !self.pending
            && (!rename
                || (self.form_error.is_none()
                    && !self.rename_input.read(cx).content.trim().is_empty()));
        let footer = row()
            .justify_end()
            .gap(px(8.))
            .child(
                self.action_window(
                    "conversation-cancel",
                    "Cancel",
                    None,
                    true,
                    |this, window, cx| {
                        this.overlays.borrow_mut().dismiss(window, cx);
                        cx.notify();
                    },
                    cx,
                )
                .track_focus(&self.cancel_focus)
                .border_1()
                .border_color(rgb(BORDER)),
            )
            .child(
                self.action(
                    "conversation-submit",
                    if rename {
                        "Save"
                    } else {
                        "Delete conversation"
                    },
                    enabled,
                    move |this, cx| {
                        if rename {
                            this.rename(cx)
                        } else {
                            this.delete(cx)
                        }
                    },
                    cx,
                )
                .track_focus(&self.submit_focus)
                .bg(rgb(if rename { PRIMARY } else { DESTRUCTIVE }))
                .text_color(rgb(if rename { PRIMARY_INK } else { TEXT })),
            );
        Some(dialog_shell(title, body, footer).into_any_element())
    }
    fn send(&mut self, cx: &mut Context<Self>) {
        if self.active.is_some() || self.pending || self.credentials_busy {
            return;
        }
        let text = self.input.read(cx).content.trim().to_owned();
        if text.is_empty() {
            return;
        }
        match commands::parse(&text) {
            Parsed::New => {
                self.reset_composer(cx);
                self.new_conversation(cx);
            }
            Parsed::Clear => {
                self.reset_composer(cx);
                self.clear_conversation(cx);
            }
            Parsed::Help => {
                self.reset_composer(cx);
                self.help_open = true;
                self.notice = None;
                self.scroll.scroll_to_bottom();
                cx.notify();
            }
            Parsed::Model(None) => {
                self.reset_composer(cx);
                self.toggle_model_menu(cx);
            }
            Parsed::Model(Some(name)) => {
                self.reset_composer(cx);
                match self.find_model(&name) {
                    Some(id) => self.select_model(Some(id), cx),
                    None => {
                        self.notice = Some(format!(
                            "No connected model matches “{name}”. Type /model to choose one."
                        ));
                        cx.notify();
                    }
                }
            }
            Parsed::Unknown(command) => {
                self.notice = Some(format!(
                    "Unknown command {command}. Type / to see commands."
                ));
                cx.notify();
            }
            Parsed::Prompt { command, prompt } => self.submit(prompt, Some(command), cx),
            Parsed::Plain(prompt) => self.submit(prompt, None, cx),
        }
    }
    fn submit(&mut self, prompt: String, command: Option<String>, cx: &mut Context<Self>) {
        if self.account.is_none() {
            cx.emit(Navigation::Settings);
            return;
        }
        self.help_open = false;
        self.notice = None;
        self.palette_index = 0;
        let conversation = self.conversation;
        self.mutate(
            move |db| {
                let id = match conversation {
                    Some(id) => id,
                    None => db.new_conversation()?,
                };
                let turn = db.begin_turn(id, &prompt, command.as_deref())?;
                Ok((id, turn))
            },
            |this, (conversation, id), cx| {
                this.conversation = Some(conversation);
                this.reload_snapshot();
                this.input.update(cx, |i, cx| {
                    i.reset();
                    cx.notify();
                });
                this.scroll.scroll_to_bottom();
                this.watch_turn(id, cx);
            },
            cx,
        );
    }
    fn clear_conversation(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.conversation else {
            self.new_conversation(cx);
            return;
        };
        self.help_open = false;
        self.mutate(
            move |db| {
                db.command(Command::DeleteConversation { id })?;
                let id = db.new_conversation()?;
                db.command(Command::SelectConversation { id })?;
                Ok(id)
            },
            |this, id, cx| {
                this.conversation = Some(id);
                this.show_chat = true;
                this.reload_snapshot();
                this.notice = Some("Conversation cleared.".into());
                cx.notify();
            },
            cx,
        );
    }
    fn reset_composer(&mut self, cx: &mut Context<Self>) {
        self.palette_index = 0;
        self.input.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
    }
    fn palette(&self, cx: &App) -> Vec<&'static commands::SlashCommand> {
        commands::suggestions(self.input.read(cx).content.trim_start())
    }
    fn accept_suggestion(&mut self, cx: &mut Context<Self>) {
        let items = self.palette(cx);
        if items.is_empty() {
            return;
        }
        let command = items[self.palette_index.min(items.len() - 1)];
        self.palette_index = 0;
        if command.argument.is_some() {
            let text = format!("/{} ", command.name);
            self.input.update(cx, |input, cx| input.set_text(&text, cx));
            cx.notify();
        } else {
            let text = format!("/{}", command.name);
            self.input.update(cx, |input, cx| input.set_text(&text, cx));
            self.send(cx);
        }
    }
    fn toggle_step(&mut self, id: i64, cx: &mut Context<Self>) {
        if !self.expanded_steps.remove(&id) {
            self.expanded_steps.insert(id);
        }
        cx.notify();
    }
    fn retry(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.active.is_some() || self.pending {
            return;
        }
        self.mutate(
            move |db| db.command(Command::Retry { id }),
            move |this, _, cx| this.watch_turn(id, cx),
            cx,
        );
    }
    fn watch_turn(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        self.active = Some(id);
        self.loading_started = Instant::now();
        cx.spawn(async move |this, cx| {
            loop {
                let db = store.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        db.refresh()?;
                        Ok::<_, anyhow::Error>(
                            db.snapshot()
                                .turns
                                .iter()
                                .find(|t| t.id == id)
                                .is_none_or(|t| t.state != "queued" && t.state != "running"),
                        )
                    })
                    .await;
                let done = this
                    .update(cx, |this, cx| {
                        this.reload_snapshot();
                        if let Err(error) = &result {
                            this.error = Some(format!(
                                "Reply continues on daemon. Refresh to reconnect: {error}"
                            ));
                        }
                        this.scroll.scroll_to_bottom();
                        cx.notify();
                        result.as_ref().copied().unwrap_or(true)
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
                cx.background_executor()
                    .spawn(async {
                        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    })
                    .await;
            }
            let _ = this.update(cx, |this, cx| {
                this.appearance = Some(Instant::now());
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save_again(&mut self, cx: &mut Context<Self>) {
        self.mutate(
            |db| db.refresh(),
            |this, _, cx| {
                if let Some(id) = this.active {
                    this.watch_turn(id, cx);
                }
            },
            cx,
        );
    }
    fn action(
        &self,
        id: impl Into<ElementId>,
        label: &str,
        enabled: bool,
        f: impl Fn(&mut Self, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.action_window(id, label, None, enabled, move |this, _, cx| f(this, cx), cx)
    }
    fn action_window(
        &self,
        id: impl Into<ElementId>,
        label: &str,
        semantic_label: Option<&str>,
        enabled: bool,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = id.into();
        let hover_id = id.clone();
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
                label: semantic_label
                    .unwrap_or(if label.is_empty() {
                        "Open conversation"
                    } else {
                        label
                    })
                    .to_owned()
                    .into(),
                kind: ButtonKind::Quiet,
                enabled,
            },
            |button| {
                compact_button(button)
                    .bg(background)
                    .on_hover(on_hover)
                    .child(label.to_owned())
            },
            f,
            cx,
        )
    }
}

#[cfg(test)]
fn fixture_turn(
    id: i64,
    prompt: &str,
    command: Option<&str>,
    state: &str,
    steps: Vec<TurnStep>,
) -> Turn {
    let response = steps
        .iter()
        .filter(|step| step.kind == "text")
        .filter_map(|step| step.content["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    Turn {
        conversation_id: 1,
        id,
        prompt: prompt.into(),
        response: (!response.is_empty()).then_some(response),
        error: None,
        state: state.into(),
        command: command.map(str::to_owned),
        created_at: 1_790_249_400,
        draft: None,
        finished_at: (state == "completed").then_some(1_790_249_460),
        model: Some("codex:model-one".into()),
        started_at: Some(1_790_249_401),
        steps,
    }
}
#[cfg(test)]
fn fixture_step(id: i64, turn_id: i64, kind: &str, content: serde_json::Value) -> TurnStep {
    TurnStep {
        id,
        turn_id,
        kind: kind.into(),
        content: content.as_object().cloned().unwrap_or_default(),
    }
}

/// What a tool call did, in one line: the request for HTTP, the name otherwise.
fn tool_summary(name: &str, input: &serde_json::Value) -> String {
    match name {
        "http_request" => format!(
            "{} {}",
            input["method"].as_str().unwrap_or("GET"),
            input["url"].as_str().unwrap_or("")
        ),
        "ticket_command" => format!(
            "{} {}",
            input["command"]["kind"].as_str().unwrap_or("command"),
            input["command"]["title"].as_str().unwrap_or("")
        )
        .trim()
        .to_owned(),
        "list_runs" => input["status"]
            .as_str()
            .map(|status| format!("status {status}"))
            .unwrap_or_else(|| "latest runs".into()),
        _ => String::new(),
    }
}
/// The response in one line: HTTP status and timing, or the size of the result.
fn result_summary(name: &str, result: &TurnStep) -> String {
    let content = &result.content["content"];
    if result.content["is_error"] == true {
        return content
            .as_str()
            .map(|text| text.chars().take(80).collect())
            .unwrap_or_else(|| "failed".into());
    }
    if name == "http_request"
        && let (Some(status), Some(ms)) =
            (content["status"].as_u64(), content["elapsed_ms"].as_u64())
    {
        return format!("{status} · {ms} ms");
    }
    let size = content.to_string().len();
    if size > 1024 {
        format!("{} KB", size / 1024)
    } else {
        format!("{size} B")
    }
}
fn pretty(value: &serde_json::Value) -> String {
    let text = match value {
        serde_json::Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    if text.chars().count() > 4000 {
        let cut: String = text.chars().take(4000).collect();
        format!("{cut}\n…")
    } else {
        text
    }
}
fn code_panel(text: String) -> Div {
    div()
        .w_full()
        .min_w_0()
        .px(px(10.))
        .py(px(8.))
        .rounded(px(6.))
        .bg(rgb(SURFACE_SEGMENT))
        .font_family("SF Mono")
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(TEXT))
        .child(text)
}
/// The label beside the typing indicator while a turn runs.
fn activity_label(turn: &Turn) -> String {
    if turn.state == "queued" {
        return "Queued…".into();
    }
    match turn.steps.last() {
        Some(step) if step.kind == "tool_use" => format!(
            "Running {}…",
            step.content["name"].as_str().unwrap_or("tool")
        ),
        _ => "Evee is thinking…".into(),
    }
}

impl AssistantPage {
    fn command_chip(&self, command: &str) -> Div {
        let (name, argument) = command
            .split_once(' ')
            .map(|(name, argument)| (name.to_owned(), Some(argument.to_owned())))
            .unwrap_or((command.to_owned(), None));
        row()
            .gap(px(8.))
            .items_center()
            .child(
                div()
                    .px(px(8.))
                    .py(px(2.))
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(SELECTED_BORDER))
                    .bg(rgb(SURFACE_SEGMENT))
                    .font_family("SF Mono")
                    .text_size(type_size(CAPTION_SIZE))
                    .text_color(rgb(TEXT))
                    .child(name),
            )
            .when_some(argument, |chip, argument| {
                chip.child(
                    div()
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(TEXT))
                        .child(argument),
                )
            })
    }
    fn user_bubble(&self, turn: &Turn) -> Div {
        column()
            .gap(px(5.))
            .p(px(12.))
            .ml_auto()
            .max_w(px(620.))
            .rounded(px(10.))
            .bg(rgb(HOVER))
            .child(
                div()
                    .text_size(type_size(10.))
                    .text_color(rgb(MUTED))
                    .child("You"),
            )
            .child(match &turn.command {
                Some(command) => self.command_chip(command),
                None => div()
                    .text_size(type_size(LABEL_SIZE))
                    .child(turn.prompt.clone()),
            })
    }
    fn tool_card(
        &self,
        step: &TurnStep,
        result: Option<&TurnStep>,
        running: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let name = step.content["name"].as_str().unwrap_or("tool").to_owned();
        let summary = tool_summary(&name, &step.content["input"]);
        let (status, color) = match result {
            Some(result) if result.content["is_error"] == true => ("Failed", ERROR),
            Some(_) => ("Done", STATUS_GREEN),
            None if running => ("Running", STATUS_AMBER),
            None => ("Interrupted", MUTED),
        };
        let expanded = self.expanded_steps.contains(&step.id);
        let id = step.id;
        column()
            .w_full()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .overflow_hidden()
            .child(
                row()
                    .px(px(12.))
                    .py(px(8.))
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .size(px(7.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(rgb(color)),
                    )
                    .child(
                        div()
                            .font_family("SF Mono")
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(TEXT_ACCENT))
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
                            .child(match result {
                                Some(result) if !summary.is_empty() => {
                                    format!("{summary} → {}", result_summary(&name, result))
                                }
                                Some(result) => result_summary(&name, result),
                                None => summary,
                            }),
                    )
                    .child(
                        div()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(color))
                            .child(status),
                    )
                    .child(
                        self.action(
                            ("step-toggle", id as u64),
                            if expanded { "Hide" } else { "Details" },
                            true,
                            move |this, cx| this.toggle_step(id, cx),
                            cx,
                        )
                        .debug_selector(move || format!("step-toggle-{id}")),
                    ),
            )
            .when(expanded, |card| {
                card.child(
                    column()
                        .border_t_1()
                        .border_color(rgb(BORDER_SUBTLE))
                        .px(px(12.))
                        .py(px(10.))
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(MUTED))
                                .child("Request"),
                        )
                        .child(code_panel(pretty(&step.content["input"])))
                        .when_some(result, |card, result| {
                            card.child(
                                div()
                                    .text_size(type_size(CAPTION_SIZE))
                                    .text_color(rgb(MUTED))
                                    .child("Response"),
                            )
                            .child(code_panel(pretty(&result.content["content"])))
                        }),
                )
            })
    }
    fn reply_column(&self, turn: &Turn, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let running = self.active == Some(turn.id);
        let mut reply = column().gap(px(10.)).px(px(2.)).max_w(px(720.)).child(
            div()
                .text_size(type_size(10.))
                .text_color(rgb(TEXT_ACCENT))
                .child("Evee"),
        );
        let steps = &turn.steps;
        for (index, step) in steps.iter().enumerate() {
            match step.kind.as_str() {
                "text" => {
                    if let Some(text) = step.content["text"].as_str() {
                        reply = reply.child(markdown::render(text));
                    }
                }
                "tool_use" => {
                    let call = step.content["id"].as_str().unwrap_or_default();
                    let result = steps[index + 1..]
                        .iter()
                        .find(|s| s.kind == "tool_result" && s.content["tool_use_id"] == call);
                    reply = reply.child(self.tool_card(step, result, running, cx));
                }
                _ => {}
            }
        }
        if steps.is_empty()
            && let Some(text) = turn.response.as_deref().filter(|t| !t.trim().is_empty())
        {
            reply = reply.child(markdown::render(text));
        }
        if running {
            if let Some(draft) = turn.draft.as_deref().filter(|d| !d.trim().is_empty()) {
                reply = reply.child(markdown::render(&format!("{draft}▍")));
            } else {
                reply = reply.child(
                    LoadingFrame::new(self.loading_started, window).inline(activity_label(turn)),
                );
            }
        } else if turn.state == "completed" {
            let response = turn.response.clone().unwrap_or_default();
            reply = reply.child(
                row().child(
                    self.action(
                        ("copy", turn.id as u64),
                        "Copy reply",
                        true,
                        move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(response.clone()))
                        },
                        cx,
                    )
                    .text_size(type_size(CAPTION_SIZE))
                    .text_color(rgb(MUTED)),
                ),
            );
        }
        reply
    }
    fn palette_view(
        &self,
        items: &[&'static commands::SlashCommand],
        cx: &mut Context<Self>,
    ) -> Div {
        column()
            .debug_selector(|| "slash-palette".into())
            .gap(px(2.))
            .p(px(4.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(BORDER_OVERLAY))
            .bg(rgb(SURFACE_MENU))
            .children(items.iter().enumerate().map(|(index, command)| {
                let selected = index == self.palette_index.min(items.len() - 1);
                let label = match command.argument {
                    Some(argument) => format!("/{} <{argument}>", command.name),
                    None => format!("/{}", command.name),
                };
                self.action_window(
                    ("slash", index),
                    "",
                    Some(command.name),
                    true,
                    move |this, _, cx| {
                        this.palette_index = index;
                        this.accept_suggestion(cx);
                    },
                    cx,
                )
                .debug_selector(move || format!("slash-{}", command.name))
                .w_full()
                .justify_start()
                .px(px(10.))
                .py(px(7.))
                .rounded(px(6.))
                .bg(rgb(if selected { SELECTED } else { SURFACE_MENU }))
                .child(
                    row()
                        .w_full()
                        .gap(px(10.))
                        .items_center()
                        .child(
                            div()
                                .font_family("SF Mono")
                                .text_size(type_size(LABEL_SIZE))
                                .text_color(rgb(TEXT))
                                .child(label),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(MUTED))
                                .child(command.summary),
                        )
                        .when(selected, |item| item.child(shortcut_badge("⏎"))),
                )
            }))
    }
}

impl Render for AssistantPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let progress = if self.reduced_motion {
            1.
        } else if let Some(start) = self.appearance {
            let t = (start.elapsed().as_secs_f32() / (MESSAGE_MS as f32 / 1000.)).min(1.);
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.appearance = None;
            }
            1. - (1. - t).powi(3)
        } else {
            1.
        };
        let latest = self.turns.last().map(|t| t.id);
        let send_enabled = self.account.is_some()
            && !self.credentials_busy
            && self.active.is_none()
            && !self.pending
            && self.store.is_some()
            && !self.input.read(cx).content.trim().is_empty();
        if !self.show_chat {
            return self
                .conversations_view(cx)
                .id("conversation-list")
                .into_any_element();
        }
        let palette = self.palette(cx);
        let turns: Vec<Turn> = self.turns.clone();
        Page::canvas()
            .child(
                column()
                    .size_full()
                    .min_h_0()
                    .gap(px(12.))
                    .p(px(24.))
                    .max_w(px(900.))
                    .mx_auto()
                    .child(
                        row()
                            .gap(px(8.))
                            .items_center()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
                            .child(
                                self.action(
                                    "back-to-conversations",
                                    "‹ Conversations",
                                    true,
                                    Self::show_list,
                                    cx,
                                )
                                .debug_selector(|| "back-to-conversations".into()),
                            )
                            .child(
                                div().flex_1().min_w_0().truncate().child(
                                    self.conversations
                                        .iter()
                                        .find(|c| Some(c.id) == self.conversation)
                                        .map(|c| c.title.clone())
                                        .unwrap_or("New conversation".into()),
                                ),
                            )
                            .child(self.action(
                                "panel-new",
                                "+",
                                self.active.is_none() && !self.pending,
                                Self::new_conversation,
                                cx,
                            )),
                    )
                    .when(self.account.is_none() && !self.credentials_busy, |s| {
                        s.child(
                            column()
                                .gap(px(5.))
                                .items_start()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(MUTED))
                                .child("Connect Claude, ChatGPT or OpenRouter to chat.")
                                .child(
                                    self.action(
                                        "open-settings",
                                        "Open Settings",
                                        true,
                                        |_, cx| cx.emit(Navigation::Settings),
                                        cx,
                                    )
                                    .border_1()
                                    .border_color(rgb(BORDER)),
                                ),
                        )
                    })
                    .when(self.store.is_none(), |s| {
                        s.child(
                            div()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(ERROR))
                                .child("Conversation data is unavailable. Refresh to reconnect."),
                        )
                    })
                    .when_some(self.error.clone(), |s, error| {
                        s.child(
                            div()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(ERROR))
                                .child(error),
                        )
                    })
                    .when(self.error.is_some(), |s| {
                        s.child(self.action(
                            "refresh-data",
                            "Refresh",
                            !self.pending,
                            Self::save_again,
                            cx,
                        ))
                    })
                    .child(
                        column()
                            .id("chat-history")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .gap(px(16.))
                            .when(turns.is_empty() && !self.help_open, |s| {
                                s.child(
                                    column()
                                        .py(px(16.))
                                        .gap(px(6.))
                                        .text_size(type_size(LABEL_SIZE))
                                        .text_color(rgb(MUTED))
                                        .child(
                                            div()
                                                .text_size(type_size(BODY_SIZE))
                                                .text_color(rgb(TEXT))
                                                .child("What are we working on?"),
                                        )
                                        .child("Ask anything, or type / for commands like /tickets, /runs and /http."),
                                )
                            })
                            .children(turns.iter().map(|turn| {
                                column()
                                    .id(("turn", turn.id as u64))
                                    .gap(px(12.))
                                    .flex_shrink_0()
                                    .when(latest == Some(turn.id), |s| {
                                        s.relative().opacity(0.4 + 0.6 * progress)
                                    })
                                    .child(self.user_bubble(turn))
                                    .when(
                                        !turn.steps.is_empty()
                                            || turn.response.is_some()
                                            || self.active == Some(turn.id),
                                        |s| s.child(self.reply_column(turn, window, cx)),
                                    )
                                    .when_some(turn.error.clone(), |s, error| {
                                        let id = turn.id;
                                        s.child(
                                            column()
                                                .gap(px(6.))
                                                .child(
                                                    div()
                                                        .text_size(type_size(CAPTION_SIZE))
                                                        .text_color(rgb(ERROR))
                                                        .child(error),
                                                )
                                                .child(self.action(
                                                    ("retry", id as u64),
                                                    "Retry reply",
                                                    self.active.is_none() && !self.pending,
                                                    move |this, cx| this.retry(id, cx),
                                                    cx,
                                                )),
                                        )
                                    })
                            }))
                            .when(self.help_open, |s| {
                                s.child(
                                    column()
                                        .debug_selector(|| "help-card".into())
                                        .gap(px(10.))
                                        .p(px(14.))
                                        .max_w(px(720.))
                                        .rounded(px(10.))
                                        .border_1()
                                        .border_color(rgb(BORDER))
                                        .bg(rgb(SURFACE_RAISED))
                                        .child(markdown::render(commands::HELP))
                                        .child(
                                            row().child(
                                                self.action(
                                                    "help-dismiss",
                                                    "Got it",
                                                    true,
                                                    |this, cx| {
                                                        this.help_open = false;
                                                        cx.notify();
                                                    },
                                                    cx,
                                                )
                                                .border_1()
                                                .border_color(rgb(BORDER)),
                                            ),
                                        ),
                                )
                            }),
                    )
                    .when_some(self.notice.clone(), |s, notice| {
                        s.child(
                            div()
                                .debug_selector(|| "composer-notice".into())
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(MUTED))
                                .child(notice),
                        )
                    })
                    .child(
                        column()
                            .id("composer")
                            .debug_selector(|| "composer".into())
                            .flex_shrink_0()
                            .gap(px(8.))
                            .p(px(12.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(SURFACE_COMPOSER))
                            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                let items = this.palette(cx);
                                if items.is_empty() {
                                    return;
                                }
                                match event.keystroke.key.as_str() {
                                    "down" => {
                                        this.palette_index = (this.palette_index + 1) % items.len();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                    "up" => {
                                        this.palette_index =
                                            (this.palette_index + items.len() - 1) % items.len();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                    "enter" | "tab" => {
                                        cx.stop_propagation();
                                        this.accept_suggestion(cx);
                                    }
                                    "escape" => {
                                        cx.stop_propagation();
                                        this.reset_composer(cx);
                                    }
                                    _ => {}
                                }
                            }))
                            .when(!palette.is_empty(), |composer| {
                                composer.child(self.palette_view(&palette, cx))
                            })
                            .child(self.input.clone())
                            .child(
                                row()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(self.model_menu(false, cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(type_size(CAPTION_SIZE))
                                            .text_color(rgb(TEXT_PLACEHOLDER))
                                            .child("/ for commands · ⇧⏎ for a new line"),
                                    )
                                    .child(
                                        self.action_window(
                                            "send",
                                            "",
                                            Some("Send message"),
                                            send_enabled,
                                            |this, _, cx| this.send(cx),
                                            cx,
                                        )
                                        .size(px(30.))
                                        .p(px(0.))
                                        .bg(rgb(HOVER_SEND))
                                        .child(icon("send", 16.).text_color(rgb(TEXT))),
                                    ),
                            ),
                    ),
            )
            .build()
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::conversation_date;

    #[test]
    fn conversation_dates_keep_the_list_compact() {
        use chrono::TimeZone;
        let precise = "2026-09-23 16:06";
        let timestamp = chrono::Local
            .with_ymd_and_hms(2026, 9, 23, 16, 6, 0)
            .single()
            .unwrap()
            .timestamp();
        assert_eq!(
            conversation_date(precise, timestamp, timestamp + 30),
            "Today, 16:06"
        );
        assert_eq!(
            conversation_date(precise, timestamp, timestamp + 100_000),
            "Yesterday"
        );
        assert_eq!(
            conversation_date(precise, timestamp, timestamp + 200_000),
            "09/23/26"
        );
    }
}
