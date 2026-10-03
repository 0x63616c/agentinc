//! The body under the title bar: the sidebar pane (width, resize handle,
//! open/close animation) beside the content card with its status bar.
use super::*;
use crate::ui_state::{SIDEBAR_DEFAULT, SIDEBAR_MAX, SIDEBAR_MIN};

impl Shell {
    /// The content card: the current page over the status bar.
    pub(super) fn main_area(&self, content: AnyElement) -> Div {
        let terminal = self.ui_state.current() == Route::Terminal;
        panel()
            .debug_selector(|| "main-pane".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_hidden()
            .child(
                column()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .when(terminal, |pane| pane.p(px(SPACE_2)))
                    .child(content),
            )
            .child(
                status_bar().child(
                    div()
                        .id("sidebar.version")
                        .debug_selector(|| "sidebar-version".into())
                        .accessibility_id("sidebar.version")
                        .role(accesskit::Role::Label)
                        .aria_label(ainc_release::identity::version())
                        .text_color(rgb(TEXT_TERTIARY))
                        .child(ainc_release::identity::version()),
                ),
            )
    }
    pub(super) fn update_required(&self, ui: &mut Ui<Self>) -> AnyElement {
        column()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(SPACE_5))
            .bg(rgb(SHELL))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .text_size(type_size(DISPLAY_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Update to continue"),
            )
            .child(caption(
                "This version of AgentInc is older than its daemon.",
            ))
            .child(
                Button::new("required-update", "Check for Updates")
                    .primary()
                    .large()
                    .build(ui, |_, _, cx| crate::updates::open(cx, true)),
            )
            .into_any_element()
    }
    pub(super) fn toggle_sidebar(&mut self) {
        let preference = &mut self.ui_state.sidebar;
        preference.open = !preference.open;
        self.sidebar_animation = Some((
            Instant::now(),
            self.sidebar_visible,
            if preference.open {
                preference.width
            } else {
                0.
            },
        ));
    }

    fn resize_handle(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("left-resizer")
            .accessibility_id("pane.left.resize")
            .role(accesskit::Role::Splitter)
            .aria_label("Resize left pane")
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                this.grip_animation = Some((
                    Instant::now(),
                    this.grip_opacity,
                    if *hovered { 1. } else { 0. },
                ));
                cx.notify();
            }))
            .w(px(10.))
            .tab_index(0)
            .track_focus(&self.sidebar_focus)
            .cursor(CursorStyle::ResizeLeftRight)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(2.))
                    .h(px(22.))
                    .rounded_full()
                    .bg(rgba(GRIP_TINT | (self.grip_opacity * 255.) as u32)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.resizing = true;
                    window.focus(&this.sidebar_focus, cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(|_, _, _| {})
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let current = this.ui_state.sidebar.width;
                let value = match event.keystroke.key.as_str() {
                    "left" => current - 20.,
                    "right" => current + 20.,
                    "home" => SIDEBAR_DEFAULT,
                    _ => return,
                };
                this.set_sidebar_width(value, window);
                cx.stop_propagation();
                this.save(cx);
            }))
    }

    fn set_sidebar_width(&mut self, width: f32, window: &mut Window) {
        let width = width.clamp(SIDEBAR_MIN, SIDEBAR_MAX.min(self.sidebar_limit(window)));
        self.ui_state.sidebar.width = width;
        self.sidebar_animation = None;
        self.sidebar_visible = width;
        window.refresh();
    }

    fn sidebar_limit(&self, window: &Window) -> f32 {
        (f32::from(window.viewport_size().width) - 378.).clamp(SIDEBAR_MIN, SIDEBAR_MAX)
    }

    pub(super) fn resize_from_pointer(&mut self, position: Point<Pixels>, window: &mut Window) {
        if self.resizing {
            self.set_sidebar_width(f32::from(position.x), window);
        }
    }

    /// Advance the grip and open/close animations; returns the sidebar's visible width.
    pub(super) fn animate_sidebar(&mut self, window: &mut Window) {
        if reduced_motion() {
            if let Some((_, _, to)) = self.grip_animation.take() {
                self.grip_opacity = to;
            }
            if let Some((_, _, to)) = self.sidebar_animation.take() {
                self.sidebar_visible = to;
            }
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
            self.sidebar_visible = from + (to - from) * (1. - (1. - t).powi(3));
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.sidebar_animation = None;
            }
        }
    }

    /// The sidebar beside the content card, under the title bar.
    pub(super) fn body(&self, sidebar: AnyElement, content: AnyElement, ui: &mut Ui<Self>) -> Div {
        let visible = self.sidebar_visible.min(self.sidebar_limit(ui.window));
        let saved = self.ui_state.sidebar.width;
        row()
            .relative()
            .flex_1()
            .min_h_0()
            .pt(px(TITLEBAR_HEIGHT))
            .pb(px(PANEL_GAP))
            .pr(px(PANEL_GAP))
            .child(
                div()
                    .relative()
                    .w(px(visible))
                    .h_full()
                    .flex_shrink_0()
                    .child(
                        div()
                            .w(px(visible))
                            .h_full()
                            .overflow_hidden()
                            .child(div().w(px(saved)).h_full().child(sidebar)),
                    ),
            )
            .pl(px(PANEL_GAP * (1. - visible / saved)))
            .child(
                row()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(column().flex_1().min_w_0().h_full().child(content)),
            )
            .when(self.ui_state.sidebar.open && visible > 0., |body| {
                body.child(
                    self.resize_handle(ui.cx)
                        .absolute()
                        .left(px(visible - 5.))
                        .top(px(TITLEBAR_HEIGHT))
                        .bottom(px(PANEL_GAP)),
                )
            })
    }
}
