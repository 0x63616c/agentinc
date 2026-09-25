use super::*;
use crate::ui::*;

impl Shell {
    pub(super) fn main_area(&self, content: AnyElement) -> Div {
        panel()
            .debug_selector(|| "main-pane".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(
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

    pub(super) fn static_page(&self, route: Route, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, detail) = route.empty();
        let mut page = column().gap(px(24.));
        if route != Route::Assistant {
            page = page.child(PageHeader::new(route.label()).build());
        }
        if route == Route::Today {
            for (index, destination) in
                [Route::Tickets, Route::Agents, Route::Calendar, Route::Home]
                    .into_iter()
                    .enumerate()
            {
                let (empty_title, empty_detail) = destination.empty();
                let (title, detail) = if destination == Route::Tickets {
                    (
                        self.tickets.read(cx).summary(),
                        "Open a Ticket to see its Comments and work.".to_owned(),
                    )
                } else if destination == Route::Agents {
                    (
                        self.tickets.read(cx).agent_summary(),
                        "Assign a Ticket to start work.".to_owned(),
                    )
                } else {
                    (empty_title.to_owned(), empty_detail.to_owned())
                };
                if destination == Route::Tickets {
                    page = page.child(
                        column()
                            .gap(px(10.))
                            .child(div().font_weight(FontWeight::MEDIUM).child("Tickets"))
                            .child(
                                div()
                                    .id("today.tickets.summary")
                                    .accessibility_id("today.tickets.summary")
                                    .role(accesskit::Role::Label)
                                    .aria_label(title.clone())
                                    .text_color(rgb(MUTED))
                                    .child(title.clone()),
                            )
                            .children(self.tickets.read(cx).preview().into_iter().map(
                                |(id, title, status)| {
                                    self.button(
                                        ("today.ticket", id as u64),
                                        title.clone(),
                                        Control::Navigate(Route::Tickets),
                                        cx,
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.tickets
                                            .update(cx, |tickets, cx| tickets.select(id, cx));
                                        this.dispatch(
                                            Control::Navigate(Route::Tickets),
                                            window,
                                            cx,
                                        );
                                    }))
                                    .w_full()
                                    .justify_start()
                                    .min_h(px(42.))
                                    .child(title)
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .text_size(type_size(CAPTION_SIZE))
                                            .text_color(rgb(MUTED))
                                            .child(status),
                                    )
                                },
                            )),
                    );
                    continue;
                }
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
                                .border_color(rgb(BORDER_SUBTLE))
                                .child(icon(destination.icon(), 20.))
                                .child(
                                    column().gap(px(5.)).child(title).child(
                                        div()
                                            .text_size(type_size(LABEL_SIZE))
                                            .text_color(rgb(MUTED))
                                            .child(detail),
                                    ),
                                ),
                        ),
                );
            }
        } else if !matches!(route, Route::Settings | Route::Assistant) {
            page = page.child(
                column()
                    .gap(px(12.))
                    .child(icon(route.icon(), 26.))
                    .child(div().font_weight(FontWeight::MEDIUM).child(title))
                    .child(
                        div()
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(MUTED))
                            .child(detail),
                    ),
            );
        }
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
        } else if route == Route::Assistant {
            page = page.child(
                self.assistant
                    .update(cx, |this, cx| this.conversations_view(cx)),
            );
        } else if route.spec().availability == Availability::Planned {
            page = page.child(
                column()
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(MUTED))
                            .child("Not available yet"),
                    )
                    .children(route.planned().iter().map(|(title, detail)| {
                        column()
                            .gap(px(6.))
                            .py(px(16.))
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(*title)
                            .child(
                                div()
                                    .text_size(type_size(LABEL_SIZE))
                                    .text_color(rgb(MUTED))
                                    .child(*detail),
                            )
                    })),
            );
        }
        page
    }
}
