#[path = "shell/header.rs"]
mod header;
#[path = "shell/layout.rs"]
mod layout;
#[path = "shell/main_content.rs"]
mod main_content;
#[path = "shell/pane.rs"]
mod pane;
#[path = "shell/right_pane.rs"]
mod right_pane;
#[path = "shell/sidebar.rs"]
mod sidebar;

use crate::{
    input::TextInput,
    model::{Availability, FontChoice, FontSize, PAGES, Route, Session},
    overlay::{Overlay, OverlayHost},
    style::*,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};
actions!(
    control,
    [
        Search,
        GoBack,
        GoForward,
        ToggleSidebar,
        ToggleEvee,
        Escape,
        Down,
        Up,
        Choose,
        FocusNext,
        FocusPrevious,
        Quit
    ]
);
#[derive(Clone, PartialEq, Action)]
#[action(namespace = control, no_json)]
struct NavigateRoute(u8);

#[derive(Clone, Copy)]
enum Control {
    Open(Route),
    Navigate(Route),
    Back,
    Forward,
    Search,
    Sidebar,
    Evee,
    Notifications,
    MarkAllRead,
    Dismiss,
    Font(FontChoice),
    FontSize(FontSize),
}
pub struct Shell {
    session: Session,
    overlays: Rc<RefCell<OverlayHost>>,
    assistant: Entity<crate::evee::AssistantPage>,
    tickets: Entity<crate::tickets::TicketsPage>,
    automations: Entity<crate::automations::AutomationsPage>,
    _automation_subscriptions: Vec<Subscription>,
    _tickets_subscription: Subscription,
    _update_subscription: Option<Subscription>,
    _assistant_subscriptions: Vec<Subscription>,
    profile: crate::profile::Profile,
    path: PathBuf,
    focus: FocusHandle,
    pane_focus: [FocusHandle; 2],
    input: Entity<TextInput>,
    picker_result_focus: Vec<FocusHandle>,
    picker_close_focus: FocusHandle,
    _input_subscription: Subscription,
    command_held: bool,
    palette_transition: Option<Instant>,
    selected: usize,
    notification_items: Vec<Notification>,
    save_error: bool,
    session_writable: bool,
    resizing: Option<pane::Side>,
    grip_opacity: [f32; 2],
    grip_animation: [Option<(Instant, f32, f32)>; 2],
    pane_visible: [f32; 2],
    pane_animation: [Option<(Instant, f32, f32)>; 2],
    #[cfg(test)]
    titlebar_zoom_requests: usize,
}
struct Notification {
    icon: &'static str,
    title: String,
    body: String,
    relative_time: String,
    unread: bool,
}
impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = std::env::var_os("AGENTINC_SESSION_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| ainc_release::identity::support_dir().join("session.json"));
        let store = Some(std::sync::Arc::new(crate::storage::Store::new()));
        let storage_error = None;
        let request = cx
            .background_executor()
            .spawn(async { crate::profile::Profile::local() });
        cx.spawn(async move |this, cx| {
            let profile = request.await;
            let _ = this.update(cx, |this, cx| {
                this.profile = profile;
                cx.notify();
            });
        })
        .detach();
        Self::with_state(
            path,
            store,
            storage_error,
            crate::profile::Profile {
                name: "Profile".into(),
                photo: None,
            },
            window,
            cx,
        )
    }

    fn with_state(
        path: PathBuf,
        store: Option<std::sync::Arc<crate::storage::Store>>,
        storage_error: Option<String>,
        profile: crate::profile::Profile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let overlays = Rc::new(RefCell::new(OverlayHost::default()));
        let assistant = cx.new(|cx| {
            crate::evee::AssistantPage::new(
                store.clone(),
                storage_error.clone(),
                overlays.clone(),
                cx,
            )
        });
        let assistant_subscriptions = vec![
            cx.observe(&assistant, |_, _, cx| cx.notify()),
            cx.subscribe(
                &assistant,
                |this, _, event: &crate::evee::Navigation, cx| {
                    match event {
                        crate::evee::Navigation::Settings => this.session.navigate(Route::Settings),
                        crate::evee::Navigation::Chat => {
                            if !this.session.panes[pane::Side::Right.index()].open {
                                this.toggle_pane(pane::Side::Right);
                            }
                        }
                    }
                    this.save(cx);
                    cx.notify();
                },
            ),
        ];
        let tickets = cx.new(|cx| {
            crate::tickets::TicketsPage::new(
                store.clone(),
                storage_error.clone(),
                overlays.clone(),
                cx,
            )
        });
        let automations =
            cx.new(|cx| crate::automations::AutomationsPage::new(store, storage_error, cx));
        let automation_subscriptions = vec![
            cx.observe(&automations, |_, _, cx| cx.notify()),
            cx.subscribe(
                &automations,
                |this, _, event: &crate::automations::OpenTicket, cx| {
                    this.tickets
                        .update(cx, |tickets, cx| tickets.select(event.0, cx));
                    this.session.navigate(Route::Tickets);
                    this.save(cx);
                    cx.notify();
                },
            ),
        ];
        let tickets_subscription = cx.observe(&tickets, |_, _, cx| cx.notify());
        let input = cx.new(TextInput::new);
        let subscription = cx.observe(&input, |this, _, cx| {
            this.selected = 0;
            cx.notify();
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let loaded = Session::load_checked(&path);
        let session_writable = loaded.is_ok();
        let session = loaded.unwrap_or_default();
        set_type_scale(session.font_size.scale());
        let pane_visible = session
            .panes
            .map(|pane| if pane.open { pane.width } else { 0. });
        let update_subscription = cx
            .try_global::<crate::updates::Updates>()
            .cloned()
            .map(|updates| cx.observe(&updates.0, |_, _, cx| cx.notify()));
        let mut shell = Self {
            session,
            overlays,
            assistant,
            tickets,
            automations,
            _automation_subscriptions: automation_subscriptions,
            _tickets_subscription: tickets_subscription,
            _update_subscription: update_subscription,
            _assistant_subscriptions: assistant_subscriptions,
            profile,
            pane_visible,
            pane_animation: [None; 2],
            #[cfg(test)]
            titlebar_zoom_requests: 0,
            path,
            focus,
            pane_focus: [cx.focus_handle(), cx.focus_handle()],
            input,
            picker_result_focus: PAGES.iter().map(|_| cx.focus_handle()).collect(),
            picker_close_focus: cx.focus_handle(),
            _input_subscription: subscription,
            command_held: false,
            palette_transition: None,
            selected: 0,
            notification_items: Vec::new(),
            save_error: !session_writable,
            session_writable,
            resizing: None,
            grip_opacity: [0.; 2],
            grip_animation: [None; 2],
        };
        shell.restore_update_drafts(cx);
        shell
    }
    #[cfg(test)]
    pub(crate) fn fixture(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = crate::storage::Store::open(&path.with_extension("sqlite3")).unwrap();
        Self::with_state(
            path,
            Some(std::sync::Arc::new(store)),
            None,
            crate::profile::Profile {
                name: "QA Profile".into(),
                photo: None,
            },
            window,
            cx,
        )
    }

    #[cfg(test)]
    pub(crate) fn fixture_state(&self) -> (Route, Option<Overlay>, bool) {
        (
            self.session.current(),
            self.overlays.borrow().active(),
            self.session.panes[pane::Side::Right.index()].open,
        )
    }

    pub(crate) fn flush_for_update(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.session_writable,
            "Session is not writable; update postponed"
        );
        self.session.save(&self.path)?;
        let drafts = serde_json::json!({
            "assistant":self.assistant.read(cx).update_drafts(cx)?,
            "tickets":self.tickets.read(cx).update_drafts(cx)?,
            "automations":self.automations.read(cx).update_drafts(cx)?,
        });
        let path = self.path.with_extension("update-drafts.json");
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&drafts)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }
    fn restore_update_drafts(&mut self, cx: &mut Context<Self>) {
        let path = self.path.with_extension("update-drafts.json");
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                self.assistant.update(cx, |view, cx| {
                    view.restore_update_drafts(&value["assistant"], cx)
                });
                self.tickets.update(cx, |view, cx| {
                    view.restore_update_drafts(&value["tickets"], cx)
                });
                self.automations.update(cx, |view, cx| {
                    view.restore_update_drafts(&value["automations"], cx)
                });
                let _ = std::fs::remove_file(path);
            } else {
                self.save_error = true;
            }
        }
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if !self.session_writable {
            return;
        }
        self.save_error = match self.session.save(&self.path) {
            Ok(()) => false,
            Err(error) => {
                eprintln!("Could not save shell session: {error}");
                true
            }
        };
        cx.notify();
    }
    fn focus_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = 0;
        self.input.update(cx, |input, _| input.reset());
        let initial_focus = self.input.focus_handle(cx);
        self.overlays
            .borrow_mut()
            .open(Overlay::Search, window, cx, Some(initial_focus));
    }
    fn cycle_focus(&self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        let handles = match self.overlays.borrow().active() {
            Some(Overlay::Search) => {
                let mut handles =
                    vec![self.input.focus_handle(cx), self.picker_close_focus.clone()];
                handles.extend(
                    self.picker_result_focus
                        .iter()
                        .take(Route::matching(&self.input.read(cx).content).len())
                        .cloned(),
                );
                handles
            }
            Some(Overlay::AddTicket | Overlay::AddAgent | Overlay::DeleteTicket(_)) => {
                self.tickets.read(cx).focus_handles(cx)
            }
            Some(Overlay::RenameConversation(_) | Overlay::DeleteConversation(_)) => {
                self.assistant.read(cx).focus_handles(cx)
            }
            _ => {
                if backwards {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                return;
            }
        };
        self.overlays
            .borrow()
            .cycle_focus(&handles, backwards, window, cx);
    }
    fn dispatch(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.session.current();
        if self
            .overlays
            .borrow()
            .active()
            .is_some_and(Overlay::is_dialog)
            && !matches!(control, Control::Dismiss)
        {
            return;
        }
        match control {
            Control::Back | Control::Forward => {
                self.session.go(matches!(control, Control::Forward));
                self.overlays.borrow_mut().dismiss(window, cx);
                window.focus(&self.focus, cx);
            }
            Control::Navigate(route) => {
                self.session.navigate(route);
                self.overlays.borrow_mut().dismiss(window, cx);
                window.focus(&self.focus, cx);
            }
            Control::Open(route) => {
                self.session.navigate(route);
                self.overlays.borrow_mut().dismiss(window, cx);
                window.focus(&self.focus, cx);
            }
            Control::Search => {
                self.palette_transition = Some(Instant::now());
                self.focus_picker(window, cx);
            }
            Control::Sidebar => self.toggle_pane(pane::Side::Left),
            Control::Evee => self.toggle_pane(pane::Side::Right),
            Control::Font(font) => self.session.font = font,
            Control::FontSize(size) => {
                self.session.font_size = size;
                set_type_scale(size.scale());
                self.assistant.update(cx, |_, cx| cx.notify());
                self.tickets.update(cx, |_, cx| cx.notify());
                self.automations.update(cx, |_, cx| cx.notify());
                self.input.update(cx, |_, cx| cx.notify());
                if let Some(updates) = cx.try_global::<crate::updates::Updates>().cloned() {
                    updates.0.update(cx, |_, cx| cx.notify());
                }
            }
            Control::Notifications => {
                let active = self.overlays.borrow().active();
                if active == Some(Overlay::Notifications) {
                    self.overlays.borrow_mut().dismiss(window, cx);
                } else {
                    self.overlays
                        .borrow_mut()
                        .open(Overlay::Notifications, window, cx, None);
                }
            }
            Control::MarkAllRead => {
                for item in &mut self.notification_items {
                    item.unread = false;
                }
            }
            Control::Dismiss => {
                self.overlays.borrow_mut().dismiss(window, cx);
            }
        }
        if before != self.session.current() {
            self.overlays.borrow_mut().dismiss(window, cx);
        }
        self.save(cx);
        window.refresh();
    }
    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        control: Control,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = id.into();
        let label: SharedString = label.into();
        let spoken: SharedString = label
            .split(" · ")
            .next()
            .unwrap_or(&label)
            .to_owned()
            .into();
        let button = action_button(
            ButtonSpec {
                id,
                label: spoken,
                kind: ButtonKind::Quiet,
                enabled: true,
            },
            |button| {
                button.gap(px(8.)).hover(move |s| {
                    if matches!(control, Control::Open(_)) {
                        s.text_color(rgb(TEXT))
                    } else {
                        s.bg(rgb(HOVER)).text_color(rgb(TEXT))
                    }
                })
            },
            move |this: &mut Self, window, cx| this.dispatch(control, window, cx),
            cx,
        );
        match control {
            Control::Sidebar | Control::Evee => button.role(accesskit::Role::Switch).aria_toggled(
                if if matches!(control, Control::Sidebar) {
                    self.session.panes[pane::Side::Left.index()].open
                } else {
                    self.session.panes[pane::Side::Right.index()].open
                } {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                },
            ),
            Control::Font(font) => button.role(accesskit::Role::RadioButton).aria_toggled(
                if self.session.font == font {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                },
            ),
            Control::FontSize(size) => button.role(accesskit::Role::RadioButton).aria_toggled(
                if self.session.font_size == size {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                },
            ),
            _ => button,
        }
    }
    fn icon_button(
        &self,
        id: &'static str,
        label: &'static str,
        name: &'static str,
        control: Control,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.button(id, label, control, cx)
            .debug_selector(move || id.into())
            .size(px(HEADER_CONTROL))
            .justify_center()
            .child(
                row()
                    .size(px(HEADER_ICON_SIZE))
                    .justify_center()
                    .debug_selector(move || format!("{id}.glyph"))
                    .child(icon(name, HEADER_ICON_SIZE)),
            )
    }
    fn command_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = Route::matching(&self.input.read(cx).content);
        panel()
            .id("search.dialog")
            .accessibility_id("search.dialog")
            .role(accesskit::Role::Dialog)
            .aria_label("Search spaces")
            .w(px(520.))
            .overflow_hidden()
            .child(
                row()
                    .h(px(56.))
                    .px(px(16.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(icon("search", 18.))
                    .child(self.input.clone())
                    .child(
                        self.button("palette-close", "Close search", Control::Dismiss, cx)
                            .track_focus(&self.picker_close_focus)
                            .child(shortcut_badge("esc").px(px(8.))),
                    ),
            )
            .child(
                column()
                    .px(px(8.))
                    .pb(px(8.))
                    .when(matches.is_empty(), |s| {
                        s.child(column().p(px(24.)).gap(px(6.)).child("No matches."))
                    })
                    .children(matches.iter().copied().enumerate().map(|(index, route)| {
                        self.button(
                            SharedString::from(format!(
                                "search.result.{}",
                                route.label().to_lowercase().replace(' ', "-")
                            )),
                            route.label(),
                            Control::Open(route),
                            cx,
                        )
                        .track_focus(&self.picker_result_focus[index])
                        .on_hover(cx.listener(move |this, hovered, _, cx| {
                            if *hovered {
                                this.selected = index;
                                cx.notify();
                            }
                        }))
                        .h(px(40.))
                        .px(px(10.))
                        .gap(px(12.))
                        .when(index == self.selected, |s| s.bg(rgb(HOVER)))
                        .child(icon(route.icon(), 17.))
                        .child(route.label())
                        .child(div().flex_1())
                        .child(
                            div()
                                .w(px(24.))
                                .when(index == self.selected, |s| s.child(shortcut_badge("↵"))),
                        )
                    })),
            )
            .child(
                row()
                    .h(px(40.))
                    .px(px(16.))
                    .gap(px(6.))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_size(type_size(CAPTION_SIZE))
                    .text_color(rgb(MUTED))
                    .child(shortcut_badge("↑ ↓"))
                    .child("Navigate")
                    .child(div().flex_1())
                    .child(shortcut_badge("↵"))
                    .child("Open"),
            )
    }
    fn notification_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        panel()
            .absolute()
            .top(px(56.))
            .right(px(16.))
            .w(px(350.))
            .overflow_hidden()
            .child(
                row()
                    .h(px(52.))
                    .px(px(16.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(div().font_weight(FontWeight::MEDIUM).child("Notifications"))
                    .child(div().flex_1())
                    .when(
                        self.notification_items.iter().any(|item| item.unread),
                        |s| {
                            s.child(
                                self.button(
                                    "mark-all-read",
                                    "Mark all read",
                                    Control::MarkAllRead,
                                    cx,
                                )
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(MUTED))
                                .child("Mark all read"),
                            )
                        },
                    )
                    .child(
                        self.button(
                            "dismiss-notifications",
                            "Close notifications",
                            Control::Dismiss,
                            cx,
                        )
                        .size(px(24.))
                        .justify_center()
                        .child(icon("close", 12.)),
                    ),
            )
            .when(self.notification_items.is_empty(), |s| {
                s.child(
                    column()
                        .py(px(24.))
                        .px(px(16.))
                        .items_center()
                        .gap(px(8.))
                        .child(icon("bell", 16.))
                        .child(
                            div()
                                .text_size(type_size(LABEL_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .child("No notifications yet"),
                        ),
                )
            })
            .children(
                self.notification_items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        row()
                            .gap(px(12.))
                            .px(px(16.))
                            .py(px(14.))
                            .when(index > 0, |s| s.border_t_1().border_color(rgb(BORDER)))
                            .child(icon(item.icon, 17.))
                            .child(
                                column()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(3.))
                                    .child(
                                        row()
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(item.title.clone()),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                div()
                                                    .text_size(type_size(CAPTION_SIZE))
                                                    .text_color(rgb(MUTED))
                                                    .child(item.relative_time.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(type_size(LABEL_SIZE))
                                            .text_color(rgb(MUTED))
                                            .child(item.body.clone()),
                                    ),
                            )
                            .when(item.unread, |s| {
                                s.child(div().size(px(5.)).rounded_full().bg(rgb(STATUS_UNREAD)))
                            })
                    }),
            )
    }
    fn keys(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlays.borrow().active() == Some(Overlay::Search) {
            let matches = Route::matching(&self.input.read(cx).content);
            match event.keystroke.key.as_str() {
                "down" => {
                    self.selected = (self.selected + 1).min(matches.len().saturating_sub(1));
                    cx.stop_propagation();
                }
                "up" => {
                    self.selected = self.selected.saturating_sub(1);
                    cx.stop_propagation();
                }
                "enter" => {
                    if let Some(route) = matches.get(self.selected) {
                        self.dispatch(Control::Open(*route), window, cx);
                    }
                    cx.stop_propagation();
                }
                _ => {}
            }
            cx.notify();
        }
    }
}
impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if ainc_client::update_required() {
            return column()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(20.))
                .bg(rgb(SHELL))
                .text_color(rgb(TEXT))
                .child(div().text_size(type_size(24.)).child("Update to continue"))
                .child(
                    div()
                        .id("required-update")
                        .role(accesskit::Role::Button)
                        .aria_label("Check for Updates")
                        .cursor_pointer()
                        .px(px(16.))
                        .py(px(10.))
                        .bg(rgb(HOVER_CONTROL))
                        .on_click(|_, _, cx| crate::updates::open(cx, true))
                        .child("Check for Updates"),
                )
                .into_any_element();
        }
        if reduced_motion() {
            for index in 0..2 {
                if let Some((_, _, to)) = self.grip_animation[index].take() {
                    self.grip_opacity[index] = to;
                }
            }
            for index in 0..2 {
                if let Some((_, _, to)) = self.pane_animation[index].take() {
                    self.pane_visible[index] = to;
                }
            }
            self.palette_transition = None;
        }
        for index in 0..2 {
            if let Some((start, from, to)) = self.grip_animation[index] {
                let t = (start.elapsed().as_secs_f32() / (HOVER_MS as f32 / 1000.)).min(1.);
                self.grip_opacity[index] = from + (to - from) * t;
                if t < 1. {
                    window.request_animation_frame();
                } else {
                    self.grip_animation[index] = None;
                }
            }
        }
        for index in 0..2 {
            if let Some((start, from, to)) = self.pane_animation[index] {
                let t = (start.elapsed().as_secs_f32() / (PANEL_MS as f32 / 1000.)).min(1.);
                self.pane_visible[index] = from + (to - from) * (1. - (1. - t).powi(3));
                if t < 1. {
                    window.request_animation_frame();
                } else {
                    self.pane_animation[index] = None;
                }
            }
        }
        let progress = |start: &mut Option<Instant>| {
            let Some(instant) = *start else {
                return 1.;
            };
            let t = (instant.elapsed().as_secs_f32() / (GRIP_MS as f32 / 1000.)).min(1.);
            if t < 1. {
                window.request_animation_frame();
            } else {
                *start = None;
            }
            1. - (1. - t).powi(3)
        };
        let palette_progress = progress(&mut self.palette_transition);
        if let Some(focus) = self.overlays.borrow_mut().take_pending_focus() {
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        let active_overlay = self.overlays.borrow().active();
        let content = match self.session.current() {
            Route::Automations => self.automations.clone().into_any_element(),
            Route::Tickets => self.tickets.clone().into_any_element(),
            Route::Agents => self.tickets.update(cx, |tickets, cx| tickets.agents(cx)),
            Route::Today
            | Route::Home
            | Route::Calendar
            | Route::Library
            | Route::Apps
            | Route::Assistant
            | Route::Settings => self
                .static_page(self.session.current(), cx)
                .into_any_element(),
        };
        let dialog_content = match active_overlay {
            Some(Overlay::Search) => Some(self.command_palette(cx).into_any_element()),
            Some(Overlay::AddTicket | Overlay::AddAgent | Overlay::DeleteTicket(_)) => {
                self.tickets.update(cx, |tickets, cx| tickets.overlay(cx))
            }
            Some(Overlay::RenameConversation(_) | Overlay::DeleteConversation(_)) => self
                .assistant
                .update(cx, |assistant, cx| assistant.overlay(cx)),
            _ => None,
        };
        column()
            .id("shell")
            .relative()
            .size_full()
            .bg(rgb(SHELL))
            .text_color(rgb(TEXT))
            .font_family(self.session.font.family())
            .text_size(type_size(BODY_SIZE))
            .line_height(relative(1.5))
            .track_focus(&self.focus)
            .key_context("Control")
            .on_click(cx.listener(|this, _, window, cx| {
                let active = this.overlays.borrow().active();
                if active
                    .is_some_and(|overlay| overlay.is_menu() || overlay == Overlay::Notifications)
                {
                    this.overlays.borrow_mut().dismiss(window, cx);
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(Self::keys))
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                this.command_held = event.modifiers.platform;
                cx.notify();
            }))
            .on_action(cx.listener(|this, action: &NavigateRoute, w, cx| {
                if let Some(page) = PAGES.iter().find(|page| page.shortcut == Some(action.0)) {
                    this.dispatch(Control::Navigate(page.route), w, cx);
                }
            }))
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                    if this.resizing.is_some() {
                        if event.dragging() {
                            this.resize_from_pointer(event.position, window);
                            cx.notify();
                        } else {
                            this.resizing = None;
                            this.save(cx);
                        }
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.resizing.take().is_some() {
                        this.save(cx);
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &GoBack, w, cx| this.dispatch(Control::Back, w, cx)))
            .on_action(
                cx.listener(|this, _: &GoForward, w, cx| this.dispatch(Control::Forward, w, cx)),
            )
            .on_action(cx.listener(|this, _: &Search, w, cx| this.dispatch(Control::Search, w, cx)))
            .on_action(
                cx.listener(|this, _: &ToggleSidebar, w, cx| {
                    this.dispatch(Control::Sidebar, w, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ToggleEvee, w, cx| this.dispatch(Control::Evee, w, cx)),
            )
            .on_action(cx.listener(|this, _: &Escape, w, cx| {
                if !this.overlays.borrow_mut().dismiss(w, cx) {
                    w.focus(&this.focus, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusNext, w, cx| this.cycle_focus(false, w, cx)))
            .on_action(cx.listener(|this, _: &FocusPrevious, w, cx| this.cycle_focus(true, w, cx)))
            .child(self.layout_body(
                self.sidebar(cx).into_any_element(),
                self.main_area(content).into_any_element(),
                self.evee(cx).into_any_element(),
                window,
                cx,
            ))
            // Header paints after panels so the current space covers the top border.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(self.header(cx)),
            )
            .when(
                cx.try_global::<crate::updates::Updates>()
                    .is_some_and(|updates| updates.0.read(cx).is_ready()),
                |view| {
                    view.child(
                        div()
                            .id("update-ready")
                            .accessibility_id("updates.ready")
                            .role(accesskit::Role::Button)
                            .aria_label("Update ready")
                            .absolute()
                            .bottom(px(16.))
                            .left(px(220.))
                            .px(px(16.))
                            .py(px(10.))
                            .rounded(px(8.))
                            .bg(rgb(HOVER_CONTROL))
                            .cursor_pointer()
                            .on_click(|_, _, cx| crate::updates::open(cx, false))
                            .child("Update ready · View update"),
                    )
                },
            )
            .when(self.save_error, |s| {
                s.child(
                    div()
                        .absolute()
                        .bottom(px(12.))
                        .left(px(190.))
                        .px(px(12.))
                        .py(px(8.))
                        .bg(rgb(SURFACE_ERROR))
                        .child("Session could not be saved. Changes remain in this window."),
                )
            })
            .when(active_overlay == Some(Overlay::Notifications), |s| {
                s.child(self.notification_panel(cx))
            })
            .when_some(dialog_content, |s, content| {
                s.child(
                    div()
                        .id("overlay-backdrop")
                        .occlude()
                        .absolute()
                        .inset_0()
                        .cursor_default()
                        .bg(rgba(SCRIM))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(active_overlay == Some(Overlay::Search), |s| {
                            s.items_start().pt(px(80.))
                        })
                        .on_mouse_move(|_, _, cx| cx.stop_propagation())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.overlays.borrow_mut().dismiss(window, cx);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .opacity(if active_overlay == Some(Overlay::Search) {
                                    0.65 + 0.35 * palette_progress
                                } else {
                                    1.
                                })
                                .child(content),
                        ),
                )
            })
            .into_any_element()
    }
}
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(
        (1..=9).map(|n| KeyBinding::new(&format!("cmd-{n}"), NavigateRoute(n), Some("Control"))),
    );
    cx.bind_keys([
        KeyBinding::new("cmd-alt-left", GoBack, Some("Control")),
        KeyBinding::new("cmd-alt-right", GoForward, Some("Control")),
        KeyBinding::new("cmd-k", Search, Some("Control")),
        KeyBinding::new("cmd-b", ToggleSidebar, Some("Control")),
        KeyBinding::new("cmd-shift-e", ToggleEvee, Some("Control")),
        KeyBinding::new("escape", Escape, Some("Control")),
        KeyBinding::new("tab", FocusNext, Some("Control")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("Control")),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
}

#[cfg(test)]
mod interaction_tests {
    use super::{Shell, bind_keys};
    use crate::{
        input,
        model::{PAGES, PANE_WIDTHS, Route, Session},
        overlay::Overlay,
    };
    use gpui::{
        Focusable, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point,
        TestAppContext, VisualTestContext, point, px,
    };

    fn double_click(cx: &mut VisualTestContext, position: Point<Pixels>) {
        for click_count in [1, 2] {
            cx.simulate_event(MouseDownEvent {
                position,
                button: MouseButton::Left,
                modifiers: Modifiers::default(),
                click_count,
                first_mouse: false,
            });
            cx.simulate_event(MouseUpEvent {
                position,
                button: MouseButton::Left,
                modifiers: Modifiers::default(),
                click_count,
            });
        }
    }

    #[gpui::test]
    fn custom_header_owns_titlebar_gestures(_cx: &mut TestAppContext) {
        let bounds = gpui::Bounds::new(point(px(0.), px(0.)), gpui::size(px(1360.), px(828.)));
        assert!(crate::main_window_options(bounds, "QA".into(), true).app_owns_titlebar_drag);
    }

    #[gpui::test]
    fn header_controls_do_not_zoom_but_empty_space_does(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        shell.update(cx, |shell, cx| {
            shell.session.navigate(Route::Tickets);
            shell.session.navigate(Route::Agents);
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let back = cx.debug_bounds("back").unwrap().center();
        double_click(cx, back);
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Today);
            assert_eq!(shell.titlebar_zoom_requests, 0);
        });
        let forward = cx.debug_bounds("forward").unwrap().center();
        double_click(cx, forward);
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Agents);
            assert_eq!(shell.titlebar_zoom_requests, 0);
        });
        for id in ["sidebar", "notifications", "shell.search"] {
            let position = cx.debug_bounds(id).unwrap().center();
            double_click(cx, position);
            shell.read_with(cx, |shell, _| assert_eq!(shell.titlebar_zoom_requests, 0));
        }
        let empty = cx.debug_bounds("titlebar-center-space").unwrap().center();
        double_click(cx, empty);
        shell.read_with(cx, |shell, _| assert_eq!(shell.titlebar_zoom_requests, 1));
    }

    #[gpui::test]
    fn sidebar_drag_persists_and_toggle_restores_its_width(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        cx.update(bind_keys);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::fixture(path.clone(), window, cx));
        let start = point(px(PANE_WIDTHS[0].2), px(200.));
        let end = point(px(260.), px(200.));
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.panes[0].width, 260.)
        });
        assert_eq!(Session::load(&path).panes[0].width, 260.);
        cx.simulate_keystrokes("cmd-b");
        shell.read_with(cx, |shell, _| assert!(!shell.session.panes[0].open));
        let focus = shell.read_with(cx, |shell, _| shell.focus.clone());
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("cmd-b");
        shell.read_with(cx, |shell, _| {
            assert!(shell.session.panes[0].open);
            assert_eq!(shell.session.panes[0].width, 260.);
        });
    }

    #[gpui::test]
    fn sidebar_badges_align_and_text_stays_inside_at_minimum_and_default_widths(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        let right =
            |bounds: gpui::Bounds<gpui::Pixels>| f32::from(bounds.origin.x + bounds.size.width);
        for width in [PANE_WIDTHS[0].0, PANE_WIDTHS[0].2] {
            cx.update(|window, cx| {
                shell.update(cx, |shell, cx| {
                    shell.session.panes[0].width = width;
                    shell.pane_visible[0] = width;
                    shell.command_held = true;
                    cx.notify();
                });
                window.draw(cx).clear(cx);
            });
            let sidebar = cx.debug_bounds("sidebar-content").unwrap();
            let title = cx.debug_bounds("workspace-title").unwrap();
            assert!(
                right(title) <= right(sidebar) - 6.,
                "title at width {width}"
            );
            let mut badge_right: Option<f32> = None;
            for index in 1..=9 {
                let badge = cx
                    .debug_bounds(match index {
                        1 => "sidebar-badge-1",
                        2 => "sidebar-badge-2",
                        3 => "sidebar-badge-3",
                        4 => "sidebar-badge-4",
                        5 => "sidebar-badge-5",
                        6 => "sidebar-badge-6",
                        7 => "sidebar-badge-7",
                        8 => "sidebar-badge-8",
                        _ => "sidebar-badge-9",
                    })
                    .unwrap();
                let label = cx
                    .debug_bounds(match index {
                        1 => "sidebar-label-1",
                        2 => "sidebar-label-2",
                        3 => "sidebar-label-3",
                        4 => "sidebar-label-4",
                        5 => "sidebar-label-5",
                        6 => "sidebar-label-6",
                        7 => "sidebar-label-7",
                        8 => "sidebar-label-8",
                        _ => "sidebar-label-9",
                    })
                    .unwrap();
                assert!(
                    right(label) + 8. <= f32::from(badge.origin.x),
                    "label {index} at width {width}"
                );
                assert!(
                    right(badge) <= right(sidebar) - 10.,
                    "badge {index} at width {width}"
                );
                if let Some(expected) = badge_right {
                    assert!(
                        (right(badge) - expected).abs() <= 1.,
                        "badge {index} at width {width}"
                    );
                } else {
                    badge_right = Some(right(badge));
                }
            }
        }
    }

    #[gpui::test]
    fn routes_and_history_use_shell_actions(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(bind_keys);
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        for page in PAGES.iter().filter(|page| page.shortcut.is_some()) {
            cx.simulate_keystrokes(&format!("cmd-{}", page.shortcut.unwrap()));
            shell.read_with(cx, |shell, _| {
                assert_eq!(shell.fixture_state().0, page.route)
            });
        }
        cx.simulate_keystrokes("cmd-alt-left");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Apps)
        });
        cx.simulate_keystrokes("cmd-alt-right");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Assistant)
        });
    }

    #[gpui::test]
    fn search_filters_selects_and_restores_focus(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            bind_keys(cx);
            input::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        cx.simulate_keystrokes("cmd-k");
        cx.simulate_input("settings");
        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.overlays.borrow().active(), Some(Overlay::Search));
            assert_eq!(
                Route::matching(&shell.input.read(cx).content),
                vec![Route::Settings]
            );
        });
        cx.simulate_keystrokes("enter");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Settings);
            assert_eq!(shell.overlays.borrow().active(), None);
        });
        cx.simulate_keystrokes("cmd-k");
        cx.simulate_input("no-such-space");
        cx.simulate_keystrokes("enter");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Settings)
        });
        cx.simulate_keystrokes("escape");
        let focus = shell.read_with(cx, |shell, _| shell.focus.clone());
        cx.update(|window, _| assert!(focus.is_focused(window)));
    }

    #[gpui::test]
    fn search_focus_wraps_and_escape_dismisses(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            bind_keys(cx);
            input::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        cx.simulate_keystrokes("cmd-k shift-tab");
        let last = shell.read_with(cx, |shell, _| {
            shell.picker_result_focus.last().unwrap().clone()
        });
        cx.update(|window, _| assert!(last.is_focused(window)));
        cx.simulate_keystrokes("tab");
        let input = shell.read_with(cx, |shell, cx| shell.input.focus_handle(cx));
        cx.update(|window, _| assert!(input.is_focused(window)));
        cx.simulate_keystrokes("escape");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.overlays.borrow().active(), None)
        });
    }
}
