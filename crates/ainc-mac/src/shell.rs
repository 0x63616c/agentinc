//! The window: title bar, sidebar, the current page in the content card, and
//! the overlays that float over them. Pages are held as [`PageHandle`]s and
//! iterated; the shell never names one.
#[cfg(test)]
#[path = "shell/fixture.rs"]
mod fixture;
#[path = "shell/header.rs"]
mod header;
#[path = "shell/menus.rs"]
mod menus;
#[path = "shell/palette.rs"]
mod palette;
#[path = "shell/pane.rs"]
mod pane;
#[path = "shell/sidebar.rs"]
mod sidebar;
#[cfg(test)]
#[path = "shell/tests.rs"]
mod tests;

use crate::{
    action::Run,
    daemon::Daemon,
    input::TextInput,
    overlay::Overlay,
    page::{Drafts, Page, PageHandle},
    routes::{Destination, PAGES, Route},
    terminal::{Shortcut, TerminalCallbacks},
    ui::*,
    ui_state::{Appearance, UiState},
};
use gpui::{prelude::*, *};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::Instant,
};
actions!(
    control,
    [
        Search,
        GoBack,
        GoForward,
        ToggleSidebar,
        Escape,
        Down,
        Up,
        Choose,
        FocusNext,
        FocusPrevious,
        OpenSettings,
        Quit
    ]
);
#[derive(Clone, PartialEq, Action)]
#[action(namespace = control, no_json)]
struct NavigateRoute(u8);

#[derive(Clone)]
pub(crate) enum Control {
    Go(Destination),
    Back,
    Forward,
    Search,
    Sidebar,
    Notifications,
    MarkAllRead,
    Dismiss,
    Appearance(Appearance),
    UserMenu,
    SupportMenu,
    CheckForUpdates,
    InstallUpdate,
    HelpCenter,
    SendFeedback,
    About,
    DismissToast(u64),
}
pub struct Shell {
    daemon: Arc<Daemon>,
    _sync_subscription: Subscription,
    ui_state: UiState,
    /// The type preferences, shared with the Settings page that edits them.
    appearance: Rc<Cell<Appearance>>,
    overlays: Rc<RefCell<OverlayHost<Overlay>>>,
    pages: Vec<PageHandle>,
    /// The route whose page was last told it is shown.
    shown: Option<Route>,
    /// A page asked for a destination; honoured at the next render, with the window.
    pending_destination: Option<Destination>,
    _settings_subscription: Subscription,
    _update_subscription: Option<Subscription>,
    profile: crate::profile::Profile,
    path: PathBuf,
    focus: FocusHandle,
    sidebar_focus: FocusHandle,
    input: Entity<TextInput>,
    picker_result_focus: Vec<FocusHandle>,
    picker_close_focus: FocusHandle,
    palette_scroll: ScrollHandle,
    _input_subscription: Subscription,
    _activation_subscription: Subscription,
    command_held: bool,
    palette_transition: Option<Instant>,
    launch_started: Option<Instant>,
    selected: usize,
    notification_items: Vec<Notification>,
    save_error: bool,
    save_toast: Option<u64>,
    state_writable: bool,
    resizing: bool,
    grip_opacity: f32,
    grip_animation: Option<(Instant, f32, f32)>,
    sidebar_visible: f32,
    sidebar_animation: Option<(Instant, f32, f32)>,
    hover: HoverFade,
    toasts: Toasts,
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
impl HoverHost for Shell {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}
/// A page emitted a destination; the shell takes it at the next render.
fn queue(shell: &mut Shell, to: Destination, cx: &mut Context<Shell>) {
    shell.pending_destination = Some(to);
    cx.notify();
}
impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = std::env::var_os("AINC_SESSION_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| ainc_release::identity::support_dir().join("session.json"));
        let daemon = Arc::new(crate::daemon::Daemon::connect());
        let clock = crate::sync::Timers(cx.background_executor().clone());
        cx.run(
            &crate::action::Pending::default(),
            || Ok(crate::profile::Profile::local()),
            |this, profile, cx| {
                let Ok(profile) = profile else { return };
                this.set_profile(profile, cx);
            },
        );
        if let Some(updates) = cx.try_global::<crate::updates::Updates>().cloned() {
            let shell = cx.weak_entity();
            updates.0.update(cx, |updates, _| {
                updates.on_before_install(move |cx| {
                    shell.update(cx, |shell, cx| shell.flush_for_update(cx))?
                })
            });
        }
        Self::with_state(
            path,
            daemon,
            clock,
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
        daemon: Arc<Daemon>,
        clock: impl crate::sync::Clock,
        profile: crate::profile::Profile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let loaded = UiState::load_checked(&path);
        let state_writable = loaded.is_ok();
        let ui_state = loaded.unwrap_or_default();
        set_type_scale(ui_state.font_size.scale());
        let appearance = Rc::new(Cell::new(ui_state.appearance()));
        let overlays = Rc::new(RefCell::new(OverlayHost::default()));
        let sync = cx.new(|cx| crate::sync::Sync::new(daemon.clone(), clock, cx));
        let sync_subscription = window.observe_window_visibility({
            let sync = sync.clone();
            move |visibility, _, cx| {
                sync.update(cx, |sync, cx| sync.set_paused(!visibility.is_visible(), cx))
            }
        });
        let shell = cx.weak_entity();
        let terminal = TerminalCallbacks {
            on_shortcut: Rc::new({
                let shell = shell.clone();
                move |shortcut, window, cx| {
                    let control = match shortcut {
                        Shortcut::Search => Control::Search,
                        Shortcut::Settings => Control::Go(Destination::Page(Route::Settings)),
                        Shortcut::Back => Control::Back,
                        Shortcut::Forward => Control::Forward,
                        Shortcut::Page(route) => Control::Go(Destination::Page(route)),
                    };
                    let _ = shell.update(cx, |shell, cx| shell.dispatch(control, window, cx));
                }
            }),
            on_command_held: Rc::new(move |held, cx| {
                let _ = shell.update(cx, |shell, cx| shell.set_command_held(held, cx));
            }),
        };
        let settings = cx.new(|cx| crate::settings::SettingsPage::new(appearance.clone(), cx));
        let settings_subscription = cx
            .subscribe(&settings, |this, _, appearance: &Appearance, cx| {
                this.apply_appearance(*appearance, cx)
            });
        let pages = vec![
            PageHandle::new(
                cx.new(|cx| {
                    crate::tickets::TicketsPage::new(
                        daemon.clone(),
                        sync.clone(),
                        overlays.clone(),
                        cx,
                    )
                }),
                cx,
                queue,
            ),
            PageHandle::new(
                cx.new(|cx| {
                    crate::evee::AssistantPage::new(
                        daemon.clone(),
                        sync.clone(),
                        overlays.clone(),
                        cx,
                    )
                }),
                cx,
                queue,
            ),
            PageHandle::new(
                cx.new(|cx| {
                    crate::agents::AgentsPage::new(
                        daemon.clone(),
                        sync.clone(),
                        overlays.clone(),
                        cx,
                    )
                }),
                cx,
                queue,
            ),
            PageHandle::new(
                cx.new(|cx| {
                    crate::automations::AutomationsPage::new(
                        daemon.clone(),
                        sync.clone(),
                        overlays.clone(),
                        cx,
                    )
                }),
                cx,
                queue,
            ),
            PageHandle::new(
                cx.new(|cx| {
                    crate::terminal::TerminalPage::new(
                        daemon.clone(),
                        overlays.clone(),
                        terminal,
                        cx,
                    )
                }),
                cx,
                queue,
            ),
            PageHandle::new(
                cx.new(|cx| crate::temporal::TemporalPage::new(daemon.clone(), cx)),
                cx,
                queue,
            ),
            PageHandle::new(settings, cx, queue),
            PageHandle::new(
                cx.new(|cx| crate::connections::ConnectionsPage::new(daemon.clone(), cx)),
                cx,
                queue,
            ),
            PageHandle::new(cx.new(crate::components::ComponentsPage::new), cx, queue),
        ];
        debug_assert!(
            PAGES
                .iter()
                .all(|spec| pages.iter().any(|page| page.route() == spec.route)),
            "every catalogued route has a page"
        );
        let input = cx.new(TextInput::new);
        let subscription = cx.observe(&input, |this, _, cx| {
            this.selected = 0;
            cx.notify();
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let sidebar_visible = if ui_state.sidebar.open {
            ui_state.sidebar.width
        } else {
            0.
        };
        let update_subscription = cx
            .try_global::<crate::updates::Updates>()
            .cloned()
            .map(|updates| cx.observe(&updates.0, |_, _, cx| cx.notify()));
        let mut shell = Self {
            daemon,
            _sync_subscription: sync_subscription,
            ui_state,
            appearance,
            overlays,
            pages,
            shown: None,
            pending_destination: None,
            _settings_subscription: settings_subscription,
            _update_subscription: update_subscription,
            profile: crate::profile::Profile {
                name: String::new(),
                photo: None,
            },
            sidebar_visible,
            sidebar_animation: None,
            hover: HoverFade::default(),
            toasts: Toasts::default(),
            #[cfg(test)]
            titlebar_zoom_requests: 0,
            path,
            focus,
            sidebar_focus: cx.focus_handle(),
            input,
            picker_result_focus: (0..64).map(|_| cx.focus_handle()).collect(),
            picker_close_focus: cx.focus_handle(),
            palette_scroll: ScrollHandle::new(),
            _input_subscription: subscription,
            _activation_subscription: cx.observe_window_activation(window, |this, window, cx| {
                // Command can be released in another app, with no modifier event
                // delivered here. Clear on deactivation only: activation is
                // queued on macOS and may arrive after fresh modifier input.
                // Never restore hints from the cached window modifier flags.
                if !window.is_window_active() {
                    this.set_command_held(false, cx);
                }
            }),
            command_held: false,
            palette_transition: None,
            launch_started: None,
            selected: 0,
            notification_items: Vec::new(),
            save_error: !state_writable,
            save_toast: None,
            state_writable,
            resizing: false,
            grip_opacity: 0.,
            grip_animation: None,
        };
        shell.set_profile(profile, cx);
        shell.restore_update_drafts(cx);
        shell
    }
    /// The page for a route; every catalogued route has one.
    fn page(&self, route: Route) -> &PageHandle {
        self.pages
            .iter()
            .find(|page| page.route() == route)
            .expect("every catalogued route has a page")
    }
    /// A page by type, for the few places that drive one directly.
    fn page_entity<P: Page>(&self) -> Entity<P> {
        self.page(P::ROUTE).entity::<P>()
    }
    /// The local person: shown in the sidebar and the Tickets owner principal.
    fn set_profile(&mut self, profile: crate::profile::Profile, cx: &mut Context<Self>) {
        self.page_entity::<crate::tickets::TicketsPage>()
            .update(cx, |tickets, cx| {
                tickets.set_owner(&profile.name, profile.photo.clone(), cx)
            });
        self.profile = profile;
        cx.notify();
    }

    pub(crate) fn flush_for_update(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.state_writable,
            "UI state is not writable; update postponed"
        );
        self.ui_state.save(&self.path)?;
        let mut drafts = serde_json::Map::new();
        for page in &self.pages {
            drafts.insert(page.route().key(), serde_json::to_value(page.drafts(cx)?)?);
        }
        let path = self.path.with_extension("update-drafts.json");
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&drafts)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }
    fn restore_update_drafts(&mut self, cx: &mut Context<Self>) {
        let path = self.path.with_extension("update-drafts.json");
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(mut saved) =
                serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes)
            {
                for page in &self.pages {
                    if let Some(drafts) = saved
                        .remove(&page.route().key())
                        .and_then(|value| serde_json::from_value::<Drafts>(value).ok())
                    {
                        page.restore(drafts, cx);
                    }
                }
                let _ = std::fs::remove_file(path);
            } else {
                self.save_error = true;
            }
        }
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if !self.state_writable {
            return;
        }
        let appearance = self.appearance.get();
        self.ui_state.font = appearance.font;
        self.ui_state.font_size = appearance.font_size;
        self.save_error = match self.ui_state.save(&self.path) {
            Ok(()) => false,
            Err(error) => {
                tracing::warn!(%error, "could not save UI state");
                true
            }
        };
        cx.notify();
    }
    /// Keeps one sticky toast in step with the save state.
    fn sync_save_toast(&mut self) {
        match (self.save_error, self.save_toast) {
            (true, None) => {
                self.save_toast = Some(self.toasts.push(
                    "Preferences could not be saved",
                    Some("Changes remain in this window.".into()),
                    Tone::Warning,
                ));
            }
            (false, Some(id)) => {
                self.toasts.dismiss(id);
                self.save_toast = None;
            }
            _ => {}
        }
    }
    /// A type preference changed, here or on the Settings page: scale, save, redraw.
    fn apply_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.appearance.set(appearance);
        set_type_scale(appearance.font_size.scale());
        for page in &self.pages {
            page.refresh(cx);
        }
        self.input.update(cx, |_, cx| cx.notify());
        if let Some(updates) = cx.try_global::<crate::updates::Updates>().cloned() {
            updates.0.update(cx, |_, cx| cx.notify());
        }
        self.save(cx);
    }
    fn focus_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = 0;
        self.input.update(cx, |input, _| input.reset());
        let initial_focus = self.input.focus_handle(cx);
        self.overlays
            .borrow_mut()
            .open(Overlay::Search, window, cx, Some(initial_focus));
    }
    fn cycle_focus(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        set_focus_visible(true);
        let handles = match self.overlays.borrow().active() {
            Some(Overlay::Search) => {
                let mut handles =
                    vec![self.input.focus_handle(cx), self.picker_close_focus.clone()];
                let count = self.palette_results(&self.input.read(cx).content).len();
                handles.extend(self.picker_result_focus.iter().take(count).cloned());
                self.overlays
                    .borrow()
                    .cycle_focus(&handles, backwards, window, cx);
                // The highlighted row follows keyboard focus so Enter and ↵ agree.
                if let Some(index) = self
                    .picker_result_focus
                    .iter()
                    .take(count)
                    .position(|handle| handle.is_focused(window))
                {
                    self.selected = index;
                    cx.notify();
                }
                return;
            }
            Some(Overlay::Dialog(route)) => self.page(route).focus_handles(cx),
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
    fn toggle_overlay(&mut self, overlay: Overlay, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.overlays.borrow().active();
        if active == Some(overlay) {
            self.overlays.borrow_mut().dismiss(window, cx);
        } else {
            self.overlays.borrow_mut().open(overlay, window, cx, None);
        }
    }
    /// Navigate to a destination's route and let the page that owns it select it.
    fn go(&mut self, to: Destination, window: &mut Window, cx: &mut Context<Self>) {
        self.overlays.borrow_mut().dismiss(window, cx);
        self.ui_state.navigate(to.route());
        window.focus(&self.focus, cx);
        for page in &self.pages {
            page.open(&to, window, cx);
        }
    }
    /// Close any page menu; returns whether one was open.
    fn dismiss_menus(&mut self, cx: &mut Context<Self>) -> bool {
        let mut closed = false;
        for page in &self.pages {
            closed |= page.dismiss_menus(cx);
        }
        closed
    }
    pub(crate) fn dispatch(
        &mut self,
        control: Control,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .overlays
            .borrow()
            .active()
            .is_some_and(Overlay::is_dialog)
            && !matches!(&control, Control::Dismiss | Control::DismissToast(_))
        {
            return;
        }
        match control {
            Control::Back | Control::Forward => {
                self.ui_state.go(matches!(&control, Control::Forward));
                self.overlays.borrow_mut().dismiss(window, cx);
                window.focus(&self.focus, cx);
            }
            Control::Go(to) => self.go(to, window, cx),
            Control::Search => {
                self.palette_transition = Some(Instant::now());
                self.focus_picker(window, cx);
            }
            Control::Sidebar => {
                self.overlays.borrow_mut().dismiss(window, cx);
                self.toggle_sidebar()
            }
            Control::Appearance(appearance) => {
                self.overlays.borrow_mut().dismiss(window, cx);
                self.apply_appearance(appearance, cx);
            }
            Control::Notifications => {
                if self.overlays.borrow().active() == Some(Overlay::Search) {
                    self.overlays.borrow_mut().dismiss(window, cx);
                }
                self.toggle_overlay(Overlay::Notifications, window, cx)
            }
            Control::UserMenu => {
                let active = self.overlays.borrow().active();
                if matches!(active, Some(Overlay::UserMenu { .. })) {
                    self.overlays.borrow_mut().dismiss(window, cx);
                } else {
                    self.overlays.borrow_mut().open(
                        Overlay::UserMenu { support: false },
                        window,
                        cx,
                        None,
                    );
                }
            }
            Control::SupportMenu => {
                let active = self.overlays.borrow().active();
                if let Some(Overlay::UserMenu { support }) = active {
                    self.overlays.borrow_mut().open(
                        Overlay::UserMenu { support: !support },
                        window,
                        cx,
                        None,
                    );
                }
            }
            Control::CheckForUpdates => {
                self.overlays.borrow_mut().dismiss(window, cx);
                if cx.has_global::<crate::updates::Updates>() {
                    crate::updates::open(cx, true);
                }
            }
            Control::InstallUpdate => {
                self.overlays.borrow_mut().dismiss(window, cx);
                if cx.has_global::<crate::updates::Updates>() {
                    crate::updates::open(cx, false);
                }
            }
            Control::HelpCenter => {
                self.overlays.borrow_mut().dismiss(window, cx);
                cx.open_url(&format!("{}#readme", ainc_release::REPOSITORY));
            }
            Control::SendFeedback => {
                self.overlays.borrow_mut().dismiss(window, cx);
                cx.open_url(&format!("{}/issues/new", ainc_release::REPOSITORY));
            }
            Control::About => {
                self.overlays.borrow_mut().dismiss(window, cx);
                crate::about::show();
            }
            Control::MarkAllRead => {
                for item in &mut self.notification_items {
                    item.unread = false;
                }
            }
            Control::DismissToast(id) => {
                self.toasts.dismiss(id);
                if self.save_toast == Some(id) {
                    self.save_toast = None;
                    self.save_error = false;
                }
            }
            Control::Dismiss => {
                self.overlays.borrow_mut().dismiss(window, cx);
            }
        }
        self.save(cx);
        window.refresh();
    }
    pub(crate) fn set_command_held(&mut self, held: bool, cx: &mut Context<Self>) {
        if self.command_held != held {
            self.command_held = held;
            cx.notify();
        }
    }

    fn keys(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlays.borrow().active() == Some(Overlay::Search) {
            let count = self.palette_results(&self.input.read(cx).content).len();
            match event.keystroke.key.as_str() {
                "down" => {
                    self.selected = (self.selected + 1).min(count.saturating_sub(1));
                    cx.stop_propagation();
                }
                "up" => {
                    self.selected = self.selected.saturating_sub(1);
                    cx.stop_propagation();
                }
                "enter" => {
                    self.choose_palette(self.selected, window, cx);
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
        if self.daemon.update_required() {
            return self.update_required(cx);
        }
        self.hover.animate(window);
        self.sync_save_toast();
        self.animate_sidebar(window);
        if reduced_motion() {
            self.palette_transition = None;
        }
        if let Some(to) = self.pending_destination.take() {
            self.go(to, window, cx);
            self.save(cx);
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
        } else if window.focused(cx).is_none() {
            // A clicked control that has since disappeared (a toast's close
            // button, a detail view's back button) must not take the keyboard
            // shortcuts with it: the shell is always a focus target of last resort.
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        let current = self.ui_state.current();
        if self.shown != Some(current) {
            if let Some(previous) = self.shown {
                self.page(previous).shown(false, cx);
            }
            self.page(current).shown(true, cx);
            self.shown = Some(current);
        }
        let active_overlay = self.overlays.borrow().active();
        let content = self.page(current).view().into_any_element();
        let dialog_content = match active_overlay {
            Some(Overlay::Search) => Some(self.command_palette(window, cx)),
            Some(Overlay::Dialog(route)) => self.page(route).overlay(window, cx),
            _ => None,
        };
        let update_ready = cx
            .try_global::<crate::updates::Updates>()
            .is_some_and(|updates| updates.0.read(cx).is_ready());
        column()
            .id("shell")
            .relative()
            .size_full()
            .bg(rgb(SHELL))
            .text_color(rgb(TEXT))
            .font_family(self.appearance.get().font.family())
            .text_size(type_size(BODY_SIZE))
            .line_height(relative(BODY_LINE_HEIGHT))
            .track_focus(&self.focus)
            .key_context("Control")
            .on_click(cx.listener(|this, _, window, cx| {
                let active = this.overlays.borrow().active();
                if active.is_some_and(Overlay::is_popover) {
                    this.overlays.borrow_mut().dismiss(window, cx);
                    cx.notify();
                }
                this.dismiss_menus(cx);
            }))
            .on_key_down(cx.listener(Self::keys))
            // Observe snapshots before a focused input consumes the event. These
            // also recover from a modifier release missed during navigation.
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.set_command_held(event.keystroke.modifiers.platform, cx);
            }))
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                this.set_command_held(event.keystroke.modifiers.platform, cx);
            }))
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                set_focus_visible(false);
                this.set_command_held(event.modifiers.platform, cx);
            }))
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                this.set_command_held(event.modifiers.platform, cx);
            }))
            .on_action(cx.listener(|this, action: &NavigateRoute, w, cx| {
                if let Some(route) = Route::from_shortcut(action.0) {
                    this.dispatch(Control::Go(Destination::Page(route)), w, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, w, cx| {
                this.dispatch(Control::Go(Destination::Page(Route::Settings)), w, cx);
            }))
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                    if this.resizing {
                        if event.dragging() {
                            this.resize_from_pointer(event.position, window);
                            cx.notify();
                        } else {
                            this.resizing = false;
                            this.save(cx);
                        }
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if std::mem::take(&mut this.resizing) {
                        this.save(cx);
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &GoBack, w, cx| this.dispatch(Control::Back, w, cx)))
            .on_action(
                cx.listener(|this, _: &GoForward, w, cx| this.dispatch(Control::Forward, w, cx)),
            )
            .on_action(cx.listener(|this, _: &Search, w, cx| {
                cx.stop_propagation();
                this.dispatch(Control::Search, w, cx);
            }))
            .on_action(
                cx.listener(|this, _: &ToggleSidebar, w, cx| {
                    this.dispatch(Control::Sidebar, w, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &Escape, w, cx| {
                let closed_menu = this.dismiss_menus(cx);
                if !this.overlays.borrow_mut().dismiss(w, cx) && !closed_menu {
                    w.focus(&this.focus, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusNext, w, cx| this.cycle_focus(false, w, cx)))
            .on_action(cx.listener(|this, _: &FocusPrevious, w, cx| this.cycle_focus(true, w, cx)))
            .child(self.body(
                self.sidebar(cx).into_any_element(),
                self.main_area(content).into_any_element(),
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
            .when(update_ready, |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom(px(PANEL_GAP + STATUS_BAR_HEIGHT + SPACE_3))
                        .left(px(self.sidebar_visible + PANEL_GAP + SPACE_3))
                        .child(
                            Button::new("update-ready", "Update ready · Install")
                                .primary()
                                .small()
                                .icon("download")
                                .build(&self.hover, |_, _, cx| crate::updates::open(cx, false), cx)
                                .accessibility_id("updates.ready")
                                .shadow(shadow_toast()),
                        ),
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
                            s.items_start().pt(px(PALETTE_TOP))
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
                                .when(active_overlay == Some(Overlay::Search), |s| {
                                    s.opacity(0.65 + 0.35 * palette_progress)
                                        .mt(px(-6. * (1. - palette_progress)))
                                })
                                .child(content),
                        ),
                )
            })
            .when(!self.toasts.is_empty(), |s| {
                s.child(self.toasts.render(
                    &self.hover,
                    PANEL_GAP + PAGE_X,
                    PANEL_GAP + STATUS_BAR_HEIGHT + PAGE_X,
                    |this: &mut Self, id, window, cx| {
                        this.dispatch(Control::DismissToast(id), window, cx)
                    },
                    cx,
                ))
            })
            .when_some(
                launch_overlay(
                    *self.launch_started.get_or_insert_with(Instant::now),
                    window,
                ),
                |s, overlay| s.child(overlay),
            )
            .into_any_element()
    }
}
pub fn bind_keys(cx: &mut App) {
    cx.on_action(|_: &Search, cx| {
        let handle = cx.active_window().or_else(|| {
            let windows = cx.windows();
            (windows.len() == 1).then(|| windows[0])
        });
        if let Some(handle) = handle {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    if let Some(shell) = window.root::<Shell>().flatten() {
                        shell.update(cx, |shell, cx| shell.dispatch(Control::Search, window, cx));
                    }
                });
            });
        }
    });
    cx.bind_keys((0..=9).map(|n| {
        KeyBinding::new(
            &shortcuts::route(n as usize).0,
            NavigateRoute(n),
            Some("Control"),
        )
    }));
    cx.bind_keys([
        KeyBinding::new(shortcuts::BACK.keystroke, GoBack, Some("Control")),
        KeyBinding::new(shortcuts::FORWARD.keystroke, GoForward, Some("Control")),
        KeyBinding::new(shortcuts::SEARCH.keystroke, Search, None),
        KeyBinding::new(shortcuts::SETTINGS.keystroke, OpenSettings, Some("Control")),
        KeyBinding::new(
            shortcuts::TOGGLE_SIDEBAR.keystroke,
            ToggleSidebar,
            Some("Control"),
        ),
        KeyBinding::new(shortcuts::DISMISS.keystroke, Escape, Some("Control")),
        KeyBinding::new("tab", FocusNext, Some("Control")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("Control")),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
}
