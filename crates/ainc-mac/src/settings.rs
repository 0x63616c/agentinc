//! Settings: appearance, theme, software updates and the keyboard shortcuts.
//! Appearance and theme changes are announced to the shell, which applies
//! and saves them.
use crate::{
    overlay::Overlay,
    page::{Page, PageOverlays},
    routes::{Destination, Route},
    theme::Themes,
    ui::*,
    ui_state::{Appearance, FontChoice, FontSize},
};
use gpui::{prelude::*, *};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

mod theme;
use theme::ThemeEditor;
pub use theme::ThemeEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
    Theme,
}

pub struct SettingsPage {
    /// Shared with the shell, which applies and saves it.
    appearance: Rc<Cell<Appearance>>,
    overlays: PageOverlays<Dialog>,
    theme_editor: ThemeEditor,
}
impl EventEmitter<Destination> for SettingsPage {}
/// The person changed a type preference on this page.
impl EventEmitter<Appearance> for SettingsPage {}

impl SettingsPage {
    pub fn new(
        appearance: Rc<Cell<Appearance>>,
        themes: Rc<RefCell<Themes>>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            appearance,
            overlays: PageOverlays::new(overlays, Route::Settings),
            theme_editor: ThemeEditor::new(themes, cx),
        }
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
                    "System pairs with SF Mono; Helvetica Neue pairs with Menlo for identifiers.",
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
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.overlays.active()?;
        Some(self.theme_sheet(window, cx))
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let mut handles: Vec<FocusHandle> = self
            .theme_editor
            .inputs()
            .map(|input| input.focus_handle(cx))
            .collect();
        handles.push(self.theme_editor.cancel_focus.clone());
        handles.push(self.theme_editor.submit_focus.clone());
        handles
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let updates = cx.try_global::<crate::updates::Updates>().cloned();
        let mut ui = Ui::new(window, cx);
        PageFrame::document(PageHeader::new(self.title()))
            .child(
                column()
                    .gap(px(SECTION_GAP))
                    .child(self.appearance_section(&mut ui))
                    .child(self.theme_section(&mut ui))
                    .when_some(updates, |view, updates| {
                        view.child(
                            updates
                                .0
                                .update(ui.cx, |this, cx| this.settings(ui.window, cx)),
                        )
                    })
                    .child(self.shortcuts_section()),
            )
            .build()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        routes::Route,
        shell::Shell,
        ui::{shortcuts, theme, theme_test_lock, themes},
        ui_state::UiState,
    };
    use gpui::{TestAppContext, px};

    #[gpui::test]
    fn choosing_a_theme_applies_it_and_relaunch_restores_it(cx: &mut TestAppContext) {
        let _applied = theme_test_lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::fixture(path.clone(), window, cx));
        shell.update(cx, |shell, cx| shell.fixture_navigate(Route::Settings, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let settings = shell.read_with(cx, |shell, _| shell.fixture_settings());
        settings.update(cx, |settings, cx| settings.choose_theme("paper", cx));
        cx.run_until_parked();
        assert_eq!(theme(), themes::PAPER);
        assert_eq!(UiState::load(&path).theme, "paper");

        let (_, cx) = cx.add_window_view(|window, cx| Shell::fixture(path.clone(), window, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(theme(), themes::PAPER);
        settings.update(cx, |settings, cx| settings.choose_theme("dark", cx));
        cx.run_until_parked();
        assert_eq!(theme(), themes::DARK);
    }

    #[gpui::test]
    fn the_theme_editor_previews_reverts_and_saves_a_custom_theme(cx: &mut TestAppContext) {
        let _applied = theme_test_lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::fixture(path.clone(), window, cx));
        shell.update(cx, |shell, cx| shell.fixture_navigate(Route::Settings, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let settings = shell.read_with(cx, |shell, _| shell.fixture_settings());
        cx.update(|window, cx| {
            settings.update(cx, |settings, cx| settings.open_theme_editor(window, cx))
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let name = settings.read_with(cx, |settings, cx| {
            settings.theme_editor.name.read(cx).content.to_string()
        });
        assert_eq!(name, "My theme");
        // Typing a color previews it across the app before anything is saved.
        assert!(cx.debug_bounds("settings.theme.shell").is_some());
        settings.update(cx, |settings, cx| {
            settings
                .theme_editor
                .slot_input("shell")
                .update(cx, |input, cx| input.set_text("#102030", cx));
        });
        cx.run_until_parked();
        assert_eq!(theme().shell, 0x102030);
        assert_eq!(theme().text, themes::DARK.text);
        // Cancel restores the chosen theme.
        let cancel = cx.debug_bounds("settings.theme.cancel").unwrap().center();
        cx.simulate_click(cancel, gpui::Modifiers::default());
        cx.run_until_parked();
        assert_eq!(theme(), themes::DARK);
        // Save writes a theme file and chooses it.
        cx.update(|window, cx| {
            settings.update(cx, |settings, cx| settings.open_theme_editor(window, cx))
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        settings.update(cx, |settings, cx| {
            settings
                .theme_editor
                .slot_input("shell")
                .update(cx, |input, cx| input.set_text("#102030", cx));
            settings
                .theme_editor
                .name
                .update(cx, |input, cx| input.set_text("Deep Sea", cx));
            settings.save_theme(cx);
        });
        cx.run_until_parked();
        assert_eq!(theme().shell, 0x102030);
        assert_eq!(UiState::load(&path).theme, "custom:deep-sea");
        assert!(dir.path().join("themes/deep-sea.json").exists());
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.fixture_state().1, None, "the sheet closed on save")
        });
        settings.update(cx, |settings, cx| settings.choose_theme("dark", cx));
        cx.run_until_parked();
    }

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
                "⌘K", "⌘[", "⌘]", "⌘T", "⌘W", "⇧⌘[", "⇧⌘]", "⇧⌘U", "⌘1–4", "⌘,", "⌘B", "⎋", "↵",
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
