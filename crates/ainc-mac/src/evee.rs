//! The Assistant page: the Conversation with Evee, its turns, and the message composer.
use crate::{
    action::{Pending, Run},
    assistant,
    daemon::Daemon,
    input::{Submit, TextInput},
    page::{Drafts, Page, PageOverlays},
    routes::{Destination, Route},
    sync::{SliceChanged, Sync},
    ui::*,
};
use ainc_client::types::{Command, Conversation, Turn};
use anyhow::Context as _;
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};

/// The one dialog this page can have open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
    Rename(i64),
    Delete(i64),
}

/// The popover id of a Conversation row's actions menu.
fn chat_menu_id(id: i64) -> SharedString {
    format!("conversation-menu.{id}").into()
}

pub struct AssistantPage {
    daemon: Arc<Daemon>,
    sync: Entity<Sync>,
    overlays: PageOverlays<Dialog>,
    turns: Vec<Turn>,
    input: Entity<TextInput>,
    conversations: Vec<Conversation>,
    conversation: Option<i64>,
    conversation_open: bool,
    focus_composer: bool,
    rename_input: Entity<TextInput>,
    form_error: Option<String>,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    /// Who the daemon is signed in to ChatGPT as; the Connections page changes it.
    signed_in_as: Option<String>,
    credentials: Pending,
    active: Option<i64>,
    error: Option<String>,
    pending: Pending,
    scroll: ScrollHandle,
    appearance: Option<Instant>,
    loading_started: Instant,
    reduced_motion: bool,
    _subscriptions: Vec<Subscription>,
}
fn evee_mark(size: f32) -> Img {
    img(ImageSource::Resource(Resource::Embedded("evee.png".into())))
        .size(px(size))
        .rounded_full()
        .flex_shrink_0()
}
impl EventEmitter<Destination> for AssistantPage {}
impl AssistantPage {
    pub fn new(
        daemon: Arc<Daemon>,
        sync: Entity<Sync>,
        overlays: Rc<RefCell<OverlayHost<crate::overlay::Overlay>>>,
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
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, event: &SliceChanged, cx| {
                if *event == SliceChanged::Product {
                    this.conversation_changed(cx);
                }
            }),
        ];
        let mut this = Self {
            daemon,
            sync,
            overlays: PageOverlays::new(overlays, Route::Assistant),
            turns: vec![],
            input,
            conversations: vec![],
            conversation: None,
            conversation_open: false,
            focus_composer: false,
            rename_input,
            form_error: None,
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            signed_in_as: None,
            credentials: Pending::default(),
            active: None,
            error: None,
            pending: Pending::default(),
            scroll: ScrollHandle::new(),
            appearance: None,
            loading_started: Instant::now(),
            reduced_motion: reduced_motion(),
            _subscriptions: subscriptions,
        };
        this.refresh_sign_in(cx);
        this.reload_snapshot();
        this
    }
    /// Re-read who the daemon is signed in as; the Connections page may have changed it.
    fn refresh_sign_in(&mut self, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        cx.run(
            &self.credentials.clone(),
            move || assistant::status(&daemon),
            |this, result, _| this.signed_in_as = result.ok().and_then(|(account, _)| account),
        );
    }
    /// The Product slice changed: a turn may have started or finished.
    fn conversation_changed(&mut self, cx: &mut Context<Self>) {
        let was_active = self.active;
        self.reload_snapshot();
        if was_active.is_some() {
            self.scroll.scroll_to_bottom();
            if self.active.is_none() {
                self.appearance = Some(Instant::now());
            }
        } else if self.active.is_some() {
            // Sync saw the same turn and is already on the active cadence.
            self.loading_started = Instant::now();
        }
        cx.notify();
    }
    fn mutate<R: Send + 'static>(
        &mut self,
        operation: impl FnOnce(Arc<Daemon>) -> anyhow::Result<R> + Send + 'static,
        apply: impl FnOnce(&mut Self, R, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let daemon = self.daemon.clone();
        let started = cx.run(
            &self.pending.clone(),
            move || operation(daemon),
            |this, result, cx| {
                this.reload_snapshot();
                match result {
                    Ok(value) => apply(this, value, cx),
                    Err(failure) => {
                        this.error = Some(failure.message("The Conversation"));
                        this.form_error = this.error.clone();
                    }
                }
            },
        );
        if started {
            self.error = None;
        }
    }
    pub(crate) fn reload_snapshot(&mut self) {
        let snapshot = self.daemon.product();
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
            .find(|t| is_active(&t.state))
            .map(|t| t.id);
    }
    /// Show Conversation `id`, with the composer focused.
    fn show_conversation(&mut self, id: i64, cx: &mut Context<Self>) {
        self.conversation = Some(id);
        self.conversation_open = true;
        self.focus_composer = true;
        self.reload_snapshot();
        self.input.update(cx, |i, cx| {
            i.reset();
            cx.notify();
        });
        self.scroll.scroll_to_bottom();
        if let Some(id) = self.active {
            self.watch_turn(id, cx);
        }
        cx.notify();
    }
    pub(crate) fn open_conversation(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.pending.busy() {
            return;
        }
        self.mutate(
            move |db| Ok(db.send(Command::SelectConversation { id })?),
            move |this, _, cx| {
                this.overlays.close();
                this.show_conversation(id, cx);
            },
            cx,
        );
    }
    fn new_conversation(&mut self, cx: &mut Context<Self>) {
        self.mutate(
            |db| {
                let id = new_conversation(&db)?;
                db.send(Command::SelectConversation { id })?;
                Ok(id)
            },
            |this, id, cx| this.show_conversation(id, cx),
            cx,
        );
    }
    fn rename(&mut self, cx: &mut Context<Self>) {
        if let Some(Dialog::Rename(id)) = self.overlays.active() {
            let title = self.rename_input.read(cx).content.trim().to_owned();
            if title.is_empty() || self.form_error.is_some() {
                return;
            }
            self.mutate(
                move |db| Ok(db.send(Command::RenameConversation { id, title })?),
                |this, _, _| {
                    this.overlays.close();
                    this.form_error = None;
                },
                cx,
            );
        }
    }
    fn delete(&mut self, cx: &mut Context<Self>) {
        if let Some(Dialog::Delete(id)) = self.overlays.active() {
            self.mutate(
                move |db| Ok(db.send(Command::DeleteConversation { id })?),
                |this, _, _| {
                    this.overlays.close();
                    this.form_error = None;
                    this.conversation_open = false;
                },
                cx,
            );
        }
    }
    fn new_conversation_button(&self, id: &'static str, ui: &mut Ui<Self>) -> Stateful<Div> {
        let enabled = self.active.is_none() && !self.pending.busy();
        Button::new(id, "New Conversation")
            .primary()
            .icon(Icon::Plus)
            .enabled(enabled)
            .build(ui, |this, _, cx| this.new_conversation(cx))
    }
    fn conversation_menu(
        &self,
        conversation: &Conversation,
        enabled: bool,
        ui: &mut Ui<Self>,
    ) -> Div {
        let id = conversation.id;
        let menu_open = self.overlays.popover_open(&chat_menu_id(id));
        column()
            .relative()
            .child(
                Button::new(("conversation-menu", id as u64), "Conversation Actions")
                    .icon(Icon::More)
                    .icon_only()
                    .ghost()
                    .small()
                    .enabled(enabled)
                    .selected(menu_open)
                    .build(ui, move |this, _, cx| {
                        this.overlays.toggle_popover(chat_menu_id(id));
                        cx.notify();
                    }),
            )
            .when(menu_open, |s| {
                s.child(floating(
                    menu_shell(MENU_WIDTH)
                        .debug_selector(|| "conversation.menu".into())
                        .child(menu_label(format!(
                            "Updated {}",
                            time::absolute(conversation.updated_at)
                        )))
                        .child(
                            MenuEntry::new(("rename-conversation", id as u64), "Rename")
                                .icon(Icon::Edit)
                                .enabled(enabled)
                                .build(ui, move |this, window, cx| {
                                    if let Some(c) = this.conversations.iter().find(|c| c.id == id)
                                    {
                                        this.rename_input
                                            .update(cx, |input, cx| input.set_text(&c.title, cx));
                                    }
                                    this.form_error = None;
                                    let focus = this.rename_input.focus_handle(cx);
                                    this.overlays.open_dialog(
                                        Dialog::Rename(id),
                                        focus,
                                        window,
                                        cx,
                                    );
                                    cx.notify();
                                }),
                        )
                        .child(
                            MenuEntry::new(("delete-conversation", id as u64), "Delete")
                                .icon(Icon::Trash)
                                .destructive()
                                .enabled(enabled)
                                .build(ui, move |this, window, cx| {
                                    this.form_error = None;
                                    let focus = this.cancel_focus.clone();
                                    this.overlays.open_dialog(
                                        Dialog::Delete(id),
                                        focus,
                                        window,
                                        cx,
                                    );
                                    cx.notify();
                                }),
                        ),
                    Anchor::TopRight,
                    point(px(CONTROL_HEIGHT_SM), px(CONTROL_HEIGHT_SM + SPACE_1)),
                ))
            })
    }
    /// The header's icon-only Refresh: every page that polls the daemon has one.
    fn refresh_button(&self, ui: &mut Ui<Self>) -> Stateful<Div> {
        let fetching = self.sync.read(ui.cx).fetching();
        Button::new("assistant.refresh", "Refresh")
            .icon(Icon::Refresh)
            .icon_only()
            .secondary()
            .enabled(!fetching && !self.pending.busy())
            .build(ui, |this, _, cx| this.save_again(cx))
    }
    pub fn conversations_view(&self, ui: &mut Ui<Self>) -> Stateful<Div> {
        let enabled = self.active.is_none() && !self.pending.busy();
        let now = time::now();
        let mut state = self.sync.read(ui.cx).load_state();
        state.error = self.error.clone().or(state.error.take());
        PageFrame::document(
            PageHeader::new(self.title())
                .description("Your Conversations with Evee.")
                .actions(
                    row_gap(CONTROL_GAP)
                        .child(self.refresh_button(ui))
                        .child(self.new_conversation_button("new-conversation", ui)),
                ),
        )
        .child(column().gap(px(SPACE_4)).children(page_frame(
            "assistant",
            &state,
            SKELETON_ROWS,
            ui,
            |ui| {
                if self.conversations.is_empty() {
                    return match state.error {
                        Some(_) => div().into_any_element(),
                        None => EmptyState::new(Icon::Spark, "No Conversations yet.")
                            .description(
                                "Ask Evee to plan your day, dig into a Ticket or kick off work.",
                            )
                            .selector("assistant.empty")
                            .action(
                                Button::new("new-conversation.empty", "New Conversation")
                                    .secondary()
                                    .icon(Icon::Plus)
                                    .enabled(enabled)
                                    .build(ui, |this, _, cx| this.new_conversation(cx)),
                            )
                            .build()
                            .into_any_element(),
                    };
                }
                column()
                    .gap(px(SPACE_HALF))
                    .children(self.conversations.iter().map(|conversation| {
                        let id = conversation.id;
                        let snippet = if conversation.snippet.trim().is_empty() {
                            "No messages yet".to_owned()
                        } else {
                            conversation
                                .snippet
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                        };
                        ListRow::new(("conversation", id as u64), conversation.title.clone())
                            .leading(evee_mark(AVATAR_SIZE))
                            .subtitle(snippet)
                            .enabled(enabled)
                            .trailing(
                                row()
                                    .gap(px(SPACE_2))
                                    .child(caption(time::relative(conversation.updated_at, now)))
                                    .child(self.conversation_menu(conversation, enabled, ui)),
                            )
                            .build(ui, move |this, _, cx| this.open_conversation(id, cx))
                    }))
                    .into_any_element()
            },
        )))
        .build()
    }
    pub fn show_list(&mut self, cx: &mut Context<Self>) {
        self.conversation_open = false;
        cx.notify();
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_conversation(&mut self, populated: bool, cx: &mut Context<Self>) {
        self.signed_in_as = Some("Fixture account".into());
        self.conversation = Some(1);
        self.conversations = vec![Conversation {
            id: 1,
            title: "Planning the day".into(),
            snippet: "Let's prioritize the work.".into(),
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
        self.conversation_open = true;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }
    /// The Conversation view before anyone has signed in to ChatGPT.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_signed_out(&mut self, cx: &mut Context<Self>) {
        self.signed_in_as = None;
        self.conversation_open = true;
        cx.notify();
    }
    fn dialog(&self, ui: &mut Ui<Self>) -> Option<AnyElement> {
        let (rename, id) = match self.overlays.active()? {
            Dialog::Rename(id) => (true, id),
            Dialog::Delete(id) => (false, id),
        };
        let conversation = self.conversations.iter().find(|c| c.id == id)?;
        let (delete_title, delete_body, _) =
            copy::confirm_delete(&conversation.title, "This Conversation and its messages");
        let title = if rename {
            "Rename Conversation".to_owned()
        } else {
            delete_title
        };
        let body = if rename {
            Field::new(self.rename_input.clone())
                .label("Title")
                .selector("Conversation title")
                .error(self.form_error.clone())
                .build(ui)
                .into_any_element()
        } else {
            column()
                .gap(px(SPACE_2))
                .child(caption(delete_body))
                .when_some(self.form_error.clone(), |s, error| {
                    s.child(error_text(error))
                })
                .into_any_element()
        };
        let enabled = self.active.is_none()
            && (!rename
                || (self.form_error.is_none()
                    && !self.rename_input.read(ui.cx).content.trim().is_empty()));
        let footer = DialogFooter::new(if rename { Verb::Save } else { Verb::Delete })
            .ids("conversation-cancel", "conversation-submit")
            .enabled(enabled)
            .pending(self.pending.busy())
            .focus(&self.cancel_focus, &self.submit_focus)
            .build(
                ui,
                |this, window, cx| {
                    this.overlays.dismiss(window, cx);
                    cx.notify();
                },
                move |this, _, cx| {
                    if rename {
                        this.rename(cx)
                    } else {
                        this.delete(cx)
                    }
                },
            );
        Some(dialog_shell(title, body, footer).into_any_element())
    }
    fn send(&mut self, cx: &mut Context<Self>) {
        if self.active.is_some() || self.pending.busy() || self.credentials.busy() {
            return;
        }
        if self.signed_in_as.is_none() {
            cx.emit(Destination::Page(Route::Connections));
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
                    None => new_conversation(&db)?,
                };
                let turn = db
                    .send(Command::Send {
                        conversation_id: id,
                        prompt,
                    })?
                    .context("Missing turn acknowledgement")?;
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
        if self.active.is_some() || self.pending.busy() {
            return;
        }
        self.mutate(
            move |db| Ok(db.send(Command::Retry { id })?),
            move |this, _, cx| this.watch_turn(id, cx),
            cx,
        );
    }
    /// Follow a turn until the daemon reports it finished. `Sync` polls faster
    /// while a turn is active; waking it starts that cadence now.
    fn watch_turn(&mut self, id: i64, cx: &mut Context<Self>) {
        self.active = Some(id);
        self.loading_started = Instant::now();
        self.sync.update(cx, |sync, cx| sync.wake(cx));
        cx.notify();
    }
    fn save_again(&mut self, cx: &mut Context<Self>) {
        self.mutate(
            |db| Ok(db.refresh()?),
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
        ui: &mut Ui<Self>,
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
                        .max_w(px(CHAT_MESSAGE_WIDTH))
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
                                    .max_w(px(CHAT_REPLY_WIDTH))
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
                                                    .icon(Icon::Copy)
                                                    .build(ui, move |_, _, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                reply.to_string(),
                                                            ),
                                                        )
                                                    })
                                                    .ml(px(-CONTROL_INSET_X_SM)),
                                            ),
                                        )
                                    })
                                    .when(self.active == Some(id), |s| {
                                        s.child(
                                            LoadingFrame::new(self.loading_started, ui.window)
                                                .inline("Evee is thinking…"),
                                        )
                                    })
                                    .when_some(turn.error.clone(), |s, error| {
                                        s.child(error_text(error)).child(
                                            row().child(
                                                Button::new(("retry", id as u64), "Retry")
                                                    .secondary()
                                                    .small()
                                                    .icon(Icon::Refresh)
                                                    .enabled(
                                                        self.active.is_none()
                                                            && !self.pending.busy(),
                                                    )
                                                    .build(ui, move |this, _, cx| {
                                                        this.retry(id, cx)
                                                    }),
                                            ),
                                        )
                                    }),
                            ),
                    )
                },
            )
    }
}

impl Page for AssistantPage {
    const ROUTE: Route = Route::Assistant;
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.dialog(&mut Ui::new(window, cx))
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        if matches!(self.overlays.active(), Some(Dialog::Rename(_))) {
            vec![
                self.rename_input.focus_handle(cx),
                self.cancel_focus.clone(),
                self.submit_focus.clone(),
            ]
        } else {
            vec![self.cancel_focus.clone(), self.submit_focus.clone()]
        }
    }
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        anyhow::ensure!(
            !self.pending.busy(),
            "Wait for the current change to finish before installing"
        );
        let mut drafts = Drafts::default();
        drafts.set("conversation", self.conversation);
        drafts.text("input", &self.input, cx);
        drafts.text("rename_input", &self.rename_input, cx);
        Ok(drafts)
    }
    fn restore(&mut self, drafts: Drafts, cx: &mut Context<Self>) {
        if let Some(conversation) = drafts.get::<Option<i64>>("conversation") {
            self.conversation = conversation;
        }
        drafts.restore_text("input", &self.input, cx);
        drafts.restore_text("rename_input", &self.rename_input, cx);
    }
    fn open(&mut self, to: &Destination, _: &mut Window, cx: &mut Context<Self>) {
        if let Destination::Conversation(id) = to {
            self.open_conversation(*id, cx);
        }
    }
    fn shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown {
            self.refresh_sign_in(cx);
        }
    }
}

impl Render for AssistantPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_composer) {
            let focus = self.input.focus_handle(cx);
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
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
        let send_enabled = self.signed_in_as.is_some()
            && !self.credentials.busy()
            && self.active.is_none()
            && !self.pending.busy()
            && !self.input.read(cx).content.trim().is_empty();
        let ui = &mut Ui::new(window, cx);
        if !self.conversation_open {
            return self
                .conversations_view(ui)
                .id("conversation-list")
                .into_any_element();
        }
        let title = self
            .conversations
            .iter()
            .find(|c| Some(c.id) == self.conversation)
            .map(|c| c.title.clone())
            .unwrap_or("New Conversation".into());
        let composer_focused = self
            .input
            .read(ui.cx)
            .focus_handle(ui.cx)
            .is_focused(ui.window);
        let has_draft = !self.input.read(ui.cx).content.trim().is_empty();
        let mut state = self.sync.read(ui.cx).load_state();
        state.error = self.error.clone().or(state.error.take());
        let turns: Vec<Stateful<Div>> = self
            .turns
            .iter()
            .map(|turn| self.turn_view(turn, latest == Some(turn.id), progress, ui))
            .collect();
        let header = PageHeader::new(title)
            .leading(
                Button::new("back-to-conversations", "Conversations")
                    .ghost()
                    .small()
                    .icon(Icon::ChevronLeft)
                    .tint(TEXT_SECONDARY)
                    .build(ui, |this, _, cx| this.show_list(cx))
                    .ml(px(-CONTROL_INSET_X_SM)),
            )
            .actions(
                row_gap(CONTROL_GAP).child(self.refresh_button(ui)).child(
                    Button::new("panel-new", "New Conversation")
                        .icon(Icon::Plus)
                        .icon_only()
                        .secondary()
                        .enabled(self.active.is_none() && !self.pending.busy())
                        .build(ui, |this, _, cx| this.new_conversation(cx)),
                ),
            );
        PageFrame::fill(header)
            .child(
                column()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .gap(px(SPACE_3))
                    .max_w(px(READING_WIDTH))
                    .mx_auto()
                    .children(page_frame("assistant", &state, SKELETON_ROWS, ui, |ui| {
                      column()
                        .flex_1()
                        .min_h_0()
                        .gap(px(SPACE_3))
                    .when(self.signed_in_as.is_none(), |s| {
                        s.child(
                            EmptyState::new(Icon::OpenAi, "No ChatGPT Connection yet.")
                                .description(
                                    "Evee replies through your ChatGPT subscription. Sign in once in Connections.",
                                )
                                .selector("assistant.signed-out")
                                .action(
                                    Button::new("open-connections", "Sign in with ChatGPT")
                                        .primary()
                                        .icon(Icon::OpenAi)
                                        .build(ui, |_, _, cx| cx.emit(Destination::Page(Route::Connections))),
                                )
                                .build(),
                        )
                    })
                    .when(self.pending.busy(), |s| s.child(caption("Waiting for acknowledgement…")))
                    .when(self.signed_in_as.is_some(), |s| {
                        s.child(
                        column()
                            .id("conversation-history")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .gap(px(SPACE_6))
                            .py(px(SPACE_2))
                            .when(self.turns.is_empty(), |s| {
                                s.child(
                                    EmptyState::new(Icon::Spark, "No messages yet.")
                                        .description(
                                            "Evee can plan, research and start work on your Tickets.",
                                        )
                                        .selector("assistant.no-messages")
                                        .build(),
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
                                            caption(format!(
                                                "{} to send · {} for a new line",
                                                shortcuts::SEND.glyph,
                                                shortcuts::NEW_LINE.glyph
                                            ))
                                                .into_any_element()
                                        } else {
                                            div().into_any_element()
                                        })
                                        .child(
                                            Button::new("send", "Send Message")
                                                .icon(Icon::Send)
                                                .icon_only()
                                                .primary()
                                                .enabled(send_enabled)
                                                .build(ui, |this, _, cx| this.send(cx))
                                                .rounded_full(),
                                        ),
                                ),
                        )
                    })
                    .into_any_element()
                    })),
            )
            .build()
            .into_any_element()
    }
}

fn new_conversation(daemon: &Daemon) -> anyhow::Result<i64> {
    daemon
        .send(Command::CreateConversation)?
        .context("Missing conversation acknowledgement")
}

#[cfg(test)]
mod tests {
    use super::{AssistantPage, Conversation, Daemon, Turn, WorkState};
    use crate::sync::{ManualClock, Sync};
    use gpui::{AppContext, TestAppContext};
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    fn page(cx: &mut TestAppContext) -> (Arc<Daemon>, ManualClock, gpui::Entity<AssistantPage>) {
        let daemon = Arc::new(Daemon::in_memory());
        let mut snapshot = daemon.product();
        snapshot.conversations.push(Conversation {
            id: 1,
            title: "Pending reply".into(),
            snippet: String::new(),
            updated_at: 0,
        });
        snapshot.turns.push(Turn {
            conversation_id: 1,
            id: 1,
            prompt: "Hello".into(),
            response: None,
            error: None,
            state: WorkState::Running.as_str().into(),
        });
        daemon.memory().edit(|state| state.product = snapshot);
        let clock = ManualClock::default();
        let sync = cx.new(|cx| Sync::new(daemon.clone(), clock.clone(), cx));
        let page =
            cx.new(|cx| AssistantPage::new(daemon.clone(), sync, Rc::new(RefCell::default()), cx));
        cx.run_until_parked();
        (daemon, clock, page)
    }

    #[gpui::test]
    fn a_running_turn_is_followed_until_the_daemon_finishes_it(cx: &mut TestAppContext) {
        let (daemon, clock, page) = page(cx);
        page.read_with(cx, |page, _| {
            assert_eq!(
                page.active,
                Some(1),
                "the first fetch set finds the running turn"
            );
            assert_eq!(page.turns[0].state, WorkState::Running.as_str());
            assert!(page.appearance.is_none());
            assert!(page.error.is_none());
        });
        daemon.memory().edit(|state| {
            let turn = &mut state.product.turns[0];
            turn.state = "done".into();
            turn.response = Some("Hi!".into());
        });
        clock.tick();
        cx.run_until_parked();
        page.read_with(cx, |page, _| {
            assert_eq!(page.active, None);
            assert_eq!(page.turns[0].response.as_deref(), Some("Hi!"));
            assert!(page.appearance.is_some(), "the reply animates in once");
        });
    }
}
