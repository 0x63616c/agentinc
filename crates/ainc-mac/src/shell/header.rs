//! The shell header: the current Page tab and its contour, with the header controls.
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
        let route = self.ui_state.current();
        row()
            .h(px(TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .items_end()
            .pr(px(HEADER_EDGE_INSET))
            .child(
                row()
                    .w(px(self.ui_state.sidebar.width + HEADER_LEFT_EXTRA))
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
                    .relative()
                    .mb(px(-TAB_SLOT_SINK))
                    .w(px(TAB_SLOT_WIDTH))
                    .h(px(TAB_SLOT_HEIGHT))
                    .child(
                        row()
                            .absolute()
                            .left(px(TAB_LABEL_INSET))
                            .bottom_0()
                            .w(px(TAB_WIDTH))
                            .h(px(TAB_HEIGHT))
                            .rounded_t(px(TAB_RADIUS))
                            .child(current_page_contour())
                            .child(
                                // The tab's visible face is its top `TAB_FACE_HEIGHT`; centre the label in it.
                                row()
                                    .h_full()
                                    .items_center()
                                    .pb(px(TAB_LABEL_LIFT))
                                    .pl(px(TAB_LABEL_INSET))
                                    .gap(px(TAB_LABEL_GAP))
                                    .text_size(type_size(LABEL_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(icon(route.icon(), TAB_ICON_SIZE))
                                    .child(route.label()),
                            ),
                    ),
            )
            .child(self.titlebar_space("titlebar-center-space", ui).flex_1())
    }
}

// Zoom requests so far on this thread; the test platform cannot observe the real zoom.
#[cfg(test)]
thread_local! {
    pub(super) static TITLEBAR_ZOOMS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
