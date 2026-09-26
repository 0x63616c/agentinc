use super::*;

// One continuous contour avoids vertical border tails at the inverse shoulders.
// The current space label is 142px wide; its shoulders meet the panel border.
pub fn current_space_contour() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let p = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
            let contour = |path: &mut PathBuilder| {
                path.move_to(p(0., 38.));
                path.cubic_bezier_to(p(12., 26.), p(8., 38.), p(12., 34.));
                path.line_to(p(12., 10.));
                path.cubic_bezier_to(p(22., 0.), p(12., 4.477), p(16.477, 0.));
                path.line_to(p(144., 0.));
                path.cubic_bezier_to(p(154., 10.), p(149.523, 0.), p(154., 4.477));
                path.line_to(p(154., 26.));
                path.cubic_bezier_to(p(166., 38.), p(154., 34.), p(158., 38.));
            };
            let mut fill = PathBuilder::fill();
            contour(&mut fill);
            fill.line_to(p(166., 40.));
            fill.line_to(p(0., 40.));
            fill.close();
            if let Ok(path) = fill.build() {
                window.paint_path(path, rgb(SURFACE));
            }
            let mut line = PathBuilder::stroke(px(1.));
            contour(&mut line);
            if let Ok(path) = line.build() {
                window.paint_path(path, rgb(BORDER));
            }
        },
    )
    .absolute()
    .top_0()
    .left(px(-12.))
    .w(px(166.))
    .h(px(40.))
}

impl Shell {
    fn titlebar_space(&self, id: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id(id)
            .debug_selector(move || id.into())
            .h_full()
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_move(|event: &MouseMoveEvent, window, _| {
                if event.dragging() {
                    window.start_window_move();
                }
            })
            .on_click(cx.listener(|_this, event: &ClickEvent, window, _| {
                if event.click_count() == 2 {
                    #[cfg(test)]
                    {
                        _this.titlebar_zoom_requests += 1;
                    }
                    window.titlebar_double_click();
                }
            }))
    }

    pub(super) fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.session.current();
        let unread = self
            .notification_items
            .iter()
            .filter(|item| item.unread)
            .count();
        row()
            .h(px(TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .items_end()
            .pr(px(9.))
            .child(
                row()
                    .w(px(self.session.panes[pane::Side::Left.index()].width + 60.))
                    .h_full()
                    .flex_shrink_0()
                    .justify_end()
                    .pr(px(9.))
                    .child(self.titlebar_space("titlebar-left-space", cx).flex_1())
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
                                .size(px(HEADER_CONTROL))
                                .justify_center()
                                .opacity(0.3)
                                .child(icon(name, HEADER_ICON_SIZE))
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
                        .child(current_space_contour())
                        .child(
                            row()
                                .h_full()
                                .pl(px(14.))
                                .gap(px(7.))
                                .text_size(type_size(LABEL_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .child(div().mt(px(1.)).child(icon(route.icon(), 14.)))
                                .child(route.label()),
                        ),
                ),
            )
            .child(self.titlebar_space("titlebar-center-space", cx).flex_1())
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .ml(px(CONTROL_GAP))
                    .mb(px(9.))
                    .child(self.icon_button(
                        "notifications",
                        "Notifications",
                        "bell",
                        Control::Notifications,
                        cx,
                    ))
                    .when(unread > 0, |s| {
                        s.child(
                            div()
                                .absolute()
                                .top(px(4.))
                                .right(px(4.))
                                .size(px(7.))
                                .rounded_full()
                                .border_1()
                                .border_color(rgb(SHELL))
                                .bg(rgb(ACCENT)),
                        )
                    }),
            )
    }
}
