//! The main content card, the Terminal and static pages, and the Settings shortcuts list.
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
                    .when(terminal, |pane| pane.p(px(SPACE_2)))
                    .child(content),
            )
            .child(
                status_bar().child(
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
        for (index, shortcut) in shortcuts::ALL.iter().enumerate() {
            if index > 0 {
                rows = rows.child(settings_divider());
            }
            rows = rows.child(
                row()
                    .min_h(px(LIST_ROW_HEIGHT))
                    .px(px(SETTINGS_INSET))
                    .justify_between()
                    .gap(px(SPACE_4))
                    .child(div().text_size(type_size(BODY_SIZE)).child(shortcut.action))
                    .child(kbd(shortcut.glyph).debug_selector(move || shortcut.glyph.to_string())),
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

#[cfg(test)]
mod tests {
    use super::Shell;
    use crate::model::Route;
    use crate::ui::shortcuts;
    use gpui::{TestAppContext, px};

    #[gpui::test]
    fn settings_shortcuts_each_use_one_complete_pill(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        shell.update(cx, |shell, cx| {
            shell.session.navigate(Route::Settings);
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            shortcuts::ALL.iter().map(|s| s.glyph).collect::<Vec<_>>(),
            ["⌘K", "⌘[", "⌘]", "⌘1–6", "⌘,", "⌘B", "⎋", "↵", "⇧↵"]
        );
        for shortcut in shortcuts::ALL {
            let pill = cx.debug_bounds(shortcut.glyph).unwrap();
            assert_eq!(pill.size.height, px(20.));
            assert!(pill.size.width >= px(20.));
        }
    }
}
