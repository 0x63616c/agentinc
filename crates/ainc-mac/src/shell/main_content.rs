use super::*;
use crate::ui::*;

impl Shell {
    pub(super) fn main_area(&self, content: AnyElement, assistant: bool) -> Div {
        let area = panel()
            .debug_selector(|| "main-pane".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden();
        if assistant {
            area.child(content)
        } else {
            area.child(
                column()
                    .id("page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(PAGE_X))
                    .py(px(PAGE_X))
                    .child(
                        div()
                            .relative()
                            .debug_selector(|| "main-content".into())
                            .child(content),
                    ),
            )
        }
    }

    pub(super) fn static_page(&self, route: Route, cx: &mut Context<Self>) -> impl IntoElement {
        let mut page = column()
            .gap(px(24.))
            .child(PageHeader::new(route.label()).build());
        if route == Route::Settings {
            page = page.w_full().max_w(px(760.)).child(
                column()
                    .gap(px(18.))
                    .child(settings_section(
                        "Appearance",
                        column()
                            .child(settings_row(
                                "Font",
                                "Choose the typeface used throughout AgentInc.",
                                settings_segments([
                                    settings_segment(
                                        "font-system",
                                        "System · SF Pro",
                                        self.session.font == FontChoice::System,
                                        true,
                                        |this: &mut Self, window, cx| {
                                            this.dispatch(
                                                Control::Font(FontChoice::System),
                                                window,
                                                cx,
                                            )
                                        },
                                        cx,
                                    ),
                                    settings_segment(
                                        "font-helvetica",
                                        "Helvetica Neue",
                                        self.session.font == FontChoice::HelveticaNeue,
                                        true,
                                        |this: &mut Self, window, cx| {
                                            this.dispatch(
                                                Control::Font(FontChoice::HelveticaNeue),
                                                window,
                                                cx,
                                            )
                                        },
                                        cx,
                                    ),
                                ]),
                            ))
                            .child(settings_divider())
                            .child(settings_row(
                                "Font size",
                                "Set the scale of text across the app.",
                                settings_segments(FontSize::ALL.into_iter().enumerate().map(
                                    |(index, (size, label))| {
                                        settings_segment(
                                            ("font-size", index),
                                            label,
                                            self.session.font_size == size,
                                            true,
                                            move |this: &mut Self, window, cx| {
                                                this.dispatch(Control::FontSize(size), window, cx)
                                            },
                                            cx,
                                        )
                                    },
                                )),
                            )),
                    ))
                    .when_some(
                        cx.try_global::<crate::updates::Updates>().cloned(),
                        |view, updates| {
                            view.child(updates.0.update(cx, |this, cx| this.settings(cx)))
                        },
                    )
                    .child(settings_section(
                        "Accounts & connections",
                        self.assistant.update(cx, |this, cx| this.settings_view(cx)),
                    )),
            );
        }
        page
    }
}
