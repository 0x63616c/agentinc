//! Settings: appearance, software updates, a pointer to Connections and the
//! keyboard shortcuts. Appearance changes are announced to the shell, which
//! applies and saves them.
use crate::{
    page::Page,
    routes::{Destination, Route},
    ui::*,
    ui_state::{Appearance, FontChoice, FontSize},
};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

pub struct SettingsPage {
    /// Shared with the shell, which applies and saves it.
    appearance: Rc<Cell<Appearance>>,
}
impl EventEmitter<Destination> for SettingsPage {}
/// The person changed a type preference on this page.
impl EventEmitter<Appearance> for SettingsPage {}

impl SettingsPage {
    pub fn new(appearance: Rc<Cell<Appearance>>, _: &mut Context<Self>) -> Self {
        Self { appearance }
    }
    fn change(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.appearance.set(appearance);
        cx.emit(appearance);
        cx.notify();
    }
    fn appearance_section(&self, ui: &mut Ui<Self>) -> Div {
        let current = self.appearance.get();
        let font = match current.font {
            FontChoice::System => 0,
            FontChoice::HelveticaNeue => 1,
        };
        let size = FontSize::ALL
            .iter()
            .position(|(size, _)| *size == current.font_size)
            .unwrap_or(1);
        settings_section(
            "Appearance",
            column()
                .child(settings_row(
                    "Font",
                    "The typeface used throughout AgentInc.",
                    Segmented::new("font", ["System · SF Pro", "Helvetica Neue"])
                        .selected(font)
                        .build(ui, |this: &mut Self, index, _, cx| {
                            let font = if index == 0 {
                                FontChoice::System
                            } else {
                                FontChoice::HelveticaNeue
                            };
                            let appearance = Appearance {
                                font,
                                ..this.appearance.get()
                            };
                            this.change(appearance, cx)
                        }),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Font size",
                    "The scale of text across the app.",
                    Segmented::new("font-size", FontSize::ALL.iter().map(|(_, label)| *label))
                        .selected(size)
                        .build(ui, |this: &mut Self, index, _, cx| {
                            let appearance = Appearance {
                                font_size: FontSize::ALL[index].0,
                                ..this.appearance.get()
                            };
                            this.change(appearance, cx)
                        }),
                )),
        )
    }
    fn connections_section(&self, ui: &mut Ui<Self>) -> Div {
        settings_section(
            "Connections",
            settings_row(
                "Connections",
                "The ChatGPT account Evee replies through, and its model.",
                Button::new("settings.connections", "Open Connections")
                    .secondary()
                    .trailing(icon(Icon::ArrowRight, ICON_SIZE_SM))
                    .build(ui, |_: &mut Self, _, cx| {
                        cx.emit(Destination::Page(Route::Connections))
                    }),
            ),
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
}

impl Page for SettingsPage {
    const ROUTE: Route = Route::Settings;
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let updates = cx.try_global::<crate::updates::Updates>().cloned();
        let mut ui = Ui::new(window, cx);
        PageFrame::document(
            PageHeader::new(self.title())
                .description("Appearance, updates, connections and shortcuts."),
        )
        .child(
            column()
                .gap(px(SECTION_GAP))
                .child(self.appearance_section(&mut ui))
                .when_some(updates, |view, updates| {
                    view.child(
                        updates
                            .0
                            .update(ui.cx, |this, cx| this.settings(ui.window, cx)),
                    )
                })
                .child(self.connections_section(&mut ui))
                .child(self.shortcuts_section()),
        )
        .build()
    }
}

#[cfg(test)]
mod tests {
    use crate::{routes::Route, shell::Shell, ui::shortcuts};
    use gpui::{TestAppContext, px};

    #[gpui::test]
    fn settings_shortcuts_each_use_one_complete_pill(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        shell.update(cx, |shell, cx| shell.fixture_navigate(Route::Settings, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            shortcuts::ALL.iter().map(|s| s.glyph).collect::<Vec<_>>(),
            [
                "⌘K", "⌘[", "⌘]", "⌘T", "⇧⌘W", "⇧⌘[", "⇧⌘]", "⇧⌘U", "⌘1–6", "⌘,", "⌘B", "⎋", "↵",
                "⇧↵"
            ]
        );
        for shortcut in shortcuts::ALL {
            let pill = cx.debug_bounds(shortcut.glyph).unwrap();
            assert_eq!(pill.size.height, px(20.));
            assert!(pill.size.width >= px(20.));
        }
    }
}
