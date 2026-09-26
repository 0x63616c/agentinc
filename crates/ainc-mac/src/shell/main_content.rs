use super::*;
use crate::ui::*;

const SHORTCUTS: &[(&str, &str)] = &[
    ("Go to…", "⌘ K"),
    ("Toggle sidebar", "⌘ B"),
    ("Back / Forward", "⌘ ⌥ ← →"),
    ("Tickets, Assistant, Agents…", "⌘1–6"),
    ("Settings", "⌘ ,"),
    ("Dismiss", "esc"),
];

impl Shell {
    pub(super) fn main_area(&self, content: AnyElement) -> Div {
        let terminal = self.session.current() == Route::Terminal;
        let route = self.session.current();
        let shortcut = PAGES
            .iter()
            .filter(|page| page.in_sidebar)
            .position(|page| page.route == route)
            .map_or_else(
                || (route == Route::Settings).then(|| "⌘,".to_owned()),
                |index| Some(format!("⌘{}", index + 1)),
            );
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
                status_bar()
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .debug_selector(|| "status-bar.route".into())
                            .child(icon(route.icon(), 12.))
                            .child(route.label())
                            .when_some(shortcut, |s, shortcut| s.child(kbd(shortcut))),
                    )
                    .child(
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
            .child(
                page.p(px(PAGE_X)).child(
                    EmptyState::new("terminal", "Terminal unavailable")
                        .description(message)
                        .selector("terminal.unavailable")
                        .build(),
                ),
            )
            .build()
            .into_any_element()
    }

    fn appearance_section(&self, cx: &mut Context<Self>) -> Div {
        let font = match self.session.font {
            FontChoice::System => 0,
            FontChoice::HelveticaNeue => 1,
        };
        let size = FontSize::ALL
            .iter()
            .position(|(size, _)| *size == self.session.font_size)
            .unwrap_or(1);
        settings_section(
            "Appearance",
            column()
                .child(settings_row(
                    "Font",
                    "The typeface used throughout AgentInc.",
                    segmented(
                        "font",
                        ["System · SF Pro", "Helvetica Neue"],
                        font,
                        true,
                        &self.hover,
                        |this: &mut Self, index, window, cx| {
                            let font = if index == 0 {
                                FontChoice::System
                            } else {
                                FontChoice::HelveticaNeue
                            };
                            this.dispatch(Control::Font(font), window, cx)
                        },
                        cx,
                    ),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Font size",
                    "The scale of text across the app.",
                    segmented(
                        "font-size",
                        FontSize::ALL.iter().map(|(_, label)| *label),
                        size,
                        true,
                        &self.hover,
                        |this: &mut Self, index, window, cx| {
                            this.dispatch(Control::FontSize(FontSize::ALL[index].0), window, cx)
                        },
                        cx,
                    ),
                )),
        )
    }

    fn shortcuts_section(&self) -> Div {
        let mut rows = column();
        for (index, (action, keys)) in SHORTCUTS.iter().enumerate() {
            if index > 0 {
                rows = rows.child(settings_divider());
            }
            rows = rows.child(
                row()
                    .min_h(px(LIST_ROW_HEIGHT))
                    .px(px(SETTINGS_INSET))
                    .justify_between()
                    .gap(px(SPACE_4))
                    .child(div().text_size(type_size(BODY_SIZE)).child(*action))
                    .child(
                        row()
                            .gap(px(SPACE_1))
                            .children(keys.split(' ').map(|key| kbd(key.to_owned()))),
                    ),
            );
        }
        settings_section("Keyboard shortcuts", rows)
    }

    pub(super) fn static_page(
        &self,
        route: Route,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut page = Page::document(
            PageHeader::new(route.label())
                .description("Appearance, updates, connections and shortcuts."),
        );
        if route == Route::Settings {
            page = page.child(
                column()
                    .gap(px(SECTION_GAP))
                    .child(self.appearance_section(cx))
                    .when_some(
                        cx.try_global::<crate::updates::Updates>().cloned(),
                        |view, updates| {
                            view.child(updates.0.update(cx, |this, cx| this.settings(window, cx)))
                        },
                    )
                    .child(settings_section(
                        "Accounts & connections",
                        self.assistant
                            .update(cx, |this, cx| this.settings_view(window, cx)),
                    ))
                    .child(self.shortcuts_section()),
            );
        }
        page.build()
    }
}
