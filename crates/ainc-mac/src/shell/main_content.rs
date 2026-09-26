use super::*;
use crate::ui::*;

impl Shell {
    pub(super) fn main_area(&self, content: AnyElement) -> Div {
        let terminal = self.session.current() == Route::Terminal;
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
                    .when(terminal, |pane| pane.p(px(8.)))
                    .child(content),
            )
            .child(status_bar())
    }

    pub(super) fn terminal_page(&self, _visible: bool) -> impl IntoElement {
        let page = column().size_full().min_h_0();
        #[cfg(target_os = "macos")]
        if let Some(host) = &self.terminal {
            return Page::canvas()
                .child(
                    page.child(div().flex_1().min_h_0().w_full().child(terminal_surface(
                        host.clone(),
                        _visible,
                        self.pending_terminal_focus,
                    ))),
                )
                .build()
                .into_any_element();
        }
        #[cfg(target_os = "macos")]
        let message = self
            .terminal_error
            .clone()
            .unwrap_or_else(|| "Ghostty could not start.".into());
        #[cfg(not(target_os = "macos"))]
        let message = "Terminal requires macOS and the Ghostty runtime.";
        Page::canvas()
            .child(page.child(div().text_color(rgb(MUTED)).child(message)))
            .build()
            .into_any_element()
    }

    pub(super) fn static_page(&self, route: Route, cx: &mut Context<Self>) -> impl IntoElement {
        let mut page = Page::document(PageHeader::new(route.label()));
        if route == Route::Settings {
            page = page.child(
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
                        "Providers",
                        self.providers_settings
                            .update(cx, |this, cx| this.providers_view(cx)),
                    ))
                    .child(settings_section(
                        "Agent tools",
                        self.providers_settings
                            .update(cx, |this, cx| this.agent_view(cx)),
                    )),
            );
        }
        page.build()
    }
}
