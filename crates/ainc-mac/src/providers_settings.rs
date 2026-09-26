//! Settings → Providers and Agent tools: sign-ins, API keys, the default
//! model and the HTTP host policy. The daemon owns every credential.
use crate::{
    input::{Submit, TextInput},
    model_menu::{self, ModelMenu},
    providers::{self, ProviderId, ProviderTest, ProvidersState},
    storage::{Command, Store},
    ui::*,
};
use ainc_client::types::{ConnectMethod, HttpPolicy, ProviderStatus};
use gpui::{prelude::*, *};
use std::sync::Arc;

pub struct ProvidersSettings {
    store: Option<Arc<Store>>,
    providers: Option<ProvidersState>,
    model: Option<String>,
    menu: ModelMenu,
    busy: bool,
    busy_provider: Option<ProviderId>,
    error: Option<String>,
    test: Option<(ProviderId, ProviderTest)>,
    notice: Option<String>,
    pending: bool,
    api_key_input: Entity<TextInput>,
    code_input: Entity<TextInput>,
    allow_input: Entity<TextInput>,
    deny_input: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}
impl ProvidersSettings {
    pub fn new(store: Option<Arc<Store>>, cx: &mut Context<Self>) -> Self {
        let api_key_input = cx
            .new(|cx| TextInput::field("sk-or-…", true, cx).identified("providers.openrouter.key"));
        let code_input = cx.new(|cx| {
            TextInput::field("Paste the code", false, cx).identified("providers.claude.code")
        });
        let allow_input =
            cx.new(|cx| TextInput::field("*", false, cx).identified("providers.policy.allow"));
        let deny_input =
            cx.new(|cx| TextInput::field("none", false, cx).identified("providers.policy.deny"));
        let subscriptions = vec![
            cx.subscribe(&api_key_input, |this, _, _: &Submit, cx| {
                this.connect(ProviderId::Openrouter, cx)
            }),
            cx.subscribe(&code_input, |this, _, _: &Submit, cx| this.submit_code(cx)),
            cx.subscribe(&allow_input, |this, _, _: &Submit, cx| this.save_policy(cx)),
            cx.subscribe(&deny_input, |this, _, _: &Submit, cx| this.save_policy(cx)),
        ];
        let mut this = Self {
            store,
            providers: None,
            model: None,
            menu: ModelMenu::default(),
            busy: false,
            busy_provider: None,
            error: None,
            test: None,
            notice: None,
            pending: false,
            api_key_input,
            code_input,
            allow_input,
            deny_input,
            _subscriptions: subscriptions,
        };
        this.load_policy(cx);
        #[cfg(not(test))]
        this.refresh(false, cx);
        this
    }
    fn apply(&mut self, result: anyhow::Result<ProvidersState>) {
        self.busy = false;
        self.busy_provider = None;
        match result {
            Ok(state) => {
                self.model = state.default_model.clone();
                self.error = None;
                self.providers = Some(state);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn provider(&self, id: ProviderId) -> Option<&ProviderStatus> {
        self.providers
            .as_ref()
            .and_then(|state| state.providers.iter().find(|provider| provider.id == id))
    }
    fn run(
        &mut self,
        id: Option<ProviderId>,
        operation: impl FnOnce() -> anyhow::Result<ProvidersState> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.busy_provider = id;
        self.error = None;
        let request = cx.background_executor().spawn(async move { operation() });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.apply(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn refresh(&mut self, refresh: bool, cx: &mut Context<Self>) {
        self.run(None, move || providers::state(refresh), cx);
    }
    fn connect(&mut self, id: ProviderId, cx: &mut Context<Self>) {
        match id {
            ProviderId::Openrouter => {
                let key = self.api_key_input.read(cx).content.trim().to_owned();
                if key.is_empty() {
                    self.error = Some("Paste your OpenRouter API key first.".into());
                    cx.notify();
                    return;
                }
                self.api_key_input.update(cx, |input, cx| {
                    input.reset();
                    cx.notify();
                });
                self.run(
                    Some(id),
                    move || providers::connect(id, Some(key), None),
                    cx,
                );
            }
            ProviderId::Claude | ProviderId::Codex => self.run(
                Some(id),
                move || {
                    providers::follow_sign_in(id, |url| {
                        let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
                    })
                },
                cx,
            ),
        }
    }
    fn submit_code(&mut self, cx: &mut Context<Self>) {
        let code = self.code_input.read(cx).content.trim().to_owned();
        if code.is_empty() {
            self.error = Some("Paste the code shown after signing in.".into());
            cx.notify();
            return;
        }
        self.code_input.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
        self.run(
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
                this.busy = false;
                this.busy_provider = None;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn disconnect(&mut self, id: ProviderId, cx: &mut Context<Self>) {
        self.test = None;
        self.run(Some(id), move || providers::disconnect(id), cx);
    }
    fn test_provider(&mut self, id: ProviderId, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.busy_provider = Some(id);
        self.test = None;
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
                this.busy = false;
                this.busy_provider = None;
                match result {
                    Ok(test) => this.test = Some((id, test)),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn command(&mut self, command: Command, done: &'static str, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        if self.pending {
            return;
        }
        self.pending = true;
        let request = cx
            .background_executor()
            .spawn(async move { store.command(command) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                match result {
                    Ok(_) => this.notice = Some(done.into()),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn select_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.menu.close(|this: &mut Self| &mut this.menu, cx);
        self.model = model.clone();
        self.command(
            Command::SelectModel {
                model: model.unwrap_or_default(),
            },
            "Model saved.",
            cx,
        );
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
        self.command(Command::SetHttpPolicy { policy }, "HTTP policy saved.", cx);
    }
    fn load_policy(&mut self, cx: &mut Context<Self>) {
        let Some(db) = &self.store else { return };
        let policy = db.snapshot().settings.http_policy;
        self.allow_input
            .update(cx, |input, cx| input.set_text(&policy.allow.join(", "), cx));
        self.deny_input
            .update(cx, |input, cx| input.set_text(&policy.deny.join(", "), cx));
    }
    pub(crate) fn workspace_changed(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        self.load_policy(cx);
        self.refresh(false, cx);
    }

    pub fn providers_view(&self, cx: &mut Context<Self>) -> Div {
        let trigger = settings_button(
            "model-select",
            model_menu::label(&self.providers, &self.model),
            self.store.is_some() && !self.pending,
            |this: &mut Self, _, cx| this.menu.toggle(|this: &mut Self| &mut this.menu, cx),
            cx,
        )
        .w_full()
        .justify_between()
        .child(icon("chevronDown", 12.).text_color(rgb(MUTED)));
        column()
            .child(self.provider_row(ProviderId::Claude, cx))
            .child(settings_divider())
            .child(self.provider_row(ProviderId::Codex, cx))
            .child(settings_divider())
            .child(self.provider_row(ProviderId::Openrouter, cx))
            .child(settings_divider())
            .child(settings_row(
                "Model",
                "Used for new replies. The composer can switch models per conversation.",
                model_menu::render(
                    &self.menu,
                    &self.providers,
                    &self.model,
                    true,
                    self.store.is_some() && !self.pending,
                    trigger,
                    |this: &mut Self, model, _, cx| this.select_model(model, cx),
                    cx,
                ),
            ))
            .child(settings_divider())
            .child(settings_row(
                "Connection status",
                match (&self.error, &self.notice) {
                    (Some(error), _) => error.clone(),
                    _ if self.busy => "Checking providers…".to_owned(),
                    (None, Some(notice)) => notice.clone(),
                    (None, None) => "Re-check every sign-in, key and model list.".to_owned(),
                },
                settings_button(
                    "providers-refresh",
                    "Refresh",
                    !self.busy,
                    |this: &mut Self, _, cx| this.refresh(true, cx),
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
                self.field("policy-allow", self.allow_input.clone()),
            ))
            .child(settings_divider())
            .child(settings_row(
                "Denied hosts",
                "Hosts Evee must never call, even when allowed above. `*.example.com` covers subdomains.",
                row()
                    .gap(px(CONTROL_GAP))
                    .items_center()
                    .child(self.field("policy-deny", self.deny_input.clone()))
                    .child(settings_button(
                        "policy-save",
                        "Save",
                        enabled,
                        |this: &mut Self, _, cx| this.save_policy(cx),
                        cx,
                    )),
            ))
    }
    fn field(&self, selector: &'static str, input: Entity<TextInput>) -> Div {
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
    fn title(id: ProviderId) -> &'static str {
        match id {
            ProviderId::Claude => "Claude",
            ProviderId::Codex => "ChatGPT",
            ProviderId::Openrouter => "OpenRouter",
        }
    }
    fn provider_row(&self, id: ProviderId, cx: &mut Context<Self>) -> Div {
        let status = self.provider(id).cloned();
        let busy = self.busy && self.busy_provider == Some(id);
        let enabled = !self.busy;
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
        } else if self.busy {
            "Checking…".to_owned()
        } else {
            "Not checked yet.".to_owned()
        };
        let connect = status
            .as_ref()
            .map(|status| status.connect)
            .unwrap_or(match id {
                ProviderId::Claude => ConnectMethod::BrowserCode,
                ProviderId::Codex => ConnectMethod::Browser,
                ProviderId::Openrouter => ConnectMethod::ApiKey,
            });
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
                match connect {
                    ConnectMethod::Browser => {
                        controls = controls.child(settings_button(
                            ("provider-sign-out", id as usize),
                            "Sign out",
                            enabled,
                            move |this: &mut Self, _, cx| this.disconnect(id, cx),
                            cx,
                        ))
                    }
                    ConnectMethod::ApiKey => {
                        controls = controls.child(settings_button(
                            ("provider-remove-key", id as usize),
                            "Remove key",
                            enabled,
                            move |this: &mut Self, _, cx| this.disconnect(id, cx),
                            cx,
                        ))
                    }
                    // Claude shares the Claude Code sign-in; signing out there is deliberate.
                    ConnectMethod::BrowserCode => {}
                }
            }
            Some(status) if status.awaiting_code => {
                controls = controls
                    .child(self.field("claude-code", self.code_input.clone()))
                    .child(settings_button(
                        "claude-finish",
                        "Finish",
                        !self.busy,
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
            _ => match connect {
                ConnectMethod::BrowserCode => {
                    controls = controls.child(settings_button(
                        "claude-sign-in",
                        "Sign in with Claude",
                        enabled,
                        move |this: &mut Self, _, cx| this.connect(id, cx),
                        cx,
                    ))
                }
                ConnectMethod::Browser => {
                    controls = controls.child(settings_icon_button(
                        "codex-sign-in",
                        "Sign in with ChatGPT",
                        "openai",
                        enabled,
                        move |this: &mut Self, _, cx| this.connect(id, cx),
                        cx,
                    ))
                }
                ConnectMethod::ApiKey => {
                    controls = controls
                        .child(self.field("openrouter-key", self.api_key_input.clone()))
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
            .test
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
            Self::title(id),
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
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture(&mut self, state: ProvidersState, cx: &mut Context<Self>) {
        self.model = state.default_model.clone();
        self.providers = Some(state);
        self.busy = false;
        cx.notify();
    }
}
