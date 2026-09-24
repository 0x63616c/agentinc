use super::*;

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
            .child(self.titlebar_space("titlebar-center-space", cx).flex_1())
            .child(
                self.button("shell.search", "Search · ⌘ K", Control::Search, cx)
                    .debug_selector(|| "shell.search".into())
                    .flex_shrink_0()
                    .w(px(224.))
                    .h(px(30.))
                    .mb(px(9.))
                    .pl(px(HEADER_SEARCH_LEFT_INSET))
                    .pr(px(HEADER_SEARCH_RIGHT_INSET))
                    .bg(rgb(SURFACE_SEARCH))
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
                    .ml(px(CONTROL_GAP))
                    .mb(px(9.))
                    .child(self.icon_button(
                        "toggle-evee",
                        "Toggle Evee panel · ⌘ ⇧ E",
                        "panel",
                        Control::Evee,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .ml(px(CONTROL_GAP))
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
