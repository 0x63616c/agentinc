#[path = "shell/header.rs"]
mod header;
#[path = "shell/layout.rs"]
mod layout;
#[path = "shell/main_content.rs"]
mod main_content;
#[path = "shell/pane.rs"]
mod pane;
#[path = "shell/sidebar.rs"]
mod sidebar;

use crate::storage::{Store, WorkspaceCommand};
use crate::{
    input::TextInput,
    model::{FontChoice, FontSize, PAGES, Route, Session},
    ui::*,
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
    Open(Route),
    Navigate(Route),
    Back,
    Forward,
    Search,
    WorkspacePicker,
    SwitchWorkspace(String),
    NewWorkspace,
    CreateWorkspace,
    Sidebar,
    Notifications,
    MarkAllRead,
    Dismiss,
    Font(FontChoice),
    FontSize(FontSize),
}
pub struct Shell {
    store: Option<std::sync::Arc<Store>>,
    session: Session,
    overlays: Rc<RefCell<OverlayHost>>,
    assistant: Entity<crate::evee::AssistantPage>,
    tickets: Entity<crate::tickets::TicketsPage>,
    automations: Entity<crate::automations::AutomationsPage>,
    temporal: Entity<crate::temporal::TemporalPage>,
    _automation_subscriptions: Vec<Subscription>,
    _temporal_subscription: Subscription,
    _tickets_subscription: Subscription,
    _update_subscription: Option<Subscription>,
    _assistant_subscriptions: Vec<Subscription>,
    profile: crate::profile::Profile,
    path: PathBuf,
    focus: FocusHandle,
    pane_focus: [FocusHandle; 1],
    input: Entity<TextInput>,
    workspace_name: Entity<TextInput>,
    workspace_icon: Entity<TextInput>,
    workspace_color: Entity<TextInput>,
    workspace_only: bool,
    creating_workspace: bool,
    workspace_pending: bool,
    workspace_error: Option<String>,
    picker_result_focus: Vec<FocusHandle>,
    picker_close_focus: FocusHandle,
    _input_subscription: Subscription,
    command_held: bool,
    palette_transition: Option<Instant>,
    launch_started: Option<Instant>,
    selected: usize,
    notification_items: Vec<Notification>,
    save_error: bool,
    session_writable: bool,
    resizing: Option<pane::Side>,
    grip_opacity: [f32; 1],
    grip_animation: [Option<(Instant, f32, f32)>; 1],
    pane_visible: [f32; 1],
    pane_animation: [Option<(Instant, f32, f32)>; 1],
    assistant_focus_pending: bool,
    shell_focus_pending: bool,
    #[cfg(target_os = "macos")]
    terminal: Option<Rc<RefCell<crate::terminal::TerminalHost>>>,
    #[cfg(target_os = "macos")]
    terminal_error: Option<String>,
    #[cfg(target_os = "macos")]
    pending_terminal_focus: bool,
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
#[derive(Clone)]
enum PaletteItem {
    Route(Route),
    Workspace {
        id: String,
        name: String,
        icon: Option<String>,
    },
    CreateWorkspace,
}
impl Shell {
    #[cfg(target_os = "macos")]
    fn reset_terminal_for_workspace(&mut self) {
        // Recreate the view with this workspace's layout while its daemon PTY keeps running.
        self.terminal.take();
        self.terminal_error = None;
    }

    #[cfg(not(target_os = "macos"))]
    fn reset_terminal_for_workspace(&mut self) {}

    fn workspace_state(&self) -> crate::storage::WorkspaceState {
        self.store
            .as_ref()
            .map(|store| store.workspaces())
            .unwrap_or_else(|| Store::new().workspaces())
    }
    fn palette_items(&self, query: &str) -> Vec<PaletteItem> {
        let query = query.trim().to_lowercase();
        let mut items = if self.workspace_only {
            Vec::new()
        } else {
            Route::matching(&query)
                .into_iter()
                .map(PaletteItem::Route)
                .collect()
        };
        let show_all = query.is_empty() || query == "workspace" || query == "switch workspace";
        for workspace in self.workspace_state().workspaces {
            if show_all || workspace.name.to_lowercase().contains(&query) {
                items.push(PaletteItem::Workspace {
                    id: workspace.id,
                    name: workspace.name,
                    icon: workspace.icon,
                });
            }
        }
        if show_all || "create workspace".contains(&query) || "new workspace".contains(&query) {
            items.push(PaletteItem::CreateWorkspace);
        }
        items
    }
    fn choose_palette(&mut self, item: PaletteItem, window: &mut Window, cx: &mut Context<Self>) {
        match item {
            PaletteItem::Route(route) => self.dispatch(Control::Open(route), window, cx),
            PaletteItem::Workspace { id, .. } => {
                self.dispatch(Control::SwitchWorkspace(id), window, cx)
            }
            PaletteItem::CreateWorkspace => self.dispatch(Control::NewWorkspace, window, cx),
        }
    }
    fn change_workspace(
        &mut self,
        command: WorkspaceCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace_pending {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        self.workspace_pending = true;
        self.workspace_error = None;
        self.overlays.borrow_mut().dismiss(window, cx);
        let request = cx
            .background_executor()
            .spawn(async move { store.workspace_command(command) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.workspace_pending = false;
                match result {
                    Ok(_) => {
                        this.reset_terminal_for_workspace();
                        this.assistant.update(cx, |page, cx| {
                            page.workspace_changed(cx);
                            cx.notify();
                        });
                        this.tickets.update(cx, |page, cx| {
                            page.workspace_changed(cx);
                            cx.notify();
                        });
                        this.automations.update(cx, |page, cx| {
                            page.workspace_changed(cx);
                            cx.notify();
                        });
                    }
                    Err(error) => {
                        this.workspace_error = Some(format!("Workspace unavailable: {error}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
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
                            this.session.navigate(Route::Assistant);
                            this.assistant_focus_pending = true;
                        }
                        crate::evee::Navigation::List => this.shell_focus_pending = true,
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
            cx.new(|cx| crate::automations::AutomationsPage::new(store.clone(), storage_error, cx));
        let temporal = cx.new(crate::temporal::TemporalPage::new);
        let temporal_subscription = cx.observe(&temporal, |_, _, cx| cx.notify());
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
        let workspace_name = cx.new(|cx| {
            TextInput::new(cx)
                .identified("workspace.name")
                .with_placeholder("Workspace name")
        });
        let workspace_icon = cx
            .new(|cx| TextInput::field("Icon (optional)", false, cx).identified("workspace.icon"));
        let workspace_color = cx.new(|cx| {
            TextInput::field("Color #RRGGBB (optional)", false, cx).identified("workspace.color")
        });
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
            store,
            session,
            overlays,
            assistant,
            tickets,
            automations,
            temporal,
            _automation_subscriptions: automation_subscriptions,
            _temporal_subscription: temporal_subscription,
            _tickets_subscription: tickets_subscription,
            _update_subscription: update_subscription,
            _assistant_subscriptions: assistant_subscriptions,
            profile,
            pane_visible,
            pane_animation: [None; 1],
            assistant_focus_pending: false,
            shell_focus_pending: false,
            #[cfg(target_os = "macos")]
            terminal: None,
            #[cfg(target_os = "macos")]
            terminal_error: None,
            #[cfg(target_os = "macos")]
            pending_terminal_focus: false,
            #[cfg(test)]
            titlebar_zoom_requests: 0,
            path,
            focus,
            pane_focus: [cx.focus_handle()],
            input,
            workspace_name,
            workspace_icon,
            workspace_color,
            workspace_only: false,
            creating_workspace: false,
            workspace_pending: false,
            workspace_error: None,
            picker_result_focus: (0..64).map(|_| cx.focus_handle()).collect(),
            picker_close_focus: cx.focus_handle(),
            _input_subscription: subscription,
            command_held: false,
            palette_transition: None,
            launch_started: None,
            selected: 0,
            notification_items: Vec::new(),
            save_error: !session_writable,
            session_writable,
            resizing: None,
            grip_opacity: [0.; 1],
            grip_animation: [None; 1],
        };
        shell.restore_update_drafts(cx);
        shell
    }
    #[cfg(test)]
    pub(crate) fn fixture(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = crate::storage::Store::open(&path.with_extension("sqlite3")).unwrap();
        let mut shell = Self::with_state(
            path,
            Some(std::sync::Arc::new(store)),
            None,
            crate::profile::Profile {
                name: "QA Profile".into(),
                photo: None,
            },
            window,
            cx,
        );
        shell.launch_started = Some(Instant::now() - std::time::Duration::from_secs(1));
        shell
    }

    #[cfg(test)]
    pub(crate) fn fixture_state(&self) -> (Route, Option<Overlay>, bool) {
        (
            self.session.current(),
            self.overlays.borrow().active(),
            false,
        )
    }
    #[cfg(all(test, feature = "rendered-tests"))]
    pub(crate) fn fixture_launch_elapsed(
        &mut self,
        elapsed: std::time::Duration,
        cx: &mut Context<Self>,
    ) {
        self.launch_started = Some(Instant::now() - elapsed);
        cx.notify();
    }
    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_temporal(
        &mut self,
        page: ainc_client::types::ExecutionPage,
        cx: &mut Context<Self>,
    ) {
        self.temporal.update(cx, |view, cx| view.fixture(page, cx));
    }
    #[cfg(all(test, feature = "rendered-tests"))]
    pub(crate) fn fixture_temporal_error(&mut self, cx: &mut Context<Self>) {
        self.temporal.update(cx, |view, cx| view.fixture_error(cx));
    }
    #[cfg(all(test, feature = "rendered-tests"))]
    pub(crate) fn fixture_temporal_loading(&mut self, cx: &mut Context<Self>) {
        self.temporal
            .update(cx, |view, cx| view.fixture_loading(cx));
    }
    #[cfg(all(test, feature = "rendered-tests"))]
    pub(crate) fn fixture_profile_name(&mut self, name: &str, cx: &mut Context<Self>) {
        self.profile.name = name.into();
        cx.notify();
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn fixture_chat(&mut self, populated: bool, cx: &mut Context<Self>) {
        self.session.navigate(Route::Assistant);
        self.assistant
            .update(cx, |assistant, cx| assistant.fixture_chat(populated, cx));
        cx.notify();
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn fixture_models(&mut self, cx: &mut Context<Self>) {
        self.assistant
            .update(cx, |assistant, cx| assistant.fixture_models(cx));
        cx.notify();
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
        self.creating_workspace = false;
        self.workspace_error = None;
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
                if self.creating_workspace {
                    handles[0] = self.workspace_name.read(cx).focus_handle(cx);
                    handles.push(self.workspace_icon.read(cx).focus_handle(cx));
                    handles.push(self.workspace_color.read(cx).focus_handle(cx));
                    return self
                        .overlays
                        .borrow()
                        .cycle_focus(&handles, backwards, window, cx);
                }
                handles.extend(
                    self.picker_result_focus
                        .iter()
                        .take(self.palette_items(&self.input.read(cx).content).len())
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
    pub(crate) fn dispatch(
        &mut self,
        control: Control,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.session.current();
        if self
            .overlays
            .borrow()
            .active()
            .is_some_and(Overlay::is_dialog)
            && !matches!(&control, Control::Dismiss)
        {
            return;
        }
        match control {
            Control::Back | Control::Forward => {
                self.session.go(matches!(&control, Control::Forward));
                self.overlays.borrow_mut().dismiss(window, cx);
                window.focus(&self.focus, cx);
            }
            Control::Navigate(route) => {
                if route == Route::Assistant {
                    self.assistant
                        .update(cx, |assistant, cx| assistant.show_list(cx));
                }
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
                self.workspace_only = false;
                self.palette_transition = Some(Instant::now());
                self.focus_picker(window, cx);
            }
            Control::WorkspacePicker => {
                self.workspace_only = true;
                self.palette_transition = Some(Instant::now());
                self.focus_picker(window, cx);
            }
            Control::SwitchWorkspace(id) => {
                self.change_workspace(WorkspaceCommand::Switch { id }, window, cx)
            }
            Control::NewWorkspace => {
                self.creating_workspace = true;
                self.workspace_name.update(cx, |input, _| input.reset());
                self.workspace_icon.update(cx, |input, _| input.reset());
                self.workspace_color.update(cx, |input, _| input.reset());
                self.selected = 0;
                window.focus(&self.workspace_name.read(cx).focus_handle(cx), cx);
            }
            Control::CreateWorkspace => {
                let name = self.workspace_name.read(cx).content.trim().to_owned();
                let icon = self.workspace_icon.read(cx).content.trim().to_owned();
                let color = self.workspace_color.read(cx).content.trim().to_owned();
                if name.is_empty()
                    || name.chars().count() > 120
                    || icon.chars().count() > 4
                    || (!color.is_empty()
                        && (color.len() != 7
                            || !color.starts_with('#')
                            || !color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())))
                {
                    self.workspace_error = Some(
                        "Enter a name, up to four icon characters, and an optional #RRGGBB color."
                            .into(),
                    );
                } else {
                    self.change_workspace(
                        WorkspaceCommand::Create {
                            name,
                            icon: (!icon.is_empty()).then_some(icon),
                            color: (!color.is_empty()).then_some(color),
                        },
                        window,
                        cx,
                    );
                }
            }
            Control::Sidebar => self.toggle_pane(pane::Side::Left),
            Control::Font(font) => self.session.font = font,
            Control::FontSize(size) => {
                self.session.font_size = size;
                set_type_scale(size.scale());
                self.assistant.update(cx, |_, cx| cx.notify());
                self.tickets.update(cx, |_, cx| cx.notify());
                self.automations.update(cx, |_, cx| cx.notify());
                self.temporal.update(cx, |_, cx| cx.notify());
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
        #[cfg(target_os = "macos")]
        if before != self.session.current() {
            if self.session.current() == Route::Terminal {
                self.pending_terminal_focus = true;
            } else if before == Route::Terminal
                && let Some(terminal) = &self.terminal
            {
                terminal.borrow_mut().hide();
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
        let hover_open = matches!(&control, Control::Open(_));
        let click_control = control.clone();
        let button = action_button(
            ButtonSpec {
                id,
                label: spoken,
                kind: ButtonKind::Quiet,
                enabled: true,
            },
            |button| {
                button.gap(px(8.)).hover(move |s| {
                    if hover_open {
                        s.text_color(rgb(TEXT))
                    } else {
                        s.bg(rgb(HOVER)).text_color(rgb(TEXT))
                    }
                })
            },
            move |this: &mut Self, window, cx| this.dispatch(click_control.clone(), window, cx),
            cx,
        );
        match &control {
            Control::Sidebar => button.role(accesskit::Role::Switch).aria_toggled(
                if self.session.panes[pane::Side::Left.index()].open {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                },
            ),
            Control::Font(font) => button.role(accesskit::Role::RadioButton).aria_toggled(
                if self.session.font == *font {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                },
            ),
            Control::FontSize(size) => button.role(accesskit::Role::RadioButton).aria_toggled(
                if self.session.font_size == *size {
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
        icon_control(
            self.button(id, label, control, cx)
                .debug_selector(move || id.into()),
        )
        .child(
            row()
                .size(px(HEADER_ICON_SIZE))
                .justify_center()
                .debug_selector(move || format!("{id}.glyph"))
                .child(icon(name, HEADER_ICON_SIZE)),
        )
    }
    fn command_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = self.palette_items(&self.input.read(cx).content);
        let current_id = self.workspace_state().current_id;
        panel()
            .id("search.dialog")
            .accessibility_id("search.dialog")
            .role(accesskit::Role::Dialog)
            .aria_label(if self.creating_workspace { "Create workspace" } else { "Search spaces and workspaces" })
            .w(px(520.))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                row()
                    .h(px(56.))
                    .px(px(16.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(icon(if self.creating_workspace { "plus" } else { "search" }, 18.))
                    .child(if self.creating_workspace { self.workspace_name.clone() } else { self.input.clone() })
                    .child(
                        self.button("palette-close", "Close search", Control::Dismiss, cx)
                            .track_focus(&self.picker_close_focus)
                            .child(shortcut_badge("esc").px(px(8.))),
                    ),
            )
            .when(self.creating_workspace, |panel| panel.child(
                column()
                    .p(px(16.))
                    .gap(px(12.))
                    .child(div().font_weight(FontWeight::MEDIUM).child("New workspace"))
                    .child(self.workspace_icon.clone())
                    .child(self.workspace_color.clone())
                    .when_some(self.workspace_error.as_ref(), |form, error| form.child(div().text_color(rgb(ERROR)).child(error.clone())))
                    .child(self.button("create-workspace", "Create workspace", Control::CreateWorkspace, cx)
                        .h(px(36.)).px(px(12.)).bg(rgb(HOVER)).child("Create workspace")),
            ))
            .when(!self.creating_workspace, |panel| panel.child(
                column()
                    .id("workspace-results")
                    .p(px(8.))
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .when(matches.is_empty(), |s| {
                        s.child(column().p(px(24.)).gap(px(6.)).child("No matches."))
                    })
                    .children(matches.iter().cloned().enumerate().map(|(index, item)| {
                        let (label, control) = match &item {
                            PaletteItem::Route(route) => (route.label().to_owned(), Control::Open(*route)),
                            PaletteItem::Workspace { id, name, .. } => (format!("Switch to {name}"), Control::SwitchWorkspace(id.clone())),
                            PaletteItem::CreateWorkspace => ("Create workspace".into(), Control::NewWorkspace),
                        };
                        let selected_workspace = matches!(&item, PaletteItem::Workspace { id, .. } if *id == current_id);
                        self.button(
                            SharedString::from(format!("search.result.{index}")),
                            "Open result",
                            control,
                            cx,
                        )
                        .when_some(self.picker_result_focus.get(index), |row, focus| row.track_focus(focus))
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
                        .child(match item {
                            PaletteItem::Route(route) => icon(route.icon(), 17.).into_any_element(),
                            PaletteItem::Workspace { icon, name, .. } => div().w(px(17.)).child(icon.unwrap_or_else(|| name.chars().next().unwrap_or('W').to_string())).into_any_element(),
                            PaletteItem::CreateWorkspace => icon("plus", 17.).into_any_element(),
                        })
                        .child(label)
                        .child(div().flex_1())
                        .when(selected_workspace, |row| row.child(div().text_color(rgb(MUTED)).child("Current")))
                        .child(
                            div()
                                .w(px(24.))
                                .when(index == self.selected, |s| s.child(shortcut_badge("↵"))),
                        )
                    })),
            ))
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
                        list_item(("notification", index), item.title.clone())
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
            let matches = self.palette_items(&self.input.read(cx).content);
            match event.keystroke.key.as_str() {
                "down" => {
                    if !self.creating_workspace {
                        self.selected = (self.selected + 1).min(matches.len().saturating_sub(1));
                    }
                    cx.stop_propagation();
                }
                "up" => {
                    if !self.creating_workspace {
                        self.selected = self.selected.saturating_sub(1);
                    }
                    cx.stop_propagation();
                }
                "enter" => {
                    if self.creating_workspace {
                        self.dispatch(Control::CreateWorkspace, window, cx);
                    } else if let Some(item) = matches.get(self.selected) {
                        self.choose_palette(item.clone(), window, cx);
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
                .child(action_button(
                    ButtonSpec {
                        id: "required-update".into(),
                        label: "Check for Updates".into(),
                        kind: ButtonKind::Secondary,
                        enabled: true,
                    },
                    |button| {
                        button
                            .px(px(16.))
                            .py(px(10.))
                            .bg(rgb(HOVER_CONTROL))
                            .child("Check for Updates")
                    },
                    |_, _, cx| crate::updates::open(cx, true),
                    cx,
                ))
                .into_any_element();
        }
        if reduced_motion() {
            for index in 0..self.pane_visible.len() {
                if let Some((_, _, to)) = self.grip_animation[index].take() {
                    self.grip_opacity[index] = to;
                }
            }
            for index in 0..self.pane_visible.len() {
                if let Some((_, _, to)) = self.pane_animation[index].take() {
                    self.pane_visible[index] = to;
                }
            }
            self.palette_transition = None;
        }
        for index in 0..self.pane_visible.len() {
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
        for index in 0..self.pane_visible.len() {
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
        if self.assistant_focus_pending {
            self.assistant_focus_pending = false;
            let focus = self.assistant.read(cx).composer_focus(cx);
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        if self.shell_focus_pending {
            self.shell_focus_pending = false;
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        // AppKit child views paint above GPUI's Metal layer. Hide Ghostty
        // before GPUI paints any app popover, menu, or dialog over this page.
        #[cfg(target_os = "macos")]
        if self.session.current() == Route::Terminal
            && active_overlay.is_some()
            && let Some(terminal) = &self.terminal
        {
            terminal.borrow_mut().hide();
        }
        #[cfg(target_os = "macos")]
        if self.session.current() == Route::Terminal
            && self.terminal.is_none()
            && self.terminal_error.is_none()
        {
            let workspace_id = self.workspace_state().current_id;
            match crate::terminal::TerminalHost::new(
                window,
                cx.to_async(),
                cx.weak_entity(),
                &workspace_id,
            ) {
                Ok(host) => {
                    self.terminal = Some(Rc::new(RefCell::new(host)));
                    self.pending_terminal_focus = true;
                }
                Err(error) => {
                    self.terminal_error = Some(format!("Ghostty could not start: {error:#}"))
                }
            }
        }
        let content = match self.session.current() {
            Route::Automations => self.automations.clone().into_any_element(),
            Route::Temporal => {
                self.temporal.update(cx, |view, cx| view.ensure_loaded(cx));
                self.temporal.clone().into_any_element()
            }
            Route::Tickets => self.tickets.clone().into_any_element(),
            Route::Agents => self
                .tickets
                .update(cx, |tickets, cx| tickets.agents(window, cx)),
            Route::Assistant => self.assistant.clone().into_any_element(),
            Route::Terminal => self
                .terminal_page(active_overlay.is_none())
                .into_any_element(),
            Route::Settings => self
                .static_page(self.session.current(), cx)
                .into_any_element(),
        };
        #[cfg(target_os = "macos")]
        {
            self.pending_terminal_focus = false;
        }
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
                if let Some(route) = Route::from_shortcut(action.0) {
                    this.dispatch(Control::Navigate(route), w, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, w, cx| {
                this.dispatch(Control::Navigate(Route::Settings), w, cx);
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
                None,
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
                    view.child(action_button(
                        ButtonSpec {
                            id: "update-ready".into(),
                            label: "Update ready".into(),
                            kind: ButtonKind::Secondary,
                            enabled: true,
                        },
                        |button| {
                            button
                                .accessibility_id("updates.ready")
                                .absolute()
                                .bottom(px(16.))
                                .left(px(220.))
                                .px(px(16.))
                                .py(px(10.))
                                .rounded(px(8.))
                                .bg(rgb(HOVER_CONTROL))
                                .child("Update ready · View update")
                        },
                        |_, _, cx| crate::updates::open(cx, false),
                        cx,
                    ))
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
            .when_some(
                self.workspace_error
                    .as_ref()
                    .filter(|_| active_overlay != Some(Overlay::Search)),
                |s, error| {
                    s.child(
                        div()
                            .absolute()
                            .bottom(px(12.))
                            .left(px(220.))
                            .px(px(12.))
                            .py(px(8.))
                            .bg(rgb(SURFACE_ERROR))
                            .text_color(rgb(ERROR))
                            .child(error.clone()),
                    )
                },
            )
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
    cx.bind_keys(
        (0..=9).map(|n| KeyBinding::new(&format!("cmd-{n}"), NavigateRoute(n), Some("Control"))),
    );
    cx.bind_keys([
        KeyBinding::new("cmd-alt-left", GoBack, Some("Control")),
        KeyBinding::new("cmd-alt-right", GoForward, Some("Control")),
        KeyBinding::new("cmd-k", Search, None),
        KeyBinding::new("cmd-,", OpenSettings, Some("Control")),
        KeyBinding::new("cmd-b", ToggleSidebar, Some("Control")),
        KeyBinding::new("escape", Escape, Some("Control")),
        KeyBinding::new("tab", FocusNext, Some("Control")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("Control")),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
}

#[cfg(test)]
mod interaction_tests {
    use super::{Shell, WorkspaceCommand, bind_keys};
    use crate::{
        input,
        model::{PANE_WIDTHS, Route, Session},
        ui::Overlay,
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
            assert_eq!(shell.session.current(), Route::Assistant);
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
            let search = cx.debug_bounds("shell.search").unwrap();
            assert!(
                f32::from(search.origin.x) >= f32::from(sidebar.origin.x),
                "search left edge at width {width}"
            );
            assert!(
                right(search) <= right(sidebar),
                "search right edge at width {width}"
            );
            let title = cx.debug_bounds("workspace-title").unwrap();
            assert!(
                right(title) <= right(sidebar) - 6.,
                "title at width {width}"
            );
            let mut badge_right: Option<f32> = None;
            for index in 1..=5 {
                let badge = cx
                    .debug_bounds(
                        [
                            "sidebar-badge-1",
                            "sidebar-badge-2",
                            "sidebar-badge-3",
                            "sidebar-badge-4",
                            "sidebar-badge-5",
                        ][index - 1],
                    )
                    .unwrap();
                let label = cx
                    .debug_bounds(
                        [
                            "sidebar-label-1",
                            "sidebar-label-2",
                            "sidebar-label-3",
                            "sidebar-label-4",
                            "sidebar-label-5",
                        ][index - 1],
                    )
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
        for number in 1..=5 {
            cx.simulate_keystrokes(&format!("cmd-{number}"));
            shell.read_with(cx, |shell, _| {
                assert_eq!(
                    shell.fixture_state().0,
                    Route::from_shortcut(number).unwrap()
                )
            });
        }
        cx.simulate_keystrokes("cmd-alt-left");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Automations)
        });
        cx.simulate_keystrokes("cmd-alt-right");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Terminal)
        });
        cx.simulate_keystrokes("cmd-,");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Settings)
        });
        cx.simulate_keystrokes("cmd-1");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let settings = cx.debug_bounds("sidebar-settings").unwrap().center();
        cx.simulate_click(settings, Modifiers::default());
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.session.current(), Route::Settings)
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
    fn command_switcher_changes_workspace(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            bind_keys(cx);
            input::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        let store = shell.read_with(cx, |shell, _| shell.store.clone().unwrap());
        let personal = store
            .workspace_command(WorkspaceCommand::Create {
                name: "Personal".into(),
                icon: Some("P".into()),
                color: None,
            })
            .unwrap();
        store
            .workspace_command(WorkspaceCommand::Switch { id: "local".into() })
            .unwrap();
        cx.simulate_keystrokes("cmd-k");
        cx.simulate_input("Personal");
        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.palette_items(&shell.input.read(cx).content).len(), 1);
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(store.workspaces().current_id, personal);
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.overlays.borrow().active(), None)
        });
        cx.simulate_keystrokes("cmd-k");
        cx.simulate_input("create workspace");
        cx.simulate_keystrokes("enter");
        shell.read_with(cx, |shell, _| assert!(shell.creating_workspace));
        cx.simulate_input("New Project");
        shell.read_with(cx, |shell, cx| {
            assert_eq!(
                shell.workspace_name.read(cx).content.as_ref(),
                "New Project"
            )
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let state = store.workspaces();
        assert_eq!(
            state
                .workspaces
                .iter()
                .find(|workspace| workspace.id == state.current_id)
                .unwrap()
                .name,
            "New Project"
        );
    }

    #[gpui::test]
    fn search_shortcut_works_with_pane_or_no_focus(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            bind_keys(cx);
            input::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        let pane_focus = shell.read_with(cx, |shell, _| shell.pane_focus[0].clone());
        cx.update(|window, cx| window.focus(&pane_focus, cx));
        cx.simulate_keystrokes("cmd-k");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.overlays.borrow().active(), Some(Overlay::Search));
        });
        cx.simulate_keystrokes("escape");
        cx.update(|window, cx| window.blur(cx));
        cx.simulate_keystrokes("cmd-k");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.overlays.borrow().active(), Some(Overlay::Search));
        });
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
        let last = shell.read_with(cx, |shell, cx| {
            let count = shell.palette_items(&shell.input.read(cx).content).len();
            shell.picker_result_focus[count - 1].clone()
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
