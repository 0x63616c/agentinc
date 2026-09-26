use crate::{
    commands::{self, Parsed},
    input::{Submit, TextInput},
    markdown,
    model_menu::{self, ModelMenu},
    providers::{self, ProvidersState},
    storage::{Command, Conversation, Store, Turn},
    ui::*,
};
use ainc_client::types::TurnStep;
use gpui::{prelude::*, *};
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
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
    menu: ModelMenu,
    credentials_busy: bool,
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
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.send(cx)),
            cx.observe(&input, |_, _, cx| cx.notify()),
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
            menu: ModelMenu::default(),
            credentials_busy: !cfg!(test),
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
                self.providers = Some(state);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub(crate) fn refresh_providers(&mut self, cx: &mut Context<Self>) {
        if self.credentials_busy {
            return;
        }
        self.credentials_busy = true;
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
        cx.notify();
    }
    /// The composer's model chip and its floating list.
    fn model_chip(&self, cx: &mut Context<Self>) -> Div {
        let enabled = self.store.is_some() && !self.pending;
        let label = model_menu::label(&self.providers, &self.model);
        let trigger = self
            .action(
                "model-select",
                &label,
                enabled,
                |this, cx| this.menu.toggle(|this: &mut Self| &mut this.menu, cx),
                cx,
            )
            .h(px(32.))
            .gap(px(6.))
            .border_1()
            .border_color(rgb(BORDER))
            .child(icon("chevronDown", 12.).text_color(rgb(MUTED)));
        model_menu::render(
            &self.menu,
            &self.providers,
            &self.model,
            false,
            enabled,
            trigger,
            |this: &mut Self, model, _, cx| this.select_model(model, cx),
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
        self.refresh_providers(cx);
    }
    fn select_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.menu.close(|this: &mut Self| &mut this.menu, cx);
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
                        serde_json::json!({"text":"Start with the **blocking Ticket**, then the agent run.\n\n1. Review `ticket-142` (blocked on the API contract)\n2. Kick off the nightly automation"}),
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
        self.scroll.scroll_to_bottom();
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
        self.scroll.scroll_to_bottom();
        cx.notify();
    }
    /// The slash palette open with a partial command typed.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_palette(&mut self, cx: &mut Context<Self>) {
        self.fixture_chat(true, cx);
        self.input.update(cx, |input, cx| input.set_text("/ti", cx));
        self.palette_index = 0;
        cx.notify();
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_models(&mut self, cx: &mut Context<Self>) {
        self.providers = Some(fixture_providers());
        self.model = Some("codex:model-one".into());
        self.account = Some("Fixture account".into());
        self.credentials_busy = false;
        cx.notify();
    }
}
#[cfg(test)]
#[allow(dead_code)]
pub fn fixture_providers() -> ProvidersState {
    {
        use ainc_client::types::{ConnectMethod, ProviderId, ProviderModel, ProviderStatus};
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
        ProvidersState {
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
                        ("claude:claude-fable-5-1", "Fable 5.1", false),
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
                        ("codex:model-one", "Codex One", false),
                        ("codex:model-two", "Codex Two", false),
                    ],
                ),
                provider(
                    ProviderId::Openrouter,
                    "OpenRouter",
                    "Hundreds of models with one API key. Jev is the cheap, capable default.",
                    ConnectMethod::ApiKey,
                    false,
                    None,
                    vec![("openrouter:typesafe/jev-router", "Jev Router", true)],
                ),
            ],
        }
    }
}
impl AssistantPage {
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
                self.menu.toggle(|this: &mut Self| &mut this.menu, cx);
            }
            Parsed::Model(Some(name)) => {
                self.reset_composer(cx);
                match model_menu::find(&self.providers, &name) {
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
            // Keep the opened details in view when they belong to the newest turn.
            if self
                .turns
                .last()
                .is_some_and(|turn| turn.steps.iter().any(|step| step.id == id))
            {
                self.scroll.scroll_to_bottom();
            }
        }
        cx.notify();
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active else { return };
        self.mutate(
            move |db| db.command(Command::StopTurn { id }),
            |this, _, cx| {
                this.reload_snapshot();
                cx.notify();
            },
            cx,
        );
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

/// The tool as a person would name it; the identifier stays in the details.
fn tool_title(name: &str) -> String {
    match name {
        "list_tickets" => "List Tickets".into(),
        "ticket_command" => "Ticket command".into(),
        "list_automations" => "List Automations".into(),
        "automation_command" => "Automation command".into(),
        "list_runs" => "List runs".into(),
        "http_request" => "HTTP request".into(),
        other => {
            let mut words = other.replace('_', " ");
            if let Some(first) = words.get(..1) {
                let upper = first.to_uppercase();
                words.replace_range(..1, &upper);
            }
            words
        }
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
    if let Some(tickets) = content["tickets"].as_array() {
        return match tickets.len() {
            1 => "1 Ticket".into(),
            n => format!("{n} Tickets"),
        };
    }
    if let Some(runs) = content["runs"].as_array() {
        return format!("{} runs", runs.len());
    }
    "done".into()
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
            .items_end()
            .gap(px(5.))
            .ml_auto()
            .max_w(px(620.))
            .child(
                div()
                    .text_size(type_size(10.))
                    .text_color(rgb(MUTED))
                    .child("You"),
            )
            .child(match &turn.command {
                Some(command) => self.command_chip(command),
                None => div()
                    .p(px(12.))
                    .rounded(px(10.))
                    .bg(rgb(HOVER))
                    .text_size(type_size(LABEL_SIZE))
                    .child(turn.prompt.clone()),
            })
    }
    fn status_dot(&self, color: u32, running: bool, window: &mut Window) -> Div {
        let mut dot = div()
            .size(px(7.))
            .flex_shrink_0()
            .rounded_full()
            .bg(rgb(color));
        if running && !reduced_motion() {
            window.request_animation_frame();
            let phase = (self.loading_started.elapsed().as_secs_f32() * 3.).sin();
            dot = dot.opacity(0.45 + 0.55 * (phase + 1.) * 0.5);
        }
        dot
    }
    fn tool_card(
        &self,
        step: &TurnStep,
        result: Option<&TurnStep>,
        running: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let name = step.content["name"].as_str().unwrap_or("tool").to_owned();
        let input = step.content["input"].clone();
        let summary = tool_summary(&name, &input);
        let (status, color) = match result {
            Some(result) if result.content["is_error"] == true => ("Failed", ERROR),
            Some(_) => ("Done", STATUS_GREEN),
            None if running => ("Running", STATUS_AMBER),
            None => ("Interrupted", MUTED),
        };
        let expanded = self.expanded_steps.contains(&step.id);
        let id = step.id;
        let empty_input = input.as_object().is_some_and(|map| map.is_empty()) || input.is_null();
        column()
            .w_full()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .overflow_hidden()
            .child(
                row()
                    .pl(px(12.))
                    .pr(px(6.))
                    .py(px(6.))
                    .gap(px(10.))
                    .items_center()
                    .child(self.status_dot(color, running && result.is_none(), window))
                    .child(
                        div()
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(TEXT))
                            .child(tool_title(&name)),
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
                                    format!("{summary} · {}", result_summary(&name, result))
                                }
                                Some(result) => result_summary(&name, result),
                                None => summary,
                            }),
                    )
                    .child(
                        div()
                            .w(px(64.))
                            .flex_shrink_0()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
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
                        .h(px(26.))
                        .min_w(px(64.))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .debug_selector(move || format!("step-toggle-{id}")),
                    ),
            )
            .when(expanded, |card| {
                card.child(
                    column()
                        .id(("step-body", id as u64))
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .border_t_1()
                        .border_color(rgb(BORDER_SUBTLE))
                        .px(px(12.))
                        .py(px(10.))
                        .gap(px(8.))
                        .child(
                            div()
                                .font_family("SF Mono")
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(MUTED))
                                .child(name.clone()),
                        )
                        .child(
                            row()
                                .gap(px(8.))
                                .items_center()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(MUTED))
                                .child("Request")
                                .when(empty_input, |label| label.child("· no arguments")),
                        )
                        .when(!empty_input, |card| card.child(code_panel(pretty(&input))))
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
                    reply = reply.child(self.tool_card(step, result, running, window, cx));
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
                reply = reply
                    .child(markdown::render(draft))
                    .child(LoadingFrame::new(self.loading_started, window).inline(""));
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
                        "Copy",
                        true,
                        move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(response.clone()))
                        },
                        cx,
                    )
                    .h(px(26.))
                    .border_1()
                    .border_color(rgb(BORDER))
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
            .pb(px(8.))
            .border_b_1()
            .border_color(rgb(BORDER))
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
                .bg(rgb(if selected { SELECTED } else { SURFACE_COMPOSER }))
                .child(
                    row()
                        .w_full()
                        .gap(px(12.))
                        .items_center()
                        .child(
                            div()
                                .w(px(150.))
                                .flex_shrink_0()
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
    fn suggestion_chip(&self, command: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        self.action(
            ElementId::Name(format!("suggest-{command}").into()),
            command,
            true,
            move |this, cx| {
                this.input
                    .update(cx, |input, cx| input.set_text(&format!("{command} "), cx));
                cx.notify();
            },
            cx,
        )
        .font_family("SF Mono")
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_full()
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
                            .pb(px(12.))
                            .border_b_1()
                            .border_color(rgb(BORDER_SUBTLE))
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
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(type_size(BODY_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(TEXT))
                                    .child(
                                        self.conversations
                                            .iter()
                                            .find(|c| Some(c.id) == self.conversation)
                                            .map(|c| c.title.clone())
                                            .unwrap_or("New conversation".into()),
                                    ),
                            )
                            .child(
                                self.action_window(
                                    "panel-new",
                                    "",
                                    Some("New conversation"),
                                    self.active.is_none() && !self.pending,
                                    |this, _, cx| this.new_conversation(cx),
                                    cx,
                                )
                                .size(px(28.))
                                .p(px(0.))
                                .child(icon("plus", 14.).text_color(rgb(TEXT))),
                            ),
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
                                        .debug_selector(|| "empty-conversation".into())
                                        .flex_1()
                                        .min_h(px(240.))
                                        .justify_center()
                                        .items_center()
                                        .gap(px(14.))
                                        .child(
                                            div()
                                                .text_size(type_size(DIALOG_TITLE_SIZE + 2.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(TEXT))
                                                .child("What are we working on?"),
                                        )
                                        .child(
                                            div()
                                                .text_size(type_size(LABEL_SIZE))
                                                .text_color(rgb(MUTED))
                                                .child("Evee can list Tickets, watch runs and fetch a URL."),
                                        )
                                        .child(
                                            row()
                                                .gap(px(8.))
                                                .child(self.suggestion_chip("/tickets", cx))
                                                .child(self.suggestion_chip("/runs", cx))
                                                .child(self.suggestion_chip("/http", cx)),
                                        ),
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
                                    .child(self.model_chip(cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(type_size(CAPTION_SIZE))
                                            .text_color(rgb(TEXT_PLACEHOLDER))
                                            .child("/ for commands · ⇧⏎ for a new line"),
                                    )
                                    .child(if let Some(active) = self.active {
                                        let _ = active;
                                        self.action_window(
                                            "stop",
                                            "",
                                            Some("Stop reply"),
                                            !self.pending,
                                            |this, _, cx| this.stop(cx),
                                            cx,
                                        )
                                        .debug_selector(|| "stop".into())
                                        .size(px(32.))
                                        .p(px(0.))
                                        .bg(rgb(PRIMARY))
                                        .child(
                                            div()
                                                .size(px(11.))
                                                .rounded(px(2.))
                                                .bg(rgb(TEXT_ON_PRIMARY)),
                                        )
                                    } else {
                                        self.action_window(
                                            "send",
                                            "",
                                            Some("Send message"),
                                            send_enabled,
                                            |this, _, cx| this.send(cx),
                                            cx,
                                        )
                                        .size(px(32.))
                                        .p(px(0.))
                                        .bg(rgb(if send_enabled { PRIMARY } else { HOVER_SEND }))
                                        .child(icon("send", 16.).text_color(rgb(
                                            if send_enabled { TEXT_ON_PRIMARY } else { TEXT },
                                        )))
                                    }),
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
