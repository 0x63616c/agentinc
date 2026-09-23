use crate::{
    input::TextInput,
    model::{FontChoice, Session, Space},
    style::*,
};
use gpui::{prelude::*, *};
use std::{path::PathBuf, time::Instant};
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
struct NavigateSpace(usize);

#[derive(Clone, Copy)]
enum Control {
    Open(Space),
    Navigate(Space),
    Back,
    Forward,
    Search,
    Sidebar,
    Evee,
    Notifications,
    MarkAllRead,
    Dismiss,
    AssistantSetup,
    Font(FontChoice),
}
pub struct Shell {
    session: Session,
    assistant: Entity<crate::evee::Evee>,
    tasks: Entity<crate::tasks::Tasks>,
    _tasks_subscription: Subscription,
    profile: crate::profile::Profile,
    path: PathBuf,
    focus: FocusHandle,
    input: Entity<TextInput>,
    picker_result_focus: Vec<FocusHandle>,
    picker_close_focus: FocusHandle,
    _input_subscription: Subscription,
    command: bool,
    command_held: bool,
    palette_transition: Option<Instant>,
    selected: usize,
    notifications: bool,
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
        let assistant =
            cx.new(|cx| crate::evee::Evee::new(store.clone(), storage_error.clone(), cx));
        let tasks = cx.new(|cx| crate::tasks::Tasks::new(store, storage_error, cx));
        let tasks_subscription = cx.observe(&tasks, |_, _, cx| cx.notify());
        let input = cx.new(TextInput::new);
        let subscription = cx.observe(&input, |this, _, cx| {
            this.selected = 0;
            cx.notify();
        });
        let focus = cx.focus_handle();
        window.focus(&focus);
        let session = Session::load(&path);
        let evee_progress = if session.evee { 1. } else { 0. };
        let sidebar_width = if session.sidebar { SIDEBAR } else { 0. };
        Self {
            session,
            assistant,
            tasks,
            _tasks_subscription: tasks_subscription,
            profile: crate::profile::Profile::local(),
            sidebar_width,
            evee_progress,
            evee_animation: None,
            sidebar_animation: None,
            path,
            focus,
            input,
            picker_result_focus: Space::ALL.iter().map(|_| cx.focus_handle()).collect(),
            picker_close_focus: cx.focus_handle(),
            _input_subscription: subscription,
            command: false,
            command_held: false,
            palette_transition: None,
            selected: 0,
            notifications: false,
            notification_items: Vec::new(),
            save_error: false,
            resizing_evee: false,
            grip_opacity: 0.,
            grip_animation: None,
        }
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
        window.focus(&self.input.focus_handle(cx));
    }
    fn cycle_focus(&self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.command {
            if backwards {
                window.focus_prev();
            } else {
                window.focus_next();
            }
            return;
        }
        // Only rendered picker controls participate, including the empty-results case.
        let mut handles = vec![self.input.focus_handle(cx)];
        if self.command {
            handles.push(self.picker_close_focus.clone());
        }
        let count = Space::matching(&self.input.read(cx).content).len();
        handles.extend(self.picker_result_focus.iter().take(count).cloned());
        let current = handles.iter().position(|handle| handle.is_focused(window));
        let next = match (current, backwards) {
            (Some(index), true) => (index + handles.len() - 1) % handles.len(),
            (Some(index), false) => (index + 1) % handles.len(),
            (None, true) => handles.len() - 1,
            (None, false) => 0,
        };
        window.focus(&handles[next]);
    }
    fn dispatch(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        let before = (self.session.active, self.session.current());
        match control {
            Control::Back | Control::Forward => {
                self.session.go(matches!(control, Control::Forward));
                self.command = false;
                window.focus(&self.focus);
            }
            Control::Navigate(space) => {
                self.session.navigate(space);
                self.command = false;
                self.notifications = false;
                window.focus(&self.focus);
            }
            Control::Open(space) => {
                self.session.open(space);
                self.command = false;
                self.notifications = false;
                window.focus(&self.focus);
            }
            Control::Search => {
                self.command = true;
                self.palette_transition = Some(Instant::now());
                self.notifications = false;
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
            Control::AssistantSetup => {
                self.session.evee = true;
                self.evee_animation = Some((Instant::now(), self.evee_progress, 1.));
                self.assistant
                    .update(cx, |assistant, cx| assistant.open_setup(cx));
            }
            Control::Font(font) => self.session.font = font,
            Control::Notifications => self.notifications = !self.notifications,
            Control::MarkAllRead => {
                for item in &mut self.notification_items {
                    item.unread = false;
                }
            }
            Control::Dismiss => {
                self.command = false;
                self.notifications = false;
                window.focus(&self.focus);
            }
        }
        if before != (self.session.active, self.session.current()) {
            self.tasks.update(cx, |tasks, cx| {
                tasks.dismiss(cx);
            });
        }
        self.save(cx);
        window.refresh();
    }
    fn button(
        &self,
        id: impl Into<ElementId>,
        _label: impl Into<SharedString>,
        control: Control,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        row()
            .id(id)
            .tab_index(0)
            .cursor_pointer()
            .rounded(px(6.))
            .gap(px(8.))
            .hover(move |s| {
                if matches!(control, Control::Open(_)) {
                    s.text_color(rgb(TEXT))
                } else {
                    s.bg(rgb(0x191919)).text_color(rgb(TEXT))
                }
            })
            .focus(move |s| {
                if matches!(control, Control::Open(_)) {
                    s.border_color(rgb(0x555555))
                } else {
                    s.bg(rgb(0x1d2520)).border_color(rgb(FOCUS))
                }
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.dispatch(control, window, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    cx.stop_propagation();
                    this.dispatch(control, window, cx);
                }
            }))
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
            .size(px(30.))
            .justify_center()
            .child(icon(name, 16.))
    }
    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let space = self.session.current().unwrap_or(Space::Today);
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
                                .text_size(px(12.))
                                .child(div().mt(px(1.)).child(icon(space.icon(), 14.)))
                                .child(space.label()),
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
                self.button("search", "Search spaces · ⌘ K", Control::Search, cx)
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
                    .text_size(px(12.))
                    .child(icon("search", 14.))
                    .child("Search")
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
        for (index, space) in Space::ALL[..7].iter().copied().enumerate() {
            if index == 3 {
                nav = nav.child(
                    div()
                        .mt(px(24.))
                        .mb(px(10.))
                        .px(px(12.))
                        .text_size(px(11.))
                        .text_color(rgb(0x888888))
                        .child("Spaces"),
                );
            }
            nav = nav.child(
                self.button(("nav", index), space.label(), Control::Navigate(space), cx)
                    .h(px(32.))
                    .px(px(10.))
                    .gap(px(12.))
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .when(self.session.current() == Some(space), |s| {
                        s.bg(rgb(0x191919))
                            .text_color(rgb(TEXT))
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .child(nav_icon(
                        space.icon(),
                        self.session.current() == Some(space),
                    ))
                    .child(space.label())
                    .child(div().flex_1())
                    .when(self.command_held, |s| {
                        s.child(shortcut_badge(format!("⌘{}", index + 1)))
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
                            .bg(rgb(0x191919))
                            .child("W"),
                    )
                    .child(div().text_size(px(12.)).child("World Wide Webb")),
            )
            .child(nav)
            .child(div().flex_1())
            .child(
                row().h(px(50.)).flex_shrink_0().gap(px(4.)).child(
                    self.button(
                        "profile",
                        "Settings",
                        Control::Navigate(Space::Settings),
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
                    .child(div().text_size(px(12.)).child(self.profile.name.clone())),
                ),
            )
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        row()
            .h(px(50.))
            .flex_shrink_0()
            .pl(px(26.))
            .pr(px(9.))
            .gap(px(12.))
            .border_b_1()
            .border_color(rgb(0x1a1a1a))
            .text_size(px(11.))
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
        let matches = Space::matching(&self.input.read(cx).content);
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
                div()
                    .px(px(16.))
                    .pt(px(14.))
                    .pb(px(8.))
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child("Spaces"),
            )
            .child(
                column()
                    .px(px(8.))
                    .pb(px(8.))
                    .when(matches.is_empty(), |s| {
                        s.child(
                            column()
                                .p(px(24.))
                                .gap(px(6.))
                                .child("No matching spaces")
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(MUTED))
                                        .child("Try a space name, such as Home or Tasks."),
                                ),
                        )
                    })
                    .children(matches.iter().copied().enumerate().map(|(index, space)| {
                        self.button(
                            ("command-result", index),
                            space.label(),
                            Control::Open(space),
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
                        .when(index == self.selected, |s| s.bg(rgb(0x191919)))
                        .child(icon(space.icon(), 17.))
                        .child(space.label())
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
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child(shortcut_badge("↑ ↓"))
                    .child("Navigate")
                    .child(div().flex_1())
                    .child(shortcut_badge("↵"))
                    .child("Open space"),
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
                    .child(
                        self.button("mark-all-read", "Mark all read", Control::MarkAllRead, cx)
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .when(self.notification_items.is_empty(), |s| s.opacity(0.35))
                            .child("Mark all read"),
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
                        .py(px(36.))
                        .px(px(16.))
                        .items_center()
                        .gap(px(8.))
                        .child(icon("bell", 22.))
                        .child(div().font_weight(FontWeight::MEDIUM).child("All caught up"))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(MUTED))
                                .child("New notifications will appear here."),
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
                                                    .text_size(px(11.))
                                                    .text_color(rgb(MUTED))
                                                    .child(item.relative_time.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
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
    fn empty_page(&self, space: Space, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, detail) = space.empty();
        let mut page = column().gap(px(28.));
        if space == Space::Today {
            for (index, destination) in [Space::Tasks, Space::Agents, Space::Calendar, Space::Home]
                .into_iter()
                .enumerate()
            {
                let (empty_title, empty_detail) = destination.empty();
                let (title, detail) = if destination == Space::Tasks {
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
                                            .text_size(px(12.))
                                            .text_color(rgb(MUTED))
                                            .child(detail),
                                    ),
                                ),
                        ),
                );
            }
        } else if !matches!(space, Space::Settings | Space::Evee) {
            page = page.child(
                column()
                    .mt(px(28.))
                    .gap(px(12.))
                    .child(icon(space.icon(), 26.))
                    .child(div().font_weight(FontWeight::MEDIUM).child(title))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(detail),
                    ),
            );
        }
        if space == Space::Settings {
            page = page.child(
                column()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Appearance"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child("Font"),
                    )
                    .child(
                        row()
                            .gap(px(8.))
                            .child(
                                self.button(
                                    "font-system",
                                    "System font",
                                    Control::Font(FontChoice::System),
                                    cx,
                                )
                                .h(px(36.))
                                .px(px(12.))
                                .border_1()
                                .border_color(rgb(if self.session.font == FontChoice::System {
                                    FOCUS
                                } else {
                                    BORDER
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
                                .h(px(36.))
                                .px(px(12.))
                                .border_1()
                                .border_color(rgb(
                                    if self.session.font == FontChoice::HelveticaNeue {
                                        FOCUS
                                    } else {
                                        BORDER
                                    },
                                ))
                                .child("Helvetica Neue"),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(28.))
                            .pt(px(24.))
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Accounts & connections"),
                    )
                    .child(
                        self.button("setup-evee", "OpenAI setup", Control::AssistantSetup, cx)
                            .p(px(12.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .child("OpenAI connection")
                            .child(div().flex_1())
                            .child("Set up →"),
                    ),
            );
        } else if space == Space::Evee {
            page = page.child(
                column()
                    .gap(px(16.))
                    .child("Your conversation is in the Evee panel.")
                    .child(
                        self.button("open-evee-chat", "Open Evee", Control::AssistantSetup, cx)
                            .p(px(12.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .child("Open Evee setup"),
                    ),
            );
        } else if space != Space::Today {
            page = page.child(
                column()
                    .gap(px(16.))
                    .mt(px(12.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child("Planned for this space"),
                    )
                    .children(space.planned().iter().map(|(title, detail)| {
                        column()
                            .gap(px(6.))
                            .py(px(16.))
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(*title)
                            .child(
                                div()
                                    .text_size(px(12.))
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
                    .text_size(px(12.))
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
        if self.command || self.session.current().is_none() {
            let matches = Space::matching(&self.input.read(cx).content);
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
                    if let Some(space) = matches.get(self.selected) {
                        self.dispatch(Control::Open(*space), window, cx);
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
            let t = (start.elapsed().as_secs_f32() / 0.14).min(1.);
            self.grip_opacity = from + (to - from) * t;
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.grip_animation = None;
            }
        }
        if let Some((start, from, to)) = self.sidebar_animation {
            let t = (start.elapsed().as_secs_f32() / 0.18).min(1.);
            let eased = 1. - (1. - t).powi(3);
            self.sidebar_width = from + (to - from) * eased;
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.sidebar_animation = None;
            }
        }
        if let Some((start, from, to)) = self.evee_animation {
            let t = (start.elapsed().as_secs_f32() / 0.18).min(1.);
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
            let t = (instant.elapsed().as_secs_f32() / 0.16).min(1.);
            if t < 1. {
                window.request_animation_frame();
            } else {
                *start = None;
            }
            1. - (1. - t).powi(3)
        };
        let palette_progress = progress(&mut self.palette_transition);
        let content = if self.session.current() == Some(Space::Tasks) {
            self.tasks.clone().into_any_element()
        } else if let Some(space) = self.session.current() {
            self.empty_page(space, cx).into_any_element()
        } else {
            self.empty_page(Space::Today, cx).into_any_element()
        };
        column()
            .id("shell")
            .relative()
            .size_full()
            .bg(rgb(SHELL))
            .text_color(rgb(TEXT))
            .font_family(self.session.font.family())
            .text_size(px(13.))
            .line_height(relative(1.5))
            .track_focus(&self.focus)
            .key_context("Control")
            .on_key_down(cx.listener(Self::keys))
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                this.command_held = event.modifiers.platform;
                cx.notify();
            }))
            .on_action(cx.listener(|this, action: &NavigateSpace, w, cx| {
                if let Some(space) = Space::ALL[..7].get(action.0) {
                    this.dispatch(Control::Navigate(*space), w, cx);
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
                if this.tasks.update(cx, |tasks, cx| tasks.dismiss(cx)) {
                    w.focus(&this.focus);
                    return;
                }
                if this.command || this.notifications {
                    this.dispatch(Control::Dismiss, w, cx)
                } else {
                    w.focus(&this.focus);
                }
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
                            .gap(px(10. * self.evee_progress))
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
                                            .px(px(26.))
                                            .py(px(28.))
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
            .when(self.notifications, |s| s.child(self.notification_panel(cx)))
            .when_some(
                self.tasks.update(cx, |tasks, cx| tasks.overlay(cx)),
                |s, overlay| s.child(overlay),
            )
            .when(self.command, |s| {
                s.child(
                    div()
                        .id("command-backdrop")
                        .absolute()
                        .inset_0()
                        .cursor_default()
                        .bg(rgba(0x00000099))
                        .flex()
                        .justify_center()
                        .items_start()
                        .pt(px(80.))
                        .on_mouse_move(|_, _, cx| cx.stop_propagation())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, w, cx| {
                            cx.stop_propagation();
                            this.dispatch(Control::Dismiss, w, cx);
                        }))
                        .child(
                            div()
                                .id("command-dialog")
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .relative()
                                .opacity(0.65 + 0.35 * palette_progress)
                                .child(self.command_palette(cx)),
                        ),
                )
            })
    }
}
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(
        (1..=7)
            .map(|n| KeyBinding::new(&format!("cmd-{n}"), NavigateSpace(n - 1), Some("Control"))),
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
