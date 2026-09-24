use crate::{
    assistant,
    input::{Submit, TextInput},
    overlay::{Overlay, OverlayHost, dialog_shell, menu_shell},
    storage::{Conversation, Store, Turn},
    style::*,
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
    store: Option<Rc<Store>>,
    overlays: Rc<RefCell<OverlayHost>>,
    turns: Vec<Turn>,
    input: Entity<TextInput>,
    conversations: Vec<Conversation>,
    conversation: Option<i64>,
    rename_input: Entity<TextInput>,
    form_error: Option<String>,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    account: Option<String>,
    models: Vec<assistant::Model>,
    model: Option<String>,
    credentials_busy: bool,
    login_cancel: Option<Arc<AtomicBool>>,
    connection_error: Option<String>,
    active: Option<i64>,
    error: Option<String>,
    unsaved: Option<i64>,
    scroll: ScrollHandle,
    appearance: Option<Instant>,
    reduced_motion: bool,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
pub enum Navigation {
    Settings,
    Chat,
}
impl EventEmitter<Navigation> for AssistantPage {}
impl AssistantPage {
    pub fn new(
        mut store: Option<Rc<Store>>,
        storage_error: Option<String>,
        overlays: Rc<RefCell<OverlayHost>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(TextInput::composer);
        let rename_input = cx.new(|cx| TextInput::field("Conversation title", false, cx));
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
        let loaded = store
            .as_ref()
            .map(|db| db.recover_interrupted().and_then(|_| db.conversations()))
            .transpose();
        let (conversations, error) = match loaded {
            Ok(items) => (items.unwrap_or_default(), storage_error),
            Err(_) => {
                store = None;
                (
                    vec![],
                    Some("Could not load conversations. Check database access and restart.".into()),
                )
            }
        };
        let conversation = conversations.first().map(|c| c.id);
        let turns = conversation
            .and_then(|id| store.as_ref().map(|db| db.turns(id)))
            .transpose();
        let (turns, error) = match turns {
            Ok(t) => (t.unwrap_or_default(), error),
            Err(e) => (vec![], Some(e.to_string())),
        };
        let model = store
            .as_ref()
            .and_then(|db| db.setting("model").ok().flatten().filter(|s| !s.is_empty()));
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
        Self {
            store,
            overlays,
            turns,
            input,
            conversations,
            conversation,
            rename_input,
            form_error: None,
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            account: None,
            models: vec![],
            model,
            credentials_busy: true,
            login_cancel: None,
            connection_error: None,
            active: None,
            error,
            unsaved: None,
            scroll: ScrollHandle::new(),
            appearance: None,
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
    pub fn settings_view(&self, cx: &mut Context<Self>) -> Div {
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
            "Not connected".to_owned()
        };
        column()
            .gap(px(12.))
            .child(
                column()
                    .gap(px(10.))
                    .child(
                        row()
                            .justify_between()
                            .gap(px(16.))
                            .child(
                                column().gap(px(3.)).child("ChatGPT").child(
                                    div()
                                        .text_size(px(CAPTION_SIZE))
                                        .text_color(rgb(MUTED))
                                        .child(state),
                                ),
                            )
                            .child(
                                self.action(
                                    "codex-sign-in",
                                    if self.account.is_some() {
                                        "Sign out"
                                    } else {
                                        "Sign in with ChatGPT"
                                    },
                                    enabled,
                                    |this, cx| {
                                        if this.account.is_some() {
                                            this.disconnect(cx)
                                        } else {
                                            this.connect(cx)
                                        }
                                    },
                                    cx,
                                )
                                .border_1()
                                .border_color(rgb(BORDER)),
                            ),
                    )
                    .when_some(self.connection_error.clone(), |s, e| {
                        s.child(
                            div()
                                .text_size(px(CAPTION_SIZE))
                                .text_color(rgb(ERROR))
                                .child(e),
                        )
                    })
                    .when(self.login_cancel.is_some(), |s| {
                        s.child(self.action(
                            "cancel-sign-in",
                            "Cancel sign-in",
                            true,
                            |this, cx| {
                                if let Some(cancel) = &this.login_cancel {
                                    cancel.store(true, Ordering::Relaxed);
                                }
                                cx.notify();
                            },
                            cx,
                        ))
                    })
                    .when(self.account.is_some(), |s| {
                        s.child(
                            column()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .mt(px(8.))
                                        .text_size(px(CAPTION_SIZE))
                                        .text_color(rgb(MUTED))
                                        .child("Model"),
                                )
                                .child(self.action(
                                    "model-default",
                                    if self.model.is_none() {
                                        "✓ Codex default"
                                    } else {
                                        "Codex default"
                                    },
                                    enabled,
                                    |this, cx| this.select_model(None, cx),
                                    cx,
                                ))
                                .children(self.models.iter().enumerate().map(|(index, model)| {
                                    let id = model.id.clone();
                                    let label = format!(
                                        "{}{}",
                                        if self.model.as_ref() == Some(&id) {
                                            "✓ "
                                        } else {
                                            ""
                                        },
                                        model.name
                                    );
                                    self.action(
                                        ("codex-model", index),
                                        &label,
                                        enabled,
                                        move |this, cx| this.select_model(Some(id.clone()), cx),
                                        cx,
                                    )
                                })),
                        )
                    }),
            )
            .child(
                row()
                    .justify_between()
                    .gap(px(12.))
                    .pt(px(16.))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .text_size(px(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
                            .child("Uses your ChatGPT subscription."),
                    )
                    .child(
                        self.action(
                            "codex-refresh",
                            "Refresh",
                            enabled,
                            Self::refresh_connection,
                            cx,
                        )
                        .text_color(rgb(MUTED)),
                    ),
            )
    }
    fn select_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        if let Some(db) = &self.store {
            match db.set_setting("model", model.as_deref().unwrap_or("")) {
                Ok(()) => self.model = model,
                Err(e) => self.connection_error = Some(e.to_string()),
            }
        }
        cx.notify();
    }
    fn reload_conversations(&mut self) {
        if let Some(db) = &self.store {
            match db.conversations() {
                Ok(c) => self.conversations = c,
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    fn open_conversation(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.active.is_some() || self.unsaved.is_some() {
            return;
        }
        if let Some(db) = &self.store {
            match db.turns(id) {
                Ok(turns) => {
                    self.turns = turns;
                    self.conversation = Some(id);
                    self.error = None;
                    self.overlays.borrow_mut().close();
                    self.input.update(cx, |i, cx| {
                        i.reset();
                        cx.notify();
                    });
                    self.scroll.scroll_to_bottom();
                    cx.emit(Navigation::Chat);
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        cx.notify();
    }
    fn new_conversation(&mut self, cx: &mut Context<Self>) {
        if self.active.is_some() || self.unsaved.is_some() {
            return;
        }
        if let Some(db) = &self.store {
            match db.new_conversation() {
                Ok(id) => {
                    self.reload_conversations();
                    self.open_conversation(id, cx);
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        cx.notify();
    }
    fn rename(&mut self, cx: &mut Context<Self>) {
        let active = self.overlays.borrow().active();
        if let (Some(db), Some(Overlay::RenameConversation(id))) = (&self.store, active) {
            if self.form_error.is_some() || self.rename_input.read(cx).content.trim().is_empty() {
                return;
            }
            match db.rename_conversation(id, &self.rename_input.read(cx).content) {
                Ok(()) => {
                    self.overlays.borrow_mut().close();
                    self.form_error = None;
                    self.reload_conversations();
                }
                Err(e) => self.form_error = Some(e.to_string()),
            }
        }
        cx.notify();
    }
    fn delete(&mut self, cx: &mut Context<Self>) {
        if self.active.is_some() || self.unsaved.is_some() {
            return;
        }
        let active = self.overlays.borrow().active();
        if let (Some(db), Some(Overlay::DeleteConversation(id))) = (&self.store, active) {
            match db.delete_conversation(id) {
                Ok(()) => {
                    self.overlays.borrow_mut().close();
                    if self.conversation == Some(id) {
                        self.conversation = None;
                        self.turns.clear();
                    }
                    self.reload_conversations();
                    if self.conversation.is_none()
                        && let Some(next) = self.conversations.first()
                    {
                        self.conversation = Some(next.id);
                        if let Some(db) = &self.store {
                            match db.turns(next.id) {
                                Ok(turns) => self.turns = turns,
                                Err(error) => self.error = Some(error.to_string()),
                            }
                        }
                    }
                }
                Err(e) => self.form_error = Some(e.to_string()),
            }
        }
        cx.notify();
    }
    pub fn conversations_view(&self, cx: &mut Context<Self>) -> Div {
        let enabled = self.active.is_none() && self.unsaved.is_none() && self.store.is_some();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_secs() as i64);
        column()
            .gap(px(16.))
            .child(
                row().child(div().flex_1()).child(
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
            .when_some(self.error.clone(), |s, e| {
                s.child(
                    div()
                        .text_size(px(LABEL_SIZE))
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
                                                .text_size(px(BODY_SIZE))
                                                .text_color(rgb(TEXT))
                                                .truncate()
                                                .child(conversation.title.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(CAPTION_SIZE))
                                                .text_color(rgb(MUTED))
                                                .truncate()
                                                .child(snippet),
                                        ),
                                ),
                            )
                            .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(
                                conversation_date(
                                    &conversation.updated,
                                    conversation.updated_at,
                                    now,
                                ),
                            ))
                            .child(self.action_window(
                                ("chat-menu", id as u64),
                                "…",
                                Some("Conversation actions"),
                                enabled,
                                move |this, window, cx| {
                                    let mut host = this.overlays.borrow_mut();
                                    if host.active() == Some(Overlay::ConversationMenu(id)) {
                                        host.dismiss(window);
                                    } else {
                                        host.open(Overlay::ConversationMenu(id), window, cx, None);
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
                                                .text_size(px(10.))
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
                                                    this.overlays.borrow_mut().open(
                                                        Overlay::RenameConversation(id),
                                                        window,
                                                        cx,
                                                        Some(this.rename_input.focus_handle(cx)),
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
            }))
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
                        .text_size(px(LABEL_SIZE))
                        .text_color(rgb(MUTED))
                        .child("Title"),
                )
                .child(
                    row()
                        .h(px(FIELD_HEIGHT))
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
                            .text_size(px(LABEL_SIZE))
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
                        .text_size(px(LABEL_SIZE))
                        .text_color(rgb(MUTED))
                        .child("This permanently removes its messages from this Mac."),
                )
                .when_some(self.form_error.clone(), |s, error| {
                    s.child(
                        div()
                            .text_size(px(LABEL_SIZE))
                            .text_color(rgb(ERROR))
                            .child(error),
                    )
                })
                .into_any_element()
        };
        let enabled = self.store.is_some()
            && self.active.is_none()
            && self.unsaved.is_none()
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
                        this.overlays.borrow_mut().dismiss(window);
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
        if self.active.is_some() || self.unsaved.is_some() || self.credentials_busy {
            return;
        }
        if self.account.is_none() {
            cx.emit(Navigation::Settings);
            return;
        }
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let prompt = self.input.read(cx).content.trim().to_owned();
        if prompt.is_empty() {
            return;
        }
        let conversation = match self.conversation {
            Some(id) => id,
            None => match store.new_conversation() {
                Ok(id) => {
                    self.conversation = Some(id);
                    id
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    cx.notify();
                    return;
                }
            },
        };
        match store.begin_turn(conversation, &prompt) {
            Ok(turn) => {
                self.input.update(cx, |input, cx| {
                    input.reset();
                    cx.notify();
                });
                self.turns.push(turn.clone());
                self.reload_conversations();
                self.run(turn, cx);
            }
            Err(error) => {
                self.error = Some(format!("Message was not sent: {error}"));
                cx.notify();
            }
        }
    }
    fn retry(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.active.is_some() || self.unsaved.is_some() || self.credentials_busy {
            return;
        }
        if self.account.is_none() {
            cx.emit(Navigation::Settings);
            return;
        }
        let Some(turn) = self.turns.iter_mut().find(|t| t.id == id) else {
            return;
        };
        let mut pending = turn.clone();
        pending.error = None;
        let Some(store) = &self.store else {
            return;
        };
        if store.save_turn(&pending).is_err() {
            self.error = Some("Could not save the retry. Check database access.".into());
            cx.notify();
            return;
        }
        *turn = pending.clone();
        self.run(pending, cx);
    }
    fn run(&mut self, current: Turn, cx: &mut Context<Self>) {
        let id = current.id;
        self.active = Some(id);
        self.error = None;
        self.appearance = Some(Instant::now());
        self.scroll.scroll_to_bottom();
        let history = self.turns.clone();
        let model = self.model.clone();
        let request = cx
            .background_executor()
            .spawn(async move { assistant::respond(model.as_deref(), &history, &current) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.active = None;
                if let Some(turn) = this.turns.iter_mut().find(|t| t.id == id) {
                    match result { Ok(reply) => turn.response = Some(reply), Err(error) => turn.error = Some(error.to_string()) }
                    if this.store.as_ref().is_none_or(|db| db.save_turn(turn).is_err()) {
                        this.unsaved = Some(id);
                        this.error = Some("Reply is in this window but could not be saved. Retry saving before closing.".into());
                    }
                }
                this.reload_conversations(); this.appearance = Some(Instant::now()); this.scroll.scroll_to_bottom(); cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn save_again(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.unsaved
            && let Some(turn) = self.turns.iter().find(|t| t.id == id)
            && self
                .store
                .as_ref()
                .is_some_and(|db| db.save_turn(turn).is_ok())
        {
            self.unsaved = None;
            self.error = None;
            self.reload_conversations();
        }
        cx.notify();
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
                button
                    .justify_center()
                    .px(px(10.))
                    .py(px(6.))
                    .text_size(px(CAPTION_SIZE))
                    .bg(background)
                    .on_hover(on_hover)
                    .child(label.to_owned())
            },
            f,
            cx,
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
            && self.unsaved.is_none()
            && self.store.is_some()
            && !self.input.read(cx).content.trim().is_empty();
        column()
            .size_full()
            .min_h_0()
            .gap(px(12.))
            .child(
                row()
                    .gap(px(8.))
                    .text_size(px(CAPTION_SIZE))
                    .text_color(rgb(MUTED))
                    .child(
                        div().flex_1().child(
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
                        self.active.is_none() && self.unsaved.is_none(),
                        Self::new_conversation,
                        cx,
                    )),
            )
            .when(self.account.is_none(), |s| {
                s.child(
                    column()
                        .gap(px(5.))
                        .items_start()
                        .text_size(px(CAPTION_SIZE))
                        .text_color(rgb(MUTED))
                        .child("Connect ChatGPT to chat.")
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
                        .text_size(px(CAPTION_SIZE))
                        .text_color(rgb(ERROR))
                        .child("Chat storage is unavailable. Check database access and restart."),
                )
            })
            .when_some(self.error.clone(), |s, error| {
                s.child(
                    div()
                        .text_size(px(CAPTION_SIZE))
                        .text_color(rgb(ERROR))
                        .child(error),
                )
            })
            .when(self.unsaved.is_some(), |s| {
                s.child(self.action("save-reply", "Retry saving", true, Self::save_again, cx))
            })
            .child(
                column()
                    .id("chat-history")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .gap(px(16.))
                    .when(self.turns.is_empty() && self.account.is_some(), |s| {
                        s.child(
                            column()
                                .py(px(16.))
                                .gap(px(8.))
                                .text_size(px(LABEL_SIZE))
                                .text_color(rgb(MUTED))
                                .child("What’s on your mind?"),
                        )
                    })
                    .children(self.turns.iter().map(|turn| {
                        column()
                            .id(("turn", turn.id as u64))
                            .gap(px(12.))
                            .flex_shrink_0()
                            .when(latest == Some(turn.id), |s| {
                                s.relative().opacity(0.4 + 0.6 * progress)
                            })
                            .child(
                                column()
                                    .gap(px(5.))
                                    .p(px(12.))
                                    .rounded(px(10.))
                                    .bg(rgb(HOVER))
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(MUTED))
                                            .child("You"),
                                    )
                                    .child(
                                        div().text_size(px(LABEL_SIZE)).child(turn.prompt.clone()),
                                    ),
                            )
                            .when_some(turn.response.clone(), |s, reply| {
                                s.child(
                                    column()
                                        .gap(px(5.))
                                        .px(px(2.))
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(rgb(FOCUS))
                                                .child("Evee"),
                                        )
                                        .child(div().text_size(px(LABEL_SIZE)).child(reply))
                                        .child(self.action(
                                            ("copy", turn.id as u64),
                                            "Copy reply",
                                            true,
                                            {
                                                let response =
                                                    turn.response.clone().unwrap_or_default();
                                                move |_, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(response.clone()),
                                                    )
                                                }
                                            },
                                            cx,
                                        )),
                                )
                            })
                            .when(self.active == Some(turn.id), |s| {
                                s.child(
                                    div()
                                        .px(px(2.))
                                        .text_size(px(LABEL_SIZE))
                                        .text_color(rgb(MUTED))
                                        .child("Evee is thinking…"),
                                )
                            })
                            .when_some(turn.error.clone(), |s, error| {
                                let id = turn.id;
                                s.child(
                                    column()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .text_size(px(CAPTION_SIZE))
                                                .text_color(rgb(ERROR))
                                                .child(error),
                                        )
                                        .child(self.action(
                                            ("retry", id as u64),
                                            "Retry reply",
                                            self.active.is_none() && self.unsaved.is_none(),
                                            move |this, cx| this.retry(id, cx),
                                            cx,
                                        )),
                                )
                            })
                    })),
            )
            .child(
                column()
                    .flex_shrink_0()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0x181818))
                    .child(self.input.clone())
                    .child(
                        row().justify_end().child(
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
                            .bg(rgb(0x2a2a2a))
                            .child(icon("send", 16.).text_color(rgb(TEXT))),
                        ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::conversation_date;

    #[test]
    fn conversation_dates_keep_the_list_compact() {
        let precise = "2026-09-23 16:06";
        assert_eq!(
            conversation_date(precise, 1_000_000, 1_000_030),
            "Today, 16:06"
        );
        assert_eq!(
            conversation_date(precise, 1_000_000, 1_100_000),
            "Yesterday"
        );
        assert_eq!(conversation_date(precise, 1_000_000, 1_200_000), "09/23/26");
    }
}
