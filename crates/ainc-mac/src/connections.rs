//! Connections: the external accounts whose capabilities AgentInc can use.
//! Today that is the ChatGPT Connection Evee replies through, and the Codex
//! model it answers with.
use crate::{
    action::{Failure, Pending, Run},
    assistant,
    daemon::Daemon,
    overlay::Overlay,
    page::{Page, PageOverlays},
    routes::{Destination, Route},
    ui::{OverlayHost, *},
};
use ainc_client::types::ConversationCommand;
use gpui::{prelude::*, *};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MODEL_SELECT: &str = "codex-model-select";

pub struct ConnectionsPage {
    daemon: Arc<Daemon>,
    signed_in_as: Option<String>,
    models: Vec<assistant::Model>,
    model: Option<String>,
    overlays: PageOverlays<()>,
    credentials: Pending,
    login_cancel: Option<Arc<AtomicBool>>,
    connection_error: Option<String>,
    pending: Pending,
}
impl EventEmitter<Destination> for ConnectionsPage {}

impl ConnectionsPage {
    pub fn new(
        daemon: Arc<Daemon>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            daemon,
            signed_in_as: None,
            models: vec![],
            model: None,
            overlays: PageOverlays::new(overlays, Route::Connections),
            credentials: Pending::default(),
            login_cancel: None,
            connection_error: None,
            pending: Pending::default(),
        };
        this.reload_model();
        this.refresh_connection(cx);
        this
    }
    fn reload_model(&mut self) {
        self.model = self
            .daemon
            .product()
            .settings
            .model
            .filter(|s| !s.is_empty());
    }
    /// A Conversation turn is in flight; changing the Connection would cut it off.
    fn turn_active(&self) -> bool {
        self.daemon
            .product()
            .turns
            .iter()
            .any(|t| is_active(&t.state))
    }
    fn apply_status(&mut self, result: Result<(Option<String>, Vec<assistant::Model>), Failure>) {
        self.login_cancel = None;
        match result {
            Ok((account, models)) => {
                self.signed_in_as = account;
                self.models = models;
                self.connection_error = None;
            }
            Err(failure) => {
                self.signed_in_as = None;
                self.connection_error = Some(failure.message("The ChatGPT Connection"));
            }
        }
    }
    /// Run one Connection action and re-read the sign-in state after it.
    fn connection(
        &mut self,
        action: impl FnOnce(&Daemon) -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.turn_active() {
            return;
        }
        let daemon = self.daemon.clone();
        cx.run(
            &self.credentials.clone(),
            move || action(&daemon).and_then(|()| assistant::status(&daemon)),
            |this, result, _| this.apply_status(result),
        );
    }
    fn refresh_connection(&mut self, cx: &mut Context<Self>) {
        self.connection(|_| Ok(()), cx);
    }
    fn connect(&mut self, cx: &mut Context<Self>) {
        if self.credentials.busy() || self.turn_active() {
            return;
        }
        self.connection_error = None;
        let cancel = Arc::new(AtomicBool::new(false));
        self.login_cancel = Some(cancel.clone());
        self.connection(
            move |daemon| {
                assistant::login(daemon, cancel, |url| {
                    let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
                })
            },
            cx,
        );
    }
    fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.connection(assistant::logout, cx);
    }
    fn select_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.overlays.close_popover();
        let daemon = self.daemon.clone();
        cx.run(
            &self.pending.clone(),
            move || {
                Ok(daemon.send(ConversationCommand::SelectModel {
                    model: model.unwrap_or_default(),
                })?)
            },
            |this, result, _| {
                this.reload_model();
                if let Err(failure) = result {
                    this.connection_error = Some(failure.message("The model choice"));
                }
            },
        );
    }
    #[cfg(any(test, feature = "fixtures"))]
    pub fn fixture_selected_model(&self) -> Option<String> {
        self.model.clone()
    }
    #[cfg(any(test, feature = "fixtures"))]
    pub fn fixture_models(&mut self, cx: &mut Context<Self>) {
        self.signed_in_as = Some("Fixture account".into());
        self.connection_error = None;
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
    fn chatgpt_section(&self, ui: &mut Ui<Self>) -> Div {
        let enabled = !self.credentials.busy() && !self.turn_active();
        let state = if self.credentials.busy() {
            if self.login_cancel.is_some() {
                "Complete sign-in in your browser.".to_owned()
            } else {
                "Checking connection…".to_owned()
            }
        } else if let Some(account) = &self.signed_in_as {
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
        settings_section(
            "ChatGPT",
            column()
                .child(settings_row(
                    "ChatGPT",
                    state,
                    if self.signed_in_as.is_some() {
                        Button::new("codex-sign-in", "Sign Out")
                            .secondary()
                            .enabled(enabled)
                            .build(ui, |this: &mut Self, _, cx| this.disconnect(cx))
                    } else {
                        Button::new("codex-sign-in", "Sign in with ChatGPT")
                            .primary()
                            .icon(Icon::OpenAi)
                            .enabled(enabled)
                            .build(ui, |this: &mut Self, _, cx| this.connect(cx))
                    },
                ))
                .when(self.login_cancel.is_some(), |s| {
                    s.child(settings_divider()).child(settings_row(
                        "Sign-in in progress",
                        "Waiting for your browser to complete sign-in.",
                        Button::new("cancel-sign-in", "Cancel").secondary().build(
                            ui,
                            |this: &mut Self, _, cx| {
                                if let Some(cancel) = &this.login_cancel {
                                    cancel.store(true, Ordering::Relaxed);
                                }
                                cx.notify();
                            },
                        ),
                    ))
                })
                .when(self.signed_in_as.is_some(), |s| {
                    s.child(settings_divider()).child(settings_row(
                        "Model",
                        "The Codex model Evee replies with.",
                        Select::new(MODEL_SELECT, options)
                            .value(Some(model_index))
                            .open(self.overlays.popover_open(MODEL_SELECT))
                            .enabled(enabled && !self.pending.busy())
                            .width(SELECT_WIDTH)
                            .build(
                                ui,
                                |this: &mut Self, _, cx| {
                                    this.overlays.toggle_popover(MODEL_SELECT);
                                    cx.notify();
                                },
                                |this: &mut Self, index, _, cx| {
                                    let model = index
                                        .checked_sub(1)
                                        .and_then(|index| this.models.get(index))
                                        .map(|model| model.id.clone());
                                    this.select_model(model, cx);
                                },
                            ),
                    ))
                })
                .child(settings_divider())
                .child(settings_row(
                    "Connection status",
                    "Refresh your ChatGPT account and available models.",
                    Button::new("codex-refresh", "Refresh")
                        .secondary()
                        .icon(Icon::Refresh)
                        .enabled(enabled)
                        .build(ui, |this: &mut Self, _, cx| this.refresh_connection(cx)),
                )),
        )
    }
}

impl Page for ConnectionsPage {
    const ROUTE: Route = Route::Connections;
}

impl Render for ConnectionsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui = &mut Ui::new(window, cx);
        PageFrame::document(
            PageHeader::new(self.title())
                .description("External accounts whose capabilities Evee and your agents can use."),
        )
        .child(
            column()
                .gap(px(SECTION_GAP))
                .when_some(self.connection_error.clone(), |s, error| {
                    s.child(banner(Tone::Danger, error))
                })
                .child(self.chatgpt_section(ui)),
        )
        .build()
    }
}
