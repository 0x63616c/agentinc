use crate::{
    assistant,
    input::{Submit, TextInput},
    model::Overlay,
    storage::{Command, Conversation, Store, Turn},
    ui::*,
};
use gpui::{prelude::*, *};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
    overlays: Rc<RefCell<OverlayHost<Overlay>>>,
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
    models: Vec<assistant::Model>,
    model: Option<String>,
    model_menu_open: bool,
    credentials_busy: bool,
    login_cancel: Option<Arc<AtomicBool>>,
    connection_error: Option<String>,
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
impl HoverHost for AssistantPage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}
fn evee_mark(size: f32) -> Img {
    img(ImageSource::Resource(Resource::Embedded("evee.png".into())))
        .size(px(size))
        .rounded_full()
        .flex_shrink_0()
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
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
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
                .spawn(async { assistant::status() });
            cx.spawn(async move |this, cx| {
                let result = request.await;
                let _ = this.update(cx, |this, cx| {
                    this.apply_status(result);
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
            models: vec![],
            model,
            model_menu_open: false,
            credentials_busy: !cfg!(test),
            login_cancel: None,
            connection_error: None,
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
    fn apply_status(&mut self, result: anyhow::Result<(Option<String>, Vec<assistant::Model>)>) {
        self.credentials_busy = false;
        self.login_cancel = None;
        match result {
            Ok((account, models)) => {
                self.account = account;
                self.models = models;
                self.connection_error = None;
            }
            Err(e) => {
                self.account = None;
                self.connection_error = Some(e.to_string());
            }
        }
    }
    fn refresh_connection(&mut self, cx: &mut Context<Self>) {
        if self.credentials_busy || self.active.is_some() {
            return;
        }
        self.credentials_busy = true;
        let request = cx
            .background_executor()
            .spawn(async { assistant::status() });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.apply_status(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn connect(&mut self, cx: &mut Context<Self>) {
        if self.credentials_busy || self.active.is_some() {
            return;
        }
        self.credentials_busy = true;
        self.connection_error = None;
        let cancel = Arc::new(AtomicBool::new(false));
        self.login_cancel = Some(cancel.clone());
        let request = cx.background_executor().spawn(async move {
            assistant::login(cancel, |url| {
                let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
            })
            .and_then(|_| assistant::status())
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.apply_status(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn disconnect(&mut self, cx: &mut Context<Self>) {
        if self.credentials_busy || self.active.is_some() {
            return;
        }
        self.credentials_busy = true;
        let request = cx
            .background_executor()
            .spawn(async { assistant::logout().and_then(|_| assistant::status()) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.apply_status(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn settings_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        self.hover.animate(window);
        let enabled = !self.credentials_busy && self.active.is_none();
        let state = if self.credentials_busy {
            if self.login_cancel.is_some() {
                "Complete sign-in in your browser.".to_owned()
            } else {
                "Checking connection…".to_owned()
            }
        } else if let Some(account) = &self.account {
            format!("Connected as {account}")
        } else {
            "Not connected. Evee uses your ChatGPT subscription.".to_owned()
        };
        let options: Vec<SelectOption> = std::iter::once(SelectOption::new("Codex default"))
            .chain(
                self.models
                    .iter()
                    .map(|model| SelectOption::new(model.name.clone())),
            )
            .collect();
        let model_index = self
            .model
            .as_ref()
            .and_then(|id| self.models.iter().position(|model| &model.id == id))
            .map_or(0, |index| index + 1);
        column()
            .child(settings_row(
                "ChatGPT",
                state,
                if self.account.is_some() {
                    Button::new("codex-sign-in", "Sign out")
                        .secondary()
                        .enabled(enabled)
                        .build(
                            &self.hover,
                            |this: &mut Self, _, cx| this.disconnect(cx),
                            cx,
                        )
                } else {
                    Button::new("codex-sign-in", "Sign in with ChatGPT")
                        .primary()
                        .icon("openai")
                        .enabled(enabled)
                        .build(&self.hover, |this: &mut Self, _, cx| this.connect(cx), cx)
                },
            ))
            .when_some(self.connection_error.clone(), |s, error| {
                s.child(settings_divider()).child(settings_row(
                    "Connection issue",
                    error,
                    Button::new("codex-refresh-error", "Retry")
                        .secondary()
                        .enabled(enabled)
                        .build(
                            &self.hover,
                            |this: &mut Self, _, cx| this.refresh_connection(cx),
                            cx,
                        ),
                ))
            })
            .when(self.login_cancel.is_some(), |s| {
                s.child(settings_divider()).child(settings_row(
                    "Sign-in in progress",
                    "Waiting for your browser to complete sign-in.",
                    Button::new("cancel-sign-in", "Cancel").secondary().build(
                        &self.hover,
                        |this: &mut Self, _, cx| {
                            if let Some(cancel) = &this.login_cancel {
                                cancel.store(true, Ordering::Relaxed);
                            }
                            cx.notify();
                        },
                        cx,
                    ),
                ))
            })
            .when(self.account.is_some(), |s| {
                s.child(settings_divider()).child(settings_row(
                    "Model",
                    "The Codex model Evee replies with.",
                    Select::new("codex-model-select", options)
                        .value(Some(model_index))
                        .open(self.model_menu_open)
                        .enabled(enabled)
                        .width(240.)
                        .build(
                            &self.hover,
                            |this: &mut Self, _, cx| {
                                this.model_menu_open = !this.model_menu_open;
                                cx.notify();
                            },
                            |this: &mut Self, index, _, cx| {
                                let model = index
                                    .checked_sub(1)
                                    .and_then(|index| this.models.get(index))
                                    .map(|model| model.id.clone());
                                this.select_model(model, cx);
                            },
                            cx,
                        ),
                ))
            })
            .child(settings_divider())
            .child(settings_row(
                "Connection status",
                "Refresh your ChatGPT account and available models.",
                Button::new("codex-refresh", "Refresh")
                    .secondary()
                    .icon("refresh")
                    .enabled(enabled)
                    .build(
                        &self.hover,
                        |this: &mut Self, _, cx| this.refresh_connection(cx),
                        cx,
                    ),
            ))
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
        self.reload_snapshot();
    }
    fn select_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.model_menu_open = false;
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
    fn new_conversation_button(&self, id: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        let enabled = self.active.is_none() && !self.pending && self.store.is_some();
        Button::new(id, "New Conversation")
            .primary()
            .icon("plus")
            .enabled(enabled)
            .build(&self.hover, |this, _, cx| this.new_conversation(cx), cx)
    }
    pub fn conversations_view(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let enabled = self.active.is_none() && !self.pending && self.store.is_some();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_secs() as i64);
        Page::document(
            PageHeader::new("Assistant")
                .description("Your conversations with Evee.")
                .actions(self.new_conversation_button("new-chat", cx)),
        )
        .child(
            column()
                .gap(px(SPACE_4))
                .when_some(self.error.clone(), |s, e| s.child(error_text(e)))
                .when(self.conversations.is_empty(), |s| {
                    s.child(
                        EmptyState::new("spark", "Start a conversation")
                            .description(
                                "Ask Evee to plan your day, dig into a Ticket or kick off work.",
                            )
                            .selector("assistant.empty")
                            .action(self.new_conversation_button("new-chat.empty", cx))
                            .build(),
                    )
                })
                .child(
                    column()
                        .gap(px(SPACE_HALF))
                        .children(self.conversations.iter().map(|conversation| {
                            let id = conversation.id;
                            let selected = self.conversation == Some(id);
                            let menu_open = self.overlays.borrow().active()
                                == Some(Overlay::ConversationMenu(id));
                            let snippet = if conversation.snippet.trim().is_empty() {
                                "No messages yet".to_owned()
                            } else {
                                conversation
                                    .snippet
                                    .split_whitespace()
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            };
                            let menu = column()
                                .relative()
                                .child(
                                    Button::new(("chat-menu", id as u64), "Conversation actions")
                                        .icon("more")
                                        .icon_only()
                                        .ghost()
                                        .small()
                                        .enabled(enabled)
                                        .selected(menu_open)
                                        .build(
                                            &self.hover,
                                            move |this, window, cx| {
                                                let mut host = this.overlays.borrow_mut();
                                                if host.active()
                                                    == Some(Overlay::ConversationMenu(id))
                                                {
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
                                        ),
                                )
                                .when(menu_open, |s| {
                                    s.child(floating(
                                        menu_shell(MENU_WIDTH)
                                            .debug_selector(|| "conversation.menu".into())
                                            .child(menu_label(format!(
                                                "Updated {}",
                                                conversation.updated
                                            )))
                                            .child(
                                                MenuEntry::new(
                                                    ("rename-chat", id as u64),
                                                    "Rename",
                                                )
                                                .icon("edit")
                                                .enabled(enabled)
                                                .build(
                                                    &self.hover,
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
                                                ),
                                            )
                                            .child(
                                                MenuEntry::new(
                                                    ("delete-chat", id as u64),
                                                    "Delete",
                                                )
                                                .icon("trash")
                                                .destructive()
                                                .enabled(enabled)
                                                .build(
                                                    &self.hover,
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
                                                ),
                                            ),
                                        Anchor::TopRight,
                                        point(
                                            px(CONTROL_HEIGHT_SM),
                                            px(CONTROL_HEIGHT_SM + SPACE_1),
                                        ),
                                    ))
                                });
                            ListRow::new(("conversation", id as u64), conversation.title.clone())
                                .leading(evee_mark(AVATAR_SIZE))
                                .subtitle(snippet)
                                .selected(selected)
                                .enabled(enabled)
                                .trailing(
                                    row()
                                        .gap(px(SPACE_2))
                                        .child(caption(conversation_date(
                                            &conversation.updated,
                                            conversation.updated_at,
                                            now,
                                        )))
                                        .child(menu),
                                )
                                .build(
                                    &self.hover,
                                    move |this, _, cx| this.open_conversation(id, cx),
                                    cx,
                                )
                        })),
                ),
        )
        .build()
    }
    /// Closes the model select; returns whether anything was open.
    pub fn dismiss_menus(&mut self, cx: &mut Context<Self>) -> bool {
        let was_open = self.model_menu_open;
        self.model_menu_open = false;
        if was_open {
            cx.notify();
        }
        was_open
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
                Turn { conversation_id: 1, id: 1, prompt: "What should I focus on today?".into(), response: Some("Let's prioritize the work. Review your open Tickets, then plan the next agent run.".into()), error: None, state: "done".into() },
                Turn { conversation_id: 1, id: 2, prompt: "Can you help me choose the first one?".into(), response: Some("Yes. Start with the Ticket that is blocking the rest of your plan.".into()), error: None, state: "done".into() },
            ]
        } else {
            vec![]
        };
        self.show_chat = true;
        cx.notify();
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_models(&mut self, cx: &mut Context<Self>) {
        self.account = Some("Fixture account".into());
        self.credentials_busy = false;
        self.models = vec![
            assistant::Model {
                id: "model-one".into(),
                name: "Codex One".into(),
            },
            assistant::Model {
                id: "model-two".into(),
                name: "Codex Two".into(),
            },
        ];
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
    pub fn overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
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
            Field::new(self.rename_input.clone())
                .label("Title")
                .selector("Conversation title")
                .error(self.form_error.clone())
                .build(window, cx)
                .into_any_element()
        } else {
            column()
                .gap(px(SPACE_2))
                .child(caption(
                    "This permanently removes its messages from this Mac.",
                ))
                .when_some(self.form_error.clone(), |s, error| {
                    s.child(error_text(error))
                })
                .into_any_element()
        };
        let enabled = self.store.is_some()
            && self.active.is_none()
            && !self.pending
            && (!rename
                || (self.form_error.is_none()
                    && !self.rename_input.read(cx).content.trim().is_empty()));
        let footer = dialog_footer(
            Button::new("conversation-cancel", "Cancel")
                .secondary()
                .track_focus(&self.cancel_focus)
                .build(
                    &self.hover,
                    |this, window, cx| {
                        this.overlays.borrow_mut().dismiss(window, cx);
                        cx.notify();
                    },
                    cx,
                ),
            Button::new(
                "conversation-submit",
                if rename {
                    "Save"
                } else {
                    "Delete conversation"
                },
            )
            .kind(if rename {
                ButtonKind::Primary
            } else {
                ButtonKind::Destructive
            })
            .enabled(enabled)
            .track_focus(&self.submit_focus)
            .build(
                &self.hover,
                move |this, _, cx| {
                    if rename {
                        this.rename(cx)
                    } else {
                        this.delete(cx)
                    }
                },
                cx,
            ),
        );
        Some(dialog_shell(title, body, footer).into_any_element())
    }
    fn send(&mut self, cx: &mut Context<Self>) {
        if self.active.is_some() || self.pending || self.credentials_busy {
            return;
        }
        if self.account.is_none() {
            cx.emit(Navigation::Settings);
            return;
        }
        let prompt = self.input.read(cx).content.trim().to_owned();
        if prompt.is_empty() {
            return;
        }
        let conversation = self.conversation;
        self.mutate(
            move |db| {
                let id = match conversation {
                    Some(id) => id,
                    None => db.new_conversation()?,
                };
                let turn = db.begin_turn(id, &prompt)?;
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
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
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
    fn turn_view(
        &self,
        turn: &Turn,
        latest: bool,
        progress: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = turn.id;
        column()
            .id(("turn", turn.id as u64))
            .gap(px(SPACE_4))
            .flex_shrink_0()
            .when(latest, |s| s.relative().opacity(0.4 + 0.6 * progress))
            .child(
                row().justify_end().child(
                    div()
                        .max_w(px(620.))
                        .px(px(SPACE_4))
                        .py(px(SPACE_3))
                        .rounded(px(RADIUS_XL))
                        .bg(rgb(SURFACE_CONTROL))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .text_size(type_size(BODY_SIZE))
                        .child(turn.prompt.clone()),
                ),
            )
            .when(
                turn.response.is_some() || self.active == Some(id) || turn.error.is_some(),
                |s| {
                    s.child(
                        row()
                            .items_start()
                            .gap(px(SPACE_3))
                            .child(div().mt(px(2.)).child(evee_mark(AVATAR_SIZE)))
                            .child(
                                column()
                                    .flex_1()
                                    .min_w_0()
                                    .max_w(px(680.))
                                    .gap(px(SPACE_2))
                                    .child(
                                        div()
                                            .text_size(type_size(CAPTION_SIZE))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(TEXT_SECONDARY))
                                            .child("Evee"),
                                    )
                                    .when_some(turn.response.clone(), |s, reply| {
                                        s.child(
                                            div()
                                                .text_size(type_size(BODY_SIZE))
                                                .child(reply.clone()),
                                        )
                                        .child(
                                            row().child(
                                                Button::new(("copy", id as u64), "Copy")
                                                    .ghost()
                                                    .small()
                                                    .icon("copy")
                                                    .build(
                                                        &self.hover,
                                                        move |_, _, cx| {
                                                            cx.write_to_clipboard(
                                                                ClipboardItem::new_string(
                                                                    reply.to_string(),
                                                                ),
                                                            )
                                                        },
                                                        cx,
                                                    ),
                                            ),
                                        )
                                    })
                                    .when(self.active == Some(id), |s| {
                                        s.child(
                                            LoadingFrame::new(self.loading_started, window)
                                                .inline("Evee is thinking…"),
                                        )
                                    })
                                    .when_some(turn.error.clone(), |s, error| {
                                        s.child(error_text(error)).child(
                                            row().child(
                                                Button::new(("retry", id as u64), "Retry")
                                                    .secondary()
                                                    .small()
                                                    .icon("refresh")
                                                    .enabled(self.active.is_none() && !self.pending)
                                                    .build(
                                                        &self.hover,
                                                        move |this, _, cx| this.retry(id, cx),
                                                        cx,
                                                    ),
                                            ),
                                        )
                                    }),
                            ),
                    )
                },
            )
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
        let title = self
            .conversations
            .iter()
            .find(|c| Some(c.id) == self.conversation)
            .map(|c| c.title.clone())
            .unwrap_or("New conversation".into());
        let composer_focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let has_draft = !self.input.read(cx).content.trim().is_empty();
        let turns: Vec<Stateful<Div>> = self
            .turns
            .iter()
            .map(|turn| self.turn_view(turn, latest == Some(turn.id), progress, window, cx))
            .collect();
        Page::canvas()
            .child(
                column()
                    .size_full()
                    .min_h_0()
                    .gap(px(SPACE_3))
                    .p(px(PAGE_X))
                    .max_w(px(900.))
                    .mx_auto()
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .child(
                                row().w(px(160.)).child(
                                    Button::new("back-to-conversations", "Conversations")
                                        .ghost()
                                        .icon("chevronLeft")
                                        .build(&self.hover, |this, _, cx| this.show_list(cx), cx)
                                        .ml(px(-CONTROL_INSET_X)),
                                ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_align(TextAlign::Center)
                                    .text_size(type_size(LABEL_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(title),
                            )
                            .child(
                                row().w(px(160.)).justify_end().child(
                                    Button::new("panel-new", "New Conversation")
                                        .icon("plus")
                                        .icon_only()
                                        .ghost()
                                        .enabled(self.active.is_none() && !self.pending)
                                        .build(&self.hover, |this, _, cx| this.new_conversation(cx), cx)
                                        .mr(px(-SPACE_2)),
                                ),
                            ),
                    )
                    .when(self.account.is_none(), |s| {
                        s.child(
                            column()
                                .flex_1()
                                .items_center()
                                .justify_center()
                                .gap(px(SPACE_4))
                                .child(evee_mark(56.))
                                .child(heading("Connect ChatGPT to chat with Evee"))
                                .child(caption(
                                    "Evee replies through your ChatGPT subscription. Sign in once in Settings.",
                                ))
                                .child(
                                    Button::new("open-settings", "Connect ChatGPT")
                                        .primary()
                                        .icon("openai")
                                        .build(&self.hover, |_, _, cx| cx.emit(Navigation::Settings), cx),
                                ),
                        )
                    })
                    .when(self.store.is_none(), |s| {
                        s.child(error_text(
                            "Conversation data is unavailable. Refresh to reconnect.",
                        ))
                    })
                    .when_some(self.error.clone(), |s, error| {
                        s.child(
                            row()
                                .gap(px(SPACE_3))
                                .child(error_text(error))
                                .child(
                                    Button::new("refresh-data", "Refresh")
                                        .secondary()
                                        .small()
                                        .enabled(!self.pending)
                                        .build(&self.hover, |this, _, cx| this.save_again(cx), cx),
                                ),
                        )
                    })
                    .when(self.pending, |s| s.child(caption("Waiting for acknowledgement…")))
                    .when(self.account.is_some(), |s| {
                        s.child(
                        column()
                            .id("chat-history")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .gap(px(SPACE_6))
                            .py(px(SPACE_2))
                            .when(self.turns.is_empty() && self.account.is_some(), |s| {
                                s.child(
                                    column()
                                        .flex_1()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(SPACE_3))
                                        .child(evee_mark(56.))
                                        .child(heading("What are we working on?"))
                                        .child(caption(
                                            "Evee can plan, research and start work on your Tickets.",
                                        )),
                                )
                            })
                            .children(turns),
                        )
                        .child(
                            column()
                                .flex_shrink_0()
                                .gap(px(SPACE_2))
                                .p(px(SPACE_3))
                                .rounded(px(RADIUS_LG))
                                .border_1()
                                .border_color(rgb(if composer_focused {
                                    FOCUS_FIELD
                                } else {
                                    BORDER
                                }))
                                .bg(rgb(SURFACE_INPUT))
                                .debug_selector(|| "composer".into())
                                .child(self.input.clone())
                                .child(
                                    row()
                                        .justify_between()
                                        .child(if has_draft {
                                            caption("Return to send · Shift+Return for a new line")
                                                .into_any_element()
                                        } else {
                                            div().into_any_element()
                                        })
                                        .child(
                                            Button::new("send", "Send message")
                                                .icon("send")
                                                .icon_only()
                                                .primary()
                                                .enabled(send_enabled)
                                                .build(&self.hover, |this, _, cx| this.send(cx), cx)
                                                .rounded_full(),
                                        ),
                                ),
                        )
                    }),
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
