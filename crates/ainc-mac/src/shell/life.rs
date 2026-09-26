//! The shell's side of the Dashboard, Smart Home and Calendar: their shared
//! models, pages, events and Settings sections.
use super::*;
use crate::{
    calendar::CalendarModel,
    calendar_page::CalendarPage,
    calendar_store::{Access, CalendarSource},
    dashboard::DashboardPage,
    home::{HomeFailed, HomeModel},
    model::OpenRoute,
    smart_home::{SmartHomePage, host},
};
use ainc_client::types::HomeConnectionRequest;

pub(super) struct Life {
    pub home: Entity<HomeModel>,
    pub calendar: Entity<CalendarModel>,
    pub dashboard: Entity<DashboardPage>,
    pub smart_home: Entity<SmartHomePage>,
    pub calendar_page: Entity<CalendarPage>,
    url: Entity<TextInput>,
    client_id: Entity<TextInput>,
    client_secret: Entity<TextInput>,
    connecting: bool,
    connect_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl Life {
    pub(super) fn new(
        store: Option<std::sync::Arc<Store>>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        source: std::sync::Arc<dyn CalendarSource>,
        name: String,
        cx: &mut Context<Shell>,
    ) -> Self {
        let home = cx.new(HomeModel::new);
        let calendar = cx.new(|cx| CalendarModel::new(source, cx));
        let dashboard =
            cx.new(|cx| DashboardPage::new(home.clone(), calendar.clone(), store, name, cx));
        let smart_home = cx.new(|cx| SmartHomePage::new(home.clone(), cx));
        let calendar_page = cx.new(|cx| CalendarPage::new(calendar.clone(), overlays, cx));
        let url = cx.new(|cx| {
            let mut input = TextInput::field("https://app.worldwidewebb.co", false, cx)
                .identified("settings.home.url");
            input.set_text("https://app.worldwidewebb.co", cx);
            input
        });
        let client_id = cx.new(|cx| {
            TextInput::field("Client ID", false, cx).identified("settings.home.client-id")
        });
        let client_secret = cx.new(|cx| {
            TextInput::field("Client secret", true, cx).identified("settings.home.client-secret")
        });
        let open = |this: &mut Shell, event: &OpenRoute, cx: &mut Context<Shell>| {
            this.session.navigate(event.0);
            this.save(cx);
            cx.notify();
        };
        let subscriptions = vec![
            cx.observe(&dashboard, |_, _, cx| cx.notify()),
            cx.observe(&smart_home, |_, _, cx| cx.notify()),
            cx.observe(&calendar_page, |_, _, cx| cx.notify()),
            cx.observe(&url, |_, _, cx| cx.notify()),
            cx.observe(&client_id, |_, _, cx| cx.notify()),
            cx.observe(&client_secret, |_, _, cx| cx.notify()),
            cx.subscribe(&dashboard, move |this, _, event: &OpenRoute, cx| {
                open(this, event, cx)
            }),
            cx.subscribe(&smart_home, move |this, _, event: &OpenRoute, cx| {
                open(this, event, cx)
            }),
            cx.subscribe(
                &dashboard,
                |this, _, event: &crate::automations::OpenTicket, cx| {
                    this.tickets
                        .update(cx, |tickets, cx| tickets.select(event.0, cx));
                    this.session.navigate(Route::Tickets);
                    this.save(cx);
                    cx.notify();
                },
            ),
            cx.subscribe(&home, |this, _, event: &HomeFailed, cx| {
                this.toast(
                    "Smart Home change refused",
                    Some(event.0.clone()),
                    Tone::Danger,
                    cx,
                );
            }),
        ];
        Self {
            home,
            calendar,
            dashboard,
            smart_home,
            calendar_page,
            url,
            client_id,
            client_secret,
            connecting: false,
            connect_error: None,
            _subscriptions: subscriptions,
        }
    }

    /// The page for one of this feature's Routes.
    pub(super) fn page(&self, route: Route) -> Option<AnyElement> {
        match route {
            Route::Dashboard => Some(self.dashboard.clone().into_any_element()),
            Route::SmartHome => Some(self.smart_home.clone().into_any_element()),
            Route::Calendar => Some(self.calendar_page.clone().into_any_element()),
            _ => None,
        }
    }
    /// Keep the home live only while a page that shows it is on screen.
    pub(super) fn showing(&self, route: Route, name: &str, cx: &mut App) {
        let visible = matches!(route, Route::Dashboard | Route::SmartHome);
        self.home
            .update(cx, |home, cx| home.set_visible(visible, cx));
        let name = name.to_owned();
        self.dashboard.update(cx, |page, _| page.set_name(name));
    }
    pub(super) fn notify(&self, cx: &mut App) {
        self.dashboard.update(cx, |_, cx| cx.notify());
        self.smart_home.update(cx, |_, cx| cx.notify());
        self.calendar_page.update(cx, |_, cx| cx.notify());
    }
    pub(super) fn workspace_changed(&self, cx: &mut App) {
        self.home.update(cx, |home, cx| home.refresh(cx));
        self.calendar.update(cx, |calendar, cx| {
            calendar.refresh(cx);
            calendar.sync(cx);
        });
    }
}

impl Shell {
    pub(super) fn open_new_event(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.session.navigate(Route::Calendar);
        self.save(cx);
        self.life.calendar_page.update(cx, |page, cx| {
            page.open_new(crate::calendar::today(), window, cx)
        });
    }
    fn connect_home(&mut self, cx: &mut Context<Self>) {
        let text = |input: &Entity<TextInput>, cx: &Context<Self>| {
            Some(input.read(cx).content.trim().to_owned()).filter(|t| !t.is_empty())
        };
        let request = HomeConnectionRequest {
            base_url: text(&self.life.url, cx).unwrap_or_default(),
            access_client_id: text(&self.life.client_id, cx),
            access_client_secret: text(&self.life.client_secret, cx),
        };
        self.life.connecting = true;
        self.life.connect_error = None;
        let task = self
            .life
            .home
            .update(cx, |home, cx| home.connect(request, cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.life.connecting = false;
                match result {
                    Ok(_) => {
                        this.life
                            .client_secret
                            .update(cx, |input, cx| input.set_text("", cx));
                        this.toast("Control center connected", None, Tone::Success, cx);
                    }
                    Err(error) => this.life.connect_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn disconnect_home(&mut self, cx: &mut Context<Self>) {
        let task = self.life.home.update(cx, |home, cx| home.disconnect(cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.toast(
                        "Could not disconnect",
                        Some(error.to_string()),
                        Tone::Danger,
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn smart_home_settings(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let snapshot = self.life.home.read(cx).snapshot.clone();
        let connection = snapshot.as_ref().and_then(|s| s.connection.clone());
        let rows = match connection {
            Some(connection) => {
                let reachable = snapshot.as_ref().is_some_and(|s| s.reachable);
                column().child(settings_row(
                    "Control center",
                    format!(
                        "{} · {}{}",
                        host(&connection.base_url),
                        if reachable { "connected" } else { "unreachable" },
                        if connection.access_token {
                            " · Access token in Keychain"
                        } else {
                            ""
                        }
                    ),
                    Button::new("settings.home.disconnect", "Disconnect")
                        .secondary()
                        .tint(DESTRUCTIVE_TEXT)
                        .build(
                            &self.hover,
                            |this: &mut Self, _, cx| this.disconnect_home(cx),
                            cx,
                        ),
                ))
            }
            None => column()
                .p(px(SETTINGS_INSET))
                .gap(px(FORM_STACK_GAP))
                .child(
                    column()
                        .gap(px(SPACE_HALF))
                        .child(
                            div()
                                .text_size(type_size(BODY_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Control center"),
                        )
                        .child(caption(
                            "AgentInc switches lights and reads the thermostat through World Wide Webb. The public address sits behind Cloudflare Access and needs a service token; an address without Access needs none. The token is kept in your Keychain.",
                        )),
                )
                .child(
                    column()
                        .max_w(px(FORM_WIDTH))
                        .gap(px(FORM_STACK_GAP))
                        .child(Field::new(self.life.url.clone()).label("Address").build(window, cx))
                        .child(
                            row()
                                .gap(px(CONTROL_GAP))
                                .child(
                                    div().flex_1().child(
                                        Field::new(self.life.client_id.clone())
                                            .label("Access client ID")
                                            .build(window, cx),
                                    ),
                                )
                                .child(
                                    div().flex_1().child(
                                        Field::new(self.life.client_secret.clone())
                                            .label("Access client secret")
                                            .build(window, cx),
                                    ),
                                ),
                        )
                        .when_some(self.life.connect_error.clone(), |s, error| {
                            s.child(error_text(error))
                        })
                        .child(
                            row().child(
                                Button::new(
                                    "settings.home.connect",
                                    if self.life.connecting { "Connecting…" } else { "Connect" },
                                )
                                .primary()
                                .enabled(
                                    !self.life.connecting
                                        && !self.life.url.read(cx).content.trim().is_empty(),
                                )
                                .build(
                                    &self.hover,
                                    |this: &mut Self, _, cx| this.connect_home(cx),
                                    cx,
                                ),
                            ),
                        ),
                ),
        };
        settings_section("Smart Home", rows)
    }

    pub(super) fn calendar_settings(&self, cx: &mut Context<Self>) -> Div {
        let (access, syncing) = {
            let calendar = self.life.calendar.read(cx);
            (calendar.access, calendar.syncing())
        };
        let (description, control): (&str, AnyElement) = match access {
            Access::Granted => (
                "Your Mac's calendars, including iCloud and Google, appear on the Calendar and Dashboard.",
                Button::new(
                    "settings.calendar.sync",
                    if syncing { "Syncing…" } else { "Sync now" },
                )
                .secondary()
                .icon("refresh")
                .enabled(!syncing)
                .build(
                    &self.hover,
                    |this: &mut Self, _, cx| this.life.calendar.update(cx, |c, cx| c.sync(cx)),
                    cx,
                )
                .into_any_element(),
            ),
            Access::NotAsked => (
                "Show the events from your Mac's calendars, including iCloud and Google.",
                Button::new("settings.calendar.allow", "Allow access")
                    .secondary()
                    .icon("calendar")
                    .build(
                        &self.hover,
                        |this: &mut Self, _, cx| {
                            this.life.calendar.update(cx, |c, cx| c.request_access(cx))
                        },
                        cx,
                    )
                    .into_any_element(),
            ),
            Access::Denied => (
                "Access is off. Turn on AgentInc under System Settings → Privacy & Security → Calendars.",
                div().into_any_element(),
            ),
        };
        settings_section(
            "Calendar",
            column().child(settings_row("Mac calendars", description, control)),
        )
    }
}
