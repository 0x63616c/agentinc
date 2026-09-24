use super::*;

impl Shell {
    pub(super) fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.session.current();
        row()
            .h(px(48.))
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
                        .child(current_space_contour())
                        .child(
                            row()
                                .h_full()
                                .pl(px(14.))
                                .gap(px(7.))
                                .text_size(type_size(LABEL_SIZE))
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
                self.button("shell.search", "Search · ⌘ K", Control::Search, cx)
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
                    .text_size(type_size(LABEL_SIZE))
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
}
