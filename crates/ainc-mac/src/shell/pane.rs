use super::*;
use crate::model::PANE_WIDTHS;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Side {
    Left,
}

impl Side {
    pub(super) fn index(self) -> usize {
        0
    }

    fn bounds(self) -> (f32, f32, f32) {
        PANE_WIDTHS[self.index()]
    }
}

impl Shell {
    pub(super) fn toggle_pane(&mut self, side: Side) {
        let index = side.index();
        let preference = &mut self.session.panes[index];
        preference.open = !preference.open;
        self.pane_animation[index] = Some((
            Instant::now(),
            self.pane_visible[index],
            if preference.open {
                preference.width
            } else {
                0.
            },
        ));
    }

    pub(super) fn resize_handle(&self, side: Side, cx: &mut Context<Self>) -> Stateful<Div> {
        let index = side.index();
        div()
            .id("left-resizer")
            .accessibility_id("pane.left.resize")
            .role(accesskit::Role::Splitter)
            .aria_label("Resize left pane")
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                this.grip_animation[index] = Some((
                    Instant::now(),
                    this.grip_opacity[index],
                    if *hovered { 1. } else { 0. },
                ));
                cx.notify();
            }))
            .w(px(10.))
            .tab_index(0)
            .track_focus(&self.pane_focus[index])
            .cursor(CursorStyle::ResizeLeftRight)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(2.))
                    .h(px(22.))
                    .rounded_full()
                    .bg(rgba(GRIP_TINT | (self.grip_opacity[index] * 255.) as u32)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.resizing = Some(side);
                    window.focus(&this.pane_focus[index], cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(|_, _, _| {})
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let current = this.session.panes[index].width;
                let value = match event.keystroke.key.as_str() {
                    "left" => current - 20.,
                    "right" => current + 20.,
                    "home" => side.bounds().2,
                    _ => return,
                };
                this.set_pane_width(side, value, window);
                cx.stop_propagation();
                this.save(cx);
            }))
    }

    pub(super) fn set_pane_width(&mut self, side: Side, width: f32, window: &mut Window) {
        let index = side.index();
        let (min, max, _) = side.bounds();
        let width = width.clamp(min, max.min(self.pane_limit(side, window)));
        self.session.panes[index].width = width;
        self.pane_animation[index] = None;
        self.pane_visible[index] = width;
        window.refresh();
    }

    pub(super) fn pane_limit(&self, side: Side, window: &Window) -> f32 {
        (f32::from(window.viewport_size().width) - 378.).clamp(side.bounds().0, side.bounds().1)
    }

    pub(super) fn resize_from_pointer(&mut self, position: Point<Pixels>, window: &mut Window) {
        let Some(side) = self.resizing else { return };
        let width = f32::from(position.x);
        self.set_pane_width(side, width, window);
    }

    pub(super) fn pane(&self, side: Side, content: AnyElement, visible: f32) -> Div {
        let index = side.index();
        let width = self.session.panes[index].width;
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
                    .child(div().w(px(width)).h_full().child(content)),
            )
    }
}
