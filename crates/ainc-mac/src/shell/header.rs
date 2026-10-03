//! Bounded, horizontally scrollable tabs and their panel-joining contours.
use super::*;

// One continuous contour avoids vertical border tails at the inverse shoulders.
// The current Page tab is `TAB_WIDTH` wide; its shoulders meet the panel border.
pub fn current_page_contour() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let p = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
            let (w, h) = (TAB_CONTOUR_WIDTH, TAB_FACE_HEIGHT);
            let (s, r) = (TAB_CONTOUR_SHOULDER, TAB_RADIUS);
            // Bezier handles: a third of a shoulder, and the circle constant on the corner.
            let (third, k) = (s / 3., r * 0.4477);
            let contour = |path: &mut PathBuilder| {
                path.move_to(p(0., h));
                path.cubic_bezier_to(p(s, h - s), p(s - third, h), p(s, h - third));
                path.line_to(p(s, r));
                path.cubic_bezier_to(p(s + r, 0.), p(s, k), p(s + k, 0.));
                path.line_to(p(w - s - r, 0.));
                path.cubic_bezier_to(p(w - s, r), p(w - s - k, 0.), p(w - s, k));
                path.line_to(p(w - s, h - s));
                path.cubic_bezier_to(p(w, h), p(w - s, h - third), p(w - s + third, h));
            };
            let mut fill = PathBuilder::fill();
            contour(&mut fill);
            fill.line_to(p(w, TAB_HEIGHT));
            fill.line_to(p(0., TAB_HEIGHT));
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
    .left(px(-TAB_CONTOUR_SHOULDER))
    .w(px(TAB_CONTOUR_WIDTH))
    .h(px(TAB_HEIGHT))
}

impl Shell {
    fn header_left_width(&self, window: &Window) -> f32 {
        (self.sidebar_visible.min(self.sidebar_limit(window)) + HEADER_LEFT_EXTRA)
            .max(HEADER_TRAFFIC_WIDTH + HEADER_CONTROL * 3. + HEADER_CONTROLS_INSET)
    }

    pub(super) fn tab_viewport(&self, window: &Window) -> Pixels {
        (window.viewport_size().width
            - px(self.header_left_width(window) + HEADER_EDGE_INSET + HEADER_CONTROL * 2.))
        .max(px(0.))
    }

    fn tab(&self, index: usize, route: Route, ui: &mut Ui<Self>) -> Div {
        let selected = self.ui_state.active_tab() == index;
        let group: SharedString = format!("tab-{index}").into();
        row()
            .relative()
            .flex_shrink_0()
            .w(px(TAB_SLOT_WIDTH))
            .h(px(TAB_SLOT_HEIGHT))
            .child(
                self.button(("tab", index), route.label(), Control::SelectTab(index), ui)
                    .accessibility_id(format!("tabs.{index}"))
                    .debug_selector(move || format!("tabs.{index}"))
                    .role(accesskit::Role::Tab)
                    .aria_selected(selected)
                    .group(group.clone())
                    .absolute()
                    .left(px(TAB_CONTOUR_SHOULDER))
                    .bottom_0()
                    .w(px(TAB_WIDTH))
                    .h(px(TAB_HEIGHT))
                    .rounded_t(px(TAB_RADIUS))
                    .bg(rgba(SHELL << 8))
                    .when(selected, |tab| tab.child(current_page_contour()))
                    .child(
                        row()
                            .relative()
                            .size_full()
                            .pb(px(TAB_LABEL_LIFT))
                            .pl(px(TAB_LABEL_INSET))
                            .pr(px(SPACE_1))
                            .gap(px(TAB_LABEL_GAP))
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(if selected { TEXT } else { TEXT_SECONDARY }))
                            .font_weight(FontWeight::MEDIUM)
                            .child(icon(route.icon(), TAB_ICON_SIZE))
                            .child(div().flex_1().min_w_0().truncate().child(route.label()))
                            .child(
                                self.button(
                                    ("tab-close", index),
                                    "Close tab",
                                    Control::CloseTab(index),
                                    ui,
                                )
                                .accessibility_id(format!("tabs.{index}.close"))
                                .debug_selector(move || format!("tabs.{index}.close"))
                                .size(px(TAB_CLOSE_SIZE))
                                .flex_shrink_0()
                                .justify_center()
                                .rounded(px(RADIUS_SM))
                                .bg(rgba(SHELL << 8))
                                .opacity(0.)
                                .group_hover(group, |s| s.opacity(1.))
                                .focus(|s| s.opacity(1.).shadow(focus_ring()))
                                .child(icon(Icon::Close, TAB_ICON_SIZE)),
                            ),
                    ),
            )
    }
    /// A bare ghost control with the shared contract and hover fade; callers
    /// compose its children. Labeled buttons use `ui::Button`.
    pub(super) fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        control: Control,
        ui: &mut Ui<Self>,
    ) -> Stateful<Div> {
        let id = id.into();
        let label: SharedString = label.into();
        let spoken: SharedString = label
            .split(" · ")
            .next()
            .unwrap_or(&label)
            .to_owned()
            .into();
        let click_control = control.clone();
        let (progress, on_hover) = ui.hover(&id, true);
        let button = action_button(
            ButtonSpec {
                id,
                label: spoken,
                enabled: true,
            },
            |button| {
                button
                    .gap(px(CONTROL_GAP))
                    .bg(blend(SHELL, HOVER, progress))
                    .on_hover(on_hover)
                    .hover(|s| s.text_color(rgb(TEXT)))
            },
            move |this: &mut Self, window, cx| this.dispatch(click_control.clone(), window, cx),
            ui.cx,
        );
        match &control {
            Control::Sidebar => {
                button
                    .role(accesskit::Role::Switch)
                    .aria_toggled(if self.ui_state.sidebar.open {
                        accesskit::Toggled::True
                    } else {
                        accesskit::Toggled::False
                    })
            }
            _ => button,
        }
    }
    pub(super) fn icon_button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        name: Icon,
        control: Control,
        ui: &mut Ui<Self>,
    ) -> Stateful<Div> {
        self.button(id, label, control, ui)
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
    fn titlebar_space(&self, id: &'static str, ui: &mut Ui<Self>) -> Stateful<Div> {
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
            .on_click(ui.cx.listener(|_this, event: &ClickEvent, window, _| {
                if event.click_count() == 2 {
                    #[cfg(test)]
                    TITLEBAR_ZOOMS.with(|zooms| zooms.set(zooms.get() + 1));
                    window.titlebar_double_click();
                }
            }))
    }

    pub(super) fn header(&self, ui: &mut Ui<Self>) -> impl IntoElement {
        row()
            .w_full()
            .min_w_0()
            .h(px(TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .items_end()
            .pr(px(HEADER_EDGE_INSET))
            .child(
                row()
                    .w(px(self.header_left_width(ui.window)))
                    .h_full()
                    .flex_shrink_0()
                    .justify_end()
                    .pr(px(HEADER_CONTROLS_INSET))
                    .child(self.titlebar_space("titlebar-left-space", ui).flex_1())
                    .child(self.icon_button(
                        "sidebar",
                        shortcuts::TOGGLE_SIDEBAR.labelled("Toggle sidebar"),
                        Icon::Panel,
                        Control::Sidebar,
                        ui,
                    ))
                    .children([false, true].map(|forward| {
                        let name = if forward {
                            Icon::ChevronRight
                        } else {
                            Icon::ChevronLeft
                        };
                        if self.ui_state.can_go(forward) {
                            self.icon_button(
                                if forward { "forward" } else { "back" },
                                if forward { "Forward" } else { "Back" },
                                name,
                                if forward {
                                    Control::Forward
                                } else {
                                    Control::Back
                                },
                                ui,
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
                row()
                    .id("tabs.viewport")
                    .debug_selector(|| "tabs.viewport".into())
                    .relative()
                    .mb(px(-TAB_SLOT_SINK))
                    .w(self.tab_viewport(ui.window))
                    .min_w_0()
                    .flex_shrink_0()
                    .h(px(TAB_SLOT_HEIGHT))
                    .overflow_x_scroll()
                    .overflow_y_hidden()
                    .track_scroll(&self.tab_scroll)
                    .on_scroll_wheel(ui.cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        let delta = event.delta.pixel_delta(px(CONTROL_HEIGHT));
                        // A vertical mouse wheel scrolls the tab strip too. Diagonal
                        // trackpad gestures use the dominant axis, once per event.
                        let delta = if delta.x.abs() > delta.y.abs() {
                            delta.x
                        } else {
                            delta.y
                        };
                        let x = (this.tab_scroll.offset().x + delta)
                            .clamp(-this.tab_scroll.max_offset().x, px(0.));
                        this.tab_scroll.set_offset(point(x, px(0.)));
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .children(
                        self.ui_state
                            .tabs()
                            .iter()
                            .enumerate()
                            .map(|(index, router)| self.tab(index, router.current(), ui)),
                    ),
            )
            .child(
                self.titlebar_space("titlebar-center-space", ui)
                    .w(px(HEADER_CONTROL)),
            )
            .child(div().mb(px(HEADER_EDGE_INSET)).child(self.icon_button(
                "tabs.new",
                shortcuts::NEW_TAB.labelled("New tab"),
                Icon::Plus,
                Control::NewTab,
                ui,
            )))
    }
}

// Zoom requests so far on this thread; the test platform cannot observe the real zoom.
#[cfg(test)]
thread_local! {
    pub(super) static TITLEBAR_ZOOMS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
