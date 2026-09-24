use crate::{
    assistant,
    input::{Submit, TextInput},
    overlay::{Overlay, OverlayHost, dialog_shell, menu_shell},
    storage::{Command, Conversation, Store, Turn},
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
    pending: bool,
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
            rename_input,
            form_error: None,
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            account: None,
            models: vec![],
            model,
            credentials_busy: !cfg!(test),
            login_cancel: None,
            connection_error: None,
            active: None,
            error,
            pending: false,
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
    fn reload_snapshot(&mut self) {
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
    fn select_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
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
                },
                cx,
            );
        }
    }
    pub fn conversations_view(&self, cx: &mut Context<Self>) -> Div {
        let enabled = self.active.is_none() && !self.pending && self.store.is_some();
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
                                        host.dismiss(window, cx);
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
        let request = cx.background_executor().spawn(async move {
            loop {
                store.refresh()?;
                let snapshot = store.snapshot();
                if snapshot
                    .turns
                    .iter()
                    .find(|t| t.id == id)
                    .is_none_or(|t| t.state != "queued" && t.state != "running")
                {
                    return Ok::<_, anyhow::Error>(());
                }
                crate::storage::background(async {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    Ok(())
                })?;
            }
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.reload_snapshot();
                if let Err(error) = result {
                    this.error = Some(format!(
                        "Reply continues on daemon. Refresh to reconnect: {error}"
                    ));
                }
                this.appearance = Some(Instant::now());
                this.scroll.scroll_to_bottom();
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
            && !self.pending
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
                        self.active.is_none() && !self.pending,
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
                        .child("Conversation data is unavailable. Refresh to reconnect."),
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
            .when(self.error.is_some(), |s| {
                s.child(self.action(
                    "refresh-data",
                    "Refresh",
                    !self.pending,
                    Self::save_again,
                    cx,
                ))
            })
            .when(self.pending, |s| {
                s.child(
                    div()
                        .text_color(rgb(MUTED))
                        .child("Waiting for acknowledgement…"),
                )
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
                                            self.active.is_none() && !self.pending,
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
