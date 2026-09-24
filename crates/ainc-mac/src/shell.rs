use crate::{
    input::TextInput,
    model::{Availability, FontChoice, PAGES, Route, Session},
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
}
pub struct Shell {
    session: Session,
    overlays: Rc<RefCell<OverlayHost>>,
    assistant: Entity<crate::evee::AssistantPage>,
    tasks: Entity<crate::tasks::TasksPage>,
    _tasks_subscription: Subscription,
    _assistant_subscriptions: Vec<Subscription>,
    profile: crate::profile::Profile,
    path: PathBuf,
    focus: FocusHandle,
    input: Entity<TextInput>,
    picker_result_focus: Vec<FocusHandle>,
    picker_close_focus: FocusHandle,
    _input_subscription: Subscription,
    command_held: bool,
    palette_transition: Option<Instant>,
    selected: usize,
    notification_items: Vec<Notification>,
    save_error: bool,
    resizing_evee: bool,
    grip_opacity: f32,
    grip_animation: Option<(Instant, f32, f32)>,
    sidebar_width: f32,
    evee_progress: f32,
    evee_animation: Option<(Instant, f32, f32)>,
    sidebar_animation: Option<(Instant, f32, f32)>,
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
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default()
                    .join("Library/Application Support/Agentinc OS/session.json")
            });
        let (store, storage_error) = match crate::storage::Store::default_path().and_then(|path| crate::storage::Store::open(&path)) {
            Ok(store) => (Some(std::rc::Rc::new(store)), None),
            Err(_) => (None, Some("Local storage is unavailable. Check Application Support permissions and restart.".to_owned())),
        };
        Self::with_state(
            path,
            store,
            storage_error,
            crate::profile::Profile::local(),
            window,
            cx,
        )
    }

    fn with_state(
        path: PathBuf,
        store: Option<Rc<crate::storage::Store>>,
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
                            this.session.evee = true;
                            this.evee_animation = Some((Instant::now(), this.evee_progress, 1.));
                        }
                    }
                    this.save(cx);
                    cx.notify();
                },
            ),
        ];
        let tasks =
            cx.new(|cx| crate::tasks::TasksPage::new(store, storage_error, overlays.clone(), cx));
        let tasks_subscription = cx.observe(&tasks, |_, _, cx| cx.notify());
        let input = cx.new(TextInput::new);
        let subscription = cx.observe(&input, |this, _, cx| {
            this.selected = 0;
            cx.notify();
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let session = Session::load(&path);
        let evee_progress = if session.evee { 1. } else { 0. };
        let sidebar_width = if session.sidebar { SIDEBAR } else { 0. };
        Self {
            session,
            overlays,
            assistant,
            tasks,
            _tasks_subscription: tasks_subscription,
            _assistant_subscriptions: assistant_subscriptions,
            profile,
            sidebar_width,
            evee_progress,
            evee_animation: None,
            sidebar_animation: None,
            path,
            focus,
            input,
            picker_result_focus: PAGES.iter().map(|_| cx.focus_handle()).collect(),
            picker_close_focus: cx.focus_handle(),
            _input_subscription: subscription,
            command_held: false,
            palette_transition: None,
            selected: 0,
            notification_items: Vec::new(),
            save_error: false,
            resizing_evee: false,
            grip_opacity: 0.,
            grip_animation: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn fixture(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = crate::storage::Store::open(&path.with_extension("sqlite3")).unwrap();
        Self::with_state(
            path,
            Some(Rc::new(store)),
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
            self.session.evee,
        )
    }

    fn save(&mut self, cx: &mut Context<Self>) {
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
            Some(Overlay::AddTask | Overlay::DeleteTask(_)) => {
                self.tasks.read(cx).focus_handles(cx)
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
            Control::Sidebar => {
                self.session.sidebar = !self.session.sidebar;
                self.sidebar_animation = Some((
                    Instant::now(),
                    self.sidebar_width,
                    if self.session.sidebar { SIDEBAR } else { 0. },
                ));
            }
            Control::Evee => {
                self.session.evee = !self.session.evee;
                self.evee_animation = Some((
                    Instant::now(),
                    self.evee_progress,
                    if self.session.evee { 1. } else { 0. },
                ));
            }
            Control::Font(font) => self.session.font = font,
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
        action_button(
            ButtonSpec {
                id: id.into(),
                label: label.into(),
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
        )
    }
    fn icon_button(
        &self,
        id: &'static str,
        label: &'static str,
        name: &'static str,
        control: Control,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.button(id, label, control, cx)
            .size(px(HEADER_CONTROL))
            .justify_center()
            .child(icon(name, 16.))
    }
    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.session.current();
        row()
            .h(px(48.))
            .flex_shrink_0()
            .items_end()
            .pr(px(9.))
            .child(
                row()
                    .w(px(SIDEBAR + 60.))
                    .h_full()
                    .flex_shrink_0()
                    .justify_end()
                    .pr(px(9.))
                    .child(self.icon_button(
                        "sidebar",
                        "Toggle sidebar · ⌘ B",
                        "panel",
                        Control::Sidebar,
                        cx,
                    ))
                    .children([false, true].map(|forward| {
                        let name = if forward {
                            "chevronRight"
                        } else {
                            "chevronLeft"
                        };
                        if self.session.can_go(forward) {
                            self.icon_button(
                                if forward { "forward" } else { "back" },
                                name,
                                name,
                                if forward {
                                    Control::Forward
                                } else {
                                    Control::Back
                                },
                                cx,
                            )
                            .into_any_element()
                        } else {
                            row()
                                .size(px(30.))
                                .justify_center()
                                .opacity(0.3)
                                .child(icon(name, 16.))
                                .into_any_element()
                        }
                    })),
            )
            .child(
                row().relative().mb(px(-2.)).w(px(170.)).h(px(42.)).child(
                    row()
                        .absolute()
                        .left(px(14.))
                        .bottom_0()
                        .w(px(142.))
                        .h(px(40.))
                        .rounded_t(px(10.))
                        .child(tab_contour(true))
                        .child(
                            row()
                                .h_full()
                                .pl(px(14.))
                                .gap(px(7.))
                                .text_size(px(LABEL_SIZE))
                                .child(div().mt(px(1.)).child(icon(route.icon(), 14.)))
                                .child(route.label()),
                        ),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                self.button("search", "Go to · ⌘ K", Control::Search, cx)
                    .flex_shrink_0()
                    .w(px(224.))
                    .h(px(30.))
                    .mb(px(9.))
                    .pl(px(9.))
                    .pr(px(4.))
                    .bg(rgb(0x0a0a0a))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .text_color(rgb(MUTED))
                    .text_size(px(LABEL_SIZE))
                    .child(icon("search", 14.))
                    .child("Go to…")
                    .child(div().flex_1())
                    .child(shortcut_badge("⌘ K").w(px(36.))),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .ml(px(8.))
                    .mb(px(9.))
                    .child(self.icon_button(
                        "notifications",
                        "Notifications",
                        "bell",
                        Control::Notifications,
                        cx,
                    )),
            )
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = column().gap(px(2.));
        for page in PAGES.iter().filter(|page| page.in_sidebar) {
            let route = page.route;
            let index = page.shortcut.expect("sidebar route has shortcut");
            nav = nav.child(
                self.button(
                    ("nav", index as usize),
                    route.label(),
                    Control::Navigate(route),
                    cx,
                )
                .h(px(32.))
                .px(px(10.))
                .gap(px(12.))
                .text_size(px(LABEL_SIZE))
                .text_color(rgb(MUTED))
                .when(self.session.current() == route, |s| {
                    s.bg(rgb(HOVER))
                        .text_color(rgb(TEXT))
                        .font_weight(FontWeight::MEDIUM)
                })
                .child(nav_icon(route.icon(), self.session.current() == route))
                .child(route.label())
                .child(div().flex_1())
                .when(self.command_held, |s| {
                    s.child(shortcut_badge(format!("⌘{index}")))
                }),
            );
        }
        column()
            .w(px(SIDEBAR))
            .h_full()
            .flex_shrink_0()
            .px(px(12.))
            .pt(px(20.))
            .child(
                row()
                    .pl(px(6.5))
                    .pr(px(6.))
                    .gap(px(8.5))
                    .mb(px(22.))
                    .child(
                        row()
                            .size(px(24.))
                            .flex_shrink_0()
                            .justify_center()
                            .rounded(px(7.))
                            .border_1()
                            .border_color(rgb(0x353535))
                            .bg(rgb(HOVER))
                            .child("W"),
                    )
                    .child(div().text_size(px(LABEL_SIZE)).child("World Wide Webb")),
            )
            .child(nav)
            .child(div().flex_1())
            .child(
                row().h(px(50.)).flex_shrink_0().gap(px(4.)).child(
                    self.button(
                        "profile",
                        "Settings",
                        Control::Navigate(Route::Settings),
                        cx,
                    )
                    .h(px(40.))
                    .flex_1()
                    .px(px(10.))
                    .gap(px(8.))
                    .child(match &self.profile.photo {
                        Some(photo) => img(photo.clone())
                            .size(px(24.))
                            .rounded_full()
                            .into_any_element(),
                        None => row()
                            .size(px(24.))
                            .rounded_full()
                            .bg(rgb(0x272727))
                            .justify_center()
                            .child(self.profile.name.chars().next().unwrap_or('C').to_string())
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .text_size(px(LABEL_SIZE))
                            .child(self.profile.name.clone()),
                    ),
                ),
            )
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        row()
            .h(px(50.))
            .flex_shrink_0()
            .pl(px(PAGE_X))
            .pr(px(9.))
            .gap(px(12.))
            .border_b_1()
            .border_color(rgb(0x1a1a1a))
            .text_size(px(CAPTION_SIZE))
            .text_color(rgb(MUTED))
            .child(div().flex_1())
            .child(self.icon_button(
                "toggle-evee",
                "Toggle Evee panel · ⌘ ⇧ E",
                "panel",
                Control::Evee,
                cx,
            ))
    }
    fn command_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = Route::matching(&self.input.read(cx).content);
        panel()
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
                            ("command-result", index),
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
                    .text_size(px(CAPTION_SIZE))
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
                                .text_size(px(CAPTION_SIZE))
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
                                .text_size(px(LABEL_SIZE))
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
                                                    .text_size(px(CAPTION_SIZE))
                                                    .text_color(rgb(MUTED))
                                                    .child(item.relative_time.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(LABEL_SIZE))
                                            .text_color(rgb(MUTED))
                                            .child(item.body.clone()),
                                    ),
                            )
                            .when(item.unread, |s| {
                                s.child(div().size(px(5.)).rounded_full().bg(rgb(FOCUS)))
                            })
                    }),
            )
    }
    fn static_page(&self, route: Route, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, detail) = route.empty();
        let mut page = column().gap(px(28.));
        if route == Route::Today {
            for (index, destination) in [Route::Tasks, Route::Agents, Route::Calendar, Route::Home]
                .into_iter()
                .enumerate()
            {
                let (empty_title, empty_detail) = destination.empty();
                let (title, detail) = if destination == Route::Tasks {
                    (
                        self.tasks.read(cx).summary(),
                        "Open Tasks to add or complete a to-do.".to_owned(),
                    )
                } else {
                    (empty_title.to_owned(), empty_detail.to_owned())
                };
                page = page.child(
                    column()
                        .gap(px(14.))
                        .child(
                            self.button(
                                ("overview", index),
                                destination.label(),
                                Control::Navigate(destination),
                                cx,
                            )
                            .py(px(5.))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(destination.label()),
                            )
                            .child(div().flex_1())
                            .child(icon("arrowUpRight", 13.)),
                        )
                        .child(
                            row()
                                .gap(px(12.))
                                .pb(px(20.))
                                .border_b_1()
                                .border_color(rgb(0x1a1a1a))
                                .child(icon(destination.icon(), 20.))
                                .child(
                                    column().gap(px(5.)).child(title).child(
                                        div()
                                            .text_size(px(LABEL_SIZE))
                                            .text_color(rgb(MUTED))
                                            .child(detail),
                                    ),
                                ),
                        ),
                );
            }
        } else if !matches!(route, Route::Settings | Route::Assistant) {
            page = page.child(
                column()
                    .mt(px(28.))
                    .gap(px(12.))
                    .child(icon(route.icon(), 26.))
                    .child(div().font_weight(FontWeight::MEDIUM).child(title))
                    .child(
                        div()
                            .text_size(px(LABEL_SIZE))
                            .text_color(rgb(MUTED))
                            .child(detail),
                    ),
            );
        }
        if route == Route::Settings {
            page = page.w_full().max_w(px(640.)).child(
                column()
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Appearance"),
                    )
                    .child(
                        row()
                            .justify_between()
                            .gap(px(16.))
                            .child(div().child("Font"))
                            .child(
                                row()
                                    .p(px(4.))
                                    .gap(px(2.))
                                    .rounded(px(8.))
                                    .bg(rgb(0x1b1b1b))
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .child(
                                        self.button(
                                            "font-system",
                                            "System font",
                                            Control::Font(FontChoice::System),
                                            cx,
                                        )
                                        .h(px(32.))
                                        .px(px(11.))
                                        .bg(rgb(if self.session.font == FontChoice::System {
                                            0x333333
                                        } else {
                                            0x1b1b1b
                                        }))
                                        .child("System · SF Pro"),
                                    )
                                    .child(
                                        self.button(
                                            "font-helvetica",
                                            "Helvetica Neue",
                                            Control::Font(FontChoice::HelveticaNeue),
                                            cx,
                                        )
                                        .h(px(32.))
                                        .px(px(11.))
                                        .bg(rgb(
                                            if self.session.font == FontChoice::HelveticaNeue {
                                                0x333333
                                            } else {
                                                0x1b1b1b
                                            },
                                        ))
                                        .child("Helvetica Neue"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(8.))
                            .pt(px(28.))
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Accounts & connections"),
                    )
                    .child(self.assistant.update(cx, |this, cx| this.settings_view(cx))),
            );
        } else if route == Route::Assistant {
            page = page.child(
                self.assistant
                    .update(cx, |this, cx| this.conversations_view(cx)),
            );
        } else if route.spec().availability == Availability::Planned {
            page = page.child(
                column()
                    .gap(px(16.))
                    .mt(px(12.))
                    .child(
                        div()
                            .text_size(px(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
                            .child("Not available yet"),
                    )
                    .children(route.planned().iter().map(|(title, detail)| {
                        column()
                            .gap(px(6.))
                            .py(px(16.))
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(*title)
                            .child(
                                div()
                                    .text_size(px(LABEL_SIZE))
                                    .text_color(rgb(MUTED))
                                    .child(*detail),
                            )
                    })),
            );
        }
        page
    }
    fn evee(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        panel()
            .relative()
            .w(px(width))
            .child(
                div()
                    .id("evee-resizer")
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        this.grip_animation = Some((
                            Instant::now(),
                            this.grip_opacity,
                            if *hovered { 1. } else { 0. },
                        ));
                        cx.notify();
                    }))
                    .absolute()
                    .left(px(-11.))
                    .top_0()
                    .bottom_0()
                    .w(px(10.))
                    .tab_index(0)
                    .cursor(CursorStyle::ResizeLeftRight)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(2.))
                            .h(px(22.))
                            .rounded_full()
                            .bg(rgba(0x33333300 | (self.grip_opacity * 255.) as u32)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.resizing_evee = true;
                            cx.stop_propagation();
                        }),
                    )
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "left" => {
                                this.session.evee_width = (this.session.evee_width + 20.).min(480.)
                            }
                            "right" => {
                                this.session.evee_width = (this.session.evee_width - 20.).max(220.)
                            }
                            "home" => this.session.evee_width = 258.,
                            _ => return,
                        }
                        cx.stop_propagation();
                        this.save(cx);
                    })),
            )
            .h_full()
            .flex_shrink_0()
            .px(px(20.))
            .pb(px(20.))
            .child(
                row()
                    .h(px(50.))
                    .flex_shrink_0()
                    .gap(px(8.))
                    .text_size(px(LABEL_SIZE))
                    .ml(px(-20.))
                    .mr(px(-20.))
                    .pl(px(10.))
                    .pr(px(9.))
                    .border_b_1()
                    .border_color(rgb(0x1a1a1a))
                    .mb(px(20.))
                    .child(evee_logo(29.))
                    .child("Evee")
                    .child(div().flex_1())
                    .child(self.icon_button(
                        "close-evee",
                        "Close Evee panel",
                        "close",
                        Control::Evee,
                        cx,
                    )),
            )
            .child(div().flex_1().min_h_0().child(self.assistant.clone()))
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
        if reduced_motion() {
            if let Some((_, _, to)) = self.grip_animation.take() {
                self.grip_opacity = to;
            }
            if let Some((_, _, to)) = self.sidebar_animation.take() {
                self.sidebar_width = to;
            }
            if let Some((_, _, to)) = self.evee_animation.take() {
                self.evee_progress = to;
            }
            self.palette_transition = None;
        }
        if let Some((start, from, to)) = self.grip_animation {
            let t = (start.elapsed().as_secs_f32() / (HOVER_MS as f32 / 1000.)).min(1.);
            self.grip_opacity = from + (to - from) * t;
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.grip_animation = None;
            }
        }
        if let Some((start, from, to)) = self.sidebar_animation {
            let t = (start.elapsed().as_secs_f32() / (PANEL_MS as f32 / 1000.)).min(1.);
            let eased = 1. - (1. - t).powi(3);
            self.sidebar_width = from + (to - from) * eased;
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.sidebar_animation = None;
            }
        }
        if let Some((start, from, to)) = self.evee_animation {
            let t = (start.elapsed().as_secs_f32() / (PANEL_MS as f32 / 1000.)).min(1.);
            self.evee_progress = from + (to - from) * (1. - (1. - t).powi(3));
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.evee_animation = None;
            }
        }
        let sidebar_width = self.sidebar_width + 8. * (1. - self.sidebar_width / SIDEBAR);
        let max_evee_width =
            (f32::from(window.viewport_size().width) - sidebar_width - 378.).clamp(220., 480.);
        let evee_width = self.session.evee_width.min(max_evee_width);
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
            Route::Tasks => self.tasks.clone().into_any_element(),
            Route::Today
            | Route::Agents
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
            Some(Overlay::AddTask | Overlay::DeleteTask(_)) => {
                self.tasks.update(cx, |tasks, cx| tasks.overlay(cx))
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
            .text_size(px(BODY_SIZE))
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
                    if this.resizing_evee {
                        if event.dragging() {
                            this.session.evee_width =
                                (f32::from(window.viewport_size().width - event.position.x) - 8.)
                                    .clamp(220., max_evee_width);
                            cx.notify();
                            window.refresh();
                        } else {
                            this.resizing_evee = false;
                            this.save(cx);
                        }
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.resizing_evee {
                        this.resizing_evee = false;
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
            .child(
                row()
                    .flex_1()
                    .min_h_0()
                    .pt(px(48.))
                    .pb(px(8.))
                    .pr(px(8.))
                    .child(
                        div()
                            .w(px(self.sidebar_width))
                            .h_full()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .child(self.sidebar(cx)),
                    )
                    .pl(px(8. * (1. - self.sidebar_width / SIDEBAR)))
                    .child(
                        row()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .gap(px(PANEL_GAP * self.evee_progress))
                            .child(
                                panel()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .overflow_hidden()
                                    .child(self.toolbar(cx))
                                    .child(
                                        column()
                                            .id("page")
                                            .flex_1()
                                            .min_h_0()
                                            .overflow_y_scroll()
                                            .px(px(PAGE_X))
                                            .py(px(PAGE_Y))
                                            .child(div().relative().child(content)),
                                    ),
                            )
                            .when(self.evee_progress > 0., |s| {
                                s.child(
                                    div()
                                        .w(px(evee_width * self.evee_progress))
                                        .h_full()
                                        .flex_shrink_0()
                                        .when(self.evee_animation.is_some(), |s| {
                                            s.overflow_hidden()
                                        })
                                        .child(self.evee(evee_width, cx)),
                                )
                            }),
                    ),
            )
            // Header paints after panels so active tabs cover their top border.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(self.header(cx)),
            )
            .when(self.save_error, |s| {
                s.child(
                    div()
                        .absolute()
                        .bottom(px(12.))
                        .left(px(190.))
                        .px(px(12.))
                        .py(px(8.))
                        .bg(rgb(0x241818))
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
                        .absolute()
                        .inset_0()
                        .cursor_default()
                        .bg(rgba(0x000000aa))
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
    }
}
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(
        (1..=8).map(|n| KeyBinding::new(&format!("cmd-{n}"), NavigateRoute(n), Some("Control"))),
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
        model::{PAGES, Route},
        overlay::Overlay,
    };
    use gpui::{Focusable, TestAppContext};

    #[gpui::test]
    fn routes_and_history_use_shell_actions(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir_in("target").unwrap();
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
        let dir = tempfile::tempdir_in("target").unwrap();
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
        let dir = tempfile::tempdir_in("target").unwrap();
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
