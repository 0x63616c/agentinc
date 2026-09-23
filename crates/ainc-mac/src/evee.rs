use crate::{
    assistant,
    input::{Submit, TextInput},
    storage::{Store, Turn},
    style::*,
};
use gpui::{prelude::*, *};
use std::{rc::Rc, time::Instant};

pub struct Evee {
    store: Option<Rc<Store>>,
    turns: Vec<Turn>,
    input: Entity<TextInput>,
    key_input: Entity<TextInput>,
    model_input: Entity<TextInput>,
    key: Option<String>,
    model: String,
    environment_key: bool,
    setup: bool,
    credentials_busy: bool,
    active: Option<i64>,
    error: Option<String>,
    unsaved: Option<i64>,
    scroll: ScrollHandle,
    appearance: Option<Instant>,
    reduced_motion: bool,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl Evee {
    pub fn new(
        mut store: Option<Rc<Store>>,
        storage_error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::field("Ask Evee…", false, cx));
        let key_input = cx.new(|cx| TextInput::field("Paste API key", true, cx));
        let model = std::env::var("OPENAI_MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| assistant::MODEL.into());
        let model_input = cx.new(|cx| {
            let mut field = TextInput::field("Model", false, cx);
            field.set_text(&model, cx);
            field
        });
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.send(cx)),
            cx.observe(&input, |_, _, cx| cx.notify()),
            cx.subscribe(&key_input, |this, _, _: &Submit, cx| {
                this.save_credentials(cx)
            }),
            cx.subscribe(&model_input, |this, _, _: &Submit, cx| {
                this.save_credentials(cx)
            }),
        ];
        let key = std::env::var("OPENAI_API_KEY")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let environment_key = key.is_some();
        let loaded = store
            .as_ref()
            .map(|db| db.recover_interrupted().and_then(|_| db.turns()))
            .transpose();
        let (turns, error) = match loaded {
            Ok(turns) => (turns.unwrap_or_default(), storage_error),
            Err(_) => {
                store = None;
                (
                    vec![],
                    Some(
                        "Could not load chat history. Restart after checking database access."
                            .into(),
                    ),
                )
            }
        };
        if !environment_key {
            let read = cx.read_credentials(assistant::KEYCHAIN_SERVICE);
            cx.spawn(async move |this, cx| {
                let result = read.await;
                let _ = this.update(cx, |this, cx| {
                    this.credentials_busy = false;
                    match result {
                        Ok(Some((model, bytes))) => match String::from_utf8(bytes) {
                            Ok(key) if !key.trim().is_empty() => {
                                this.key = Some(key);
                                this.scroll.scroll_to_bottom();
                                if std::env::var("OPENAI_MODEL").is_err() && !model.is_empty() {
                                    this.model = model;
                                    this.model_input
                                        .update(cx, |input, cx| input.set_text(&this.model, cx));
                                }
                            }
                            _ => {
                                this.error = Some(
                                    "The saved API key is invalid. Replace it in setup.".into(),
                                )
                            }
                        },
                        Ok(None) => this.open_setup(cx),
                        Err(_) => {
                            this.setup = true;
                            this.error = Some(
                                "Keychain could not be read. Unlock it or save your key again."
                                    .into(),
                            );
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        let scroll = ScrollHandle::new();
        if environment_key {
            scroll.scroll_to_bottom();
        }
        Self {
            store,
            turns,
            input,
            key_input,
            model_input,
            key,
            model,
            environment_key,
            setup: false,
            credentials_busy: !environment_key,
            active: None,
            error,
            unsaved: None,
            scroll,
            appearance: None,
            reduced_motion: reduced_motion(),
            hover: HoverFade::default(),
            _subscriptions: subscriptions,
        }
    }
    pub fn open_setup(&mut self, cx: &mut Context<Self>) {
        self.setup = true;
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }
    fn save_credentials(&mut self, cx: &mut Context<Self>) {
        if self.credentials_busy || self.environment_key || self.active.is_some() {
            return;
        }
        let entered = self.key_input.read(cx).content.trim().to_owned();
        let key = if entered.is_empty() {
            self.key.clone().unwrap_or_default()
        } else {
            entered
        };
        let model = self.model_input.read(cx).content.trim().to_owned();
        if key.is_empty()
            || key.len() > 512
            || !key.is_ascii()
            || key.chars().any(char::is_whitespace)
        {
            self.error = Some("Enter a valid API key.".into());
            cx.notify();
            return;
        }
        if model.is_empty()
            || model.len() > 128
            || !model
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-._:".contains(&c))
        {
            self.error = Some("Enter a valid model name.".into());
            cx.notify();
            return;
        }
        self.credentials_busy = true;
        let write = cx.write_credentials(assistant::KEYCHAIN_SERVICE, &model, key.as_bytes());
        cx.spawn(async move |this, cx| {
            let result = write.await;
            let _ = this.update(cx, |this, cx| {
                this.credentials_busy = false;
                if result.is_ok() {
                    this.key = Some(key);
                    this.model = model;
                    this.setup = false;
                    this.error = None;
                    this.key_input.update(cx, |input, cx| {
                        input.reset();
                        cx.notify();
                    });
                } else {
                    this.error =
                        Some("Could not save to Keychain. Your previous key is unchanged.".into());
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn forget_key(&mut self, cx: &mut Context<Self>) {
        if self.credentials_busy || self.environment_key || self.active.is_some() {
            return;
        }
        self.credentials_busy = true;
        let delete = cx.delete_credentials(assistant::KEYCHAIN_SERVICE);
        cx.spawn(async move |this, cx| {
            let result = delete.await;
            let _ = this.update(cx, |this, cx| {
                this.credentials_busy = false;
                if result.is_ok() {
                    this.key = None;
                    this.setup = true;
                    this.error = None;
                    this.key_input.update(cx, |input, cx| {
                        input.reset();
                        cx.notify();
                    });
                } else {
                    this.error = Some("Could not remove the key from Keychain.".into());
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn send(&mut self, cx: &mut Context<Self>) {
        if self.active.is_some() || self.unsaved.is_some() || self.credentials_busy {
            return;
        }
        if self.key.is_none() {
            self.open_setup(cx);
            return;
        }
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let prompt = self.input.read(cx).content.trim().to_owned();
        if prompt.is_empty() {
            return;
        }
        match store.begin_turn(&prompt) {
            Ok(turn) => {
                self.input.update(cx, |input, cx| {
                    input.reset();
                    cx.notify();
                });
                self.turns.push(turn.clone());
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
        if self.key.is_none() {
            self.open_setup(cx);
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
        let Some(key) = self.key.clone() else {
            return;
        };
        let id = current.id;
        self.active = Some(id);
        self.error = None;
        self.appearance = Some(Instant::now());
        self.scroll.scroll_to_bottom();
        let history = self.turns.clone();
        let model = self.model.clone();
        let request = cx
            .background_executor()
            .spawn(async move { assistant::respond(&key, &model, &history, &current) });
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
                this.appearance = Some(Instant::now()); this.scroll.scroll_to_bottom(); cx.notify();
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
        let key_action = f.clone();
        let id = id.into();
        let hover_id = id.clone();
        let background = self.hover.color(&id);
        row()
            .id(id)
            .tab_index(0)
            .justify_center()
            .rounded(px(6.))
            .px(px(10.))
            .py(px(6.))
            .text_size(px(11.))
            .cursor_pointer()
            .opacity(if enabled { 1. } else { 0.4 })
            .bg(background)
            .on_hover(cx.listener(move |this, over, _, cx| {
                this.hover.set(hover_id.clone(), *over);
                cx.notify();
            }))
            .focus(|s| s.bg(rgb(0x1d2520)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                if enabled {
                    f(this, cx);
                }
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    cx.stop_propagation();
                    if enabled {
                        key_action(this, cx);
                    }
                }
            }))
            .child(label.to_owned())
    }
    fn setup_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = !self.credentials_busy && self.active.is_none();
        column().flex_shrink_0().gap(px(12.)).p(px(12.)).rounded(px(8.)).bg(rgb(0x111111))
            .child("OpenAI setup")
            .when(self.environment_key, |s| s.child(div().text_size(px(11.)).text_color(rgb(MUTED)).child("Using OPENAI_API_KEY from the environment. Restart without it to use Keychain.")))
            .when(!self.environment_key, |s| s
                .child(column().gap(px(6.)).child(div().text_size(px(11.)).child("API key"))
                    .child(div().p(px(9.)).rounded(px(6.)).border_1().border_color(rgb(BORDER)).child(self.key_input.clone())))
                .child(column().gap(px(6.)).child(div().text_size(px(11.)).child("Model"))
                    .child(div().p(px(9.)).rounded(px(6.)).border_1().border_color(rgb(BORDER)).child(self.model_input.clone())))
                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child("Save your key to macOS Keychain. Messages are sent to OpenAI when you send."))
                .child(self.action("save-key", if self.credentials_busy {"Saving…"} else {"Save connection"}, enabled, Self::save_credentials, cx).border_1().border_color(rgb(BORDER)))
                .when(self.key.is_some(), |s| s.child(self.action("forget-key", "Remove saved key", enabled, Self::forget_key,cx))))
    }
}
impl Render for Evee {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let progress = if self.reduced_motion {
            1.
        } else if let Some(start) = self.appearance {
            let t = (start.elapsed().as_secs_f32() / 0.22).min(1.);
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
        let send_enabled = !self.credentials_busy
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
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .flex_shrink_0()
                    .child(div().flex_1().child(if self.credentials_busy {
                        "Reading Keychain…"
                    } else if self.key.is_some() {
                        "Key configured"
                    } else {
                        "Connect to start"
                    }))
                    .child(self.action(
                        "setup",
                        if self.setup { "Done" } else { "Setup" },
                        !self.credentials_busy,
                        |this, cx| {
                            this.setup = !this.setup;
                            if this.setup {
                                this.scroll.set_offset(point(px(0.), px(0.)));
                            }
                            cx.notify();
                        },
                        cx,
                    )),
            )
            .when(self.store.is_none(), |s| {
                s.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(0xe6acac))
                        .child("Chat storage is unavailable. Check database access and restart."),
                )
            })
            .when_some(self.error.clone(), |s, error| {
                s.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(0xe6acac))
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
                    .when(self.setup, |s| s.child(self.setup_view(cx)))
                    .when(self.turns.is_empty(), |s| {
                        s.child(
                            column()
                                .py(px(16.))
                                .gap(px(8.))
                                .text_size(px(12.))
                                .text_color(rgb(MUTED))
                                .child("What’s on your mind?")
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .child("Conversation history stays on this Mac."),
                                ),
                        )
                    })
                    .children(self.turns.iter().map(|turn| {
                        column()
                            .id(("turn", turn.id as u64))
                            .gap(px(12.))
                            .flex_shrink_0()
                            .when(latest == Some(turn.id), |s| {
                                s.relative()
                                    .top(px(3. * (1. - progress)))
                                    .opacity(0.4 + 0.6 * progress)
                            })
                            .child(
                                column()
                                    .gap(px(5.))
                                    .p(px(12.))
                                    .rounded(px(10.))
                                    .bg(rgb(0x191919))
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(MUTED))
                                            .child("You"),
                                    )
                                    .child(div().text_size(px(12.)).child(turn.prompt.clone())),
                            )
                            .when_some(turn.response.clone(), |s, reply| {
                                s.child(
                                    column()
                                        .gap(px(5.))
                                        .px(px(2.))
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(rgb(0xb5cabe))
                                                .child("Evee"),
                                        )
                                        .child(div().text_size(px(12.)).child(reply))
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
                                        .text_size(px(12.))
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
                                                .text_size(px(11.))
                                                .text_color(rgb(0xe6acac))
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
                        row()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(10.))
                                    .text_color(rgb(MUTED))
                                    .child("Return to send"),
                            )
                            .child(
                                self.action("send", "Send", send_enabled, Self::send, cx)
                                    .border_1()
                                    .border_color(rgb(0x555555)),
                            ),
                    ),
            )
    }
}
