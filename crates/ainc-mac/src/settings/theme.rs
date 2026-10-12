//! The Theme section of Settings: the picker over built-in and custom themes,
//! the editor sheet that previews every color live and saves a custom theme,
//! and the import, export and delete actions over theme files.
use super::SettingsPage;
use crate::{
    input::TextInput,
    theme::Themes,
    ui::{ColorForm, Theme, ThemeSlot, format_hex, parse_hex, set_theme, *},
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

/// What the person did to the theme; the shell repaints, and saves on `Changed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeEvent {
    /// The editor changed a color; the whole window should repaint.
    Preview,
    /// A different theme is chosen (or saved); persist the choice.
    Changed,
}
impl EventEmitter<ThemeEvent> for SettingsPage {}

const PICKER: &str = "settings.theme";

/// The theme editor's state, owned by the Settings page.
pub struct ThemeEditor {
    pub themes: Rc<RefCell<Themes>>,
    pub name: Entity<TextInput>,
    slots: Vec<(ThemeSlot, Entity<TextInput>)>,
    draft: Theme,
    /// A failed save, import or delete.
    error: Option<String>,
    pub cancel_focus: FocusHandle,
    pub submit_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ThemeEditor {
    pub fn new(themes: Rc<RefCell<Themes>>, cx: &mut Context<SettingsPage>) -> Self {
        let name =
            cx.new(|cx| TextInput::field("Name", false, cx).identified("settings.theme.name"));
        let mut subscriptions = vec![cx.observe(&name, |_, _, cx| cx.notify())];
        let slots: Vec<_> = Theme::slots()
            .into_iter()
            .map(|slot| {
                let input = cx.new(|cx| TextInput::field("Hex color", false, cx));
                subscriptions.push(cx.observe(&input, move |this, input, cx| {
                    let text = input.read(cx).content.clone();
                    this.theme_editor.edited(slot, &text, cx);
                }));
                (slot, input)
            })
            .collect();
        let draft = themes.borrow().current();
        Self {
            themes,
            name,
            slots,
            draft,
            error: None,
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }
    /// A slot's field changed: preview the color when it parses.
    fn edited(&mut self, slot: ThemeSlot, text: &str, cx: &mut Context<SettingsPage>) {
        let Some(value) = parse_hex(text, slot.form) else {
            return;
        };
        if (slot.get)(&self.draft) == value {
            return;
        }
        (slot.set)(&mut self.draft, value);
        set_theme(self.draft);
        cx.emit(ThemeEvent::Preview);
        cx.notify();
    }
    #[cfg(test)]
    pub fn slot_input(&self, key: &str) -> &Entity<TextInput> {
        &self
            .slots
            .iter()
            .find(|(slot, _)| slot.key == key)
            .unwrap()
            .1
    }

    pub fn inputs(&self) -> impl Iterator<Item = &Entity<TextInput>> {
        std::iter::once(&self.name).chain(self.slots.iter().map(|(_, input)| input))
    }
    /// Start editing a copy of the current theme.
    pub fn open(&mut self, cx: &mut Context<SettingsPage>) {
        let themes = self.themes.borrow();
        self.draft = themes.current();
        let name = themes.suggested_name();
        drop(themes);
        self.error = None;
        self.name.update(cx, |input, cx| input.set_text(&name, cx));
        for (slot, input) in &self.slots {
            let text = format_hex((slot.get)(&self.draft), slot.form);
            input.update(cx, |input, cx| input.set_text(&text, cx));
        }
    }
    /// Drop the draft and show the chosen theme again.
    pub fn revert(&mut self, cx: &mut Context<SettingsPage>) {
        let current = self.themes.borrow().current();
        if self.draft != current {
            self.draft = current;
            set_theme(current);
            cx.emit(ThemeEvent::Preview);
        }
    }
    /// Every field holds a color and the name is not blank.
    fn valid(&self, cx: &App) -> bool {
        !self.name.read(cx).content.trim().is_empty()
            && self
                .slots
                .iter()
                .all(|(slot, input)| parse_hex(&input.read(cx).content, slot.form).is_some())
    }
    /// Write the draft as a custom theme and choose it. Ok when it saved.
    pub fn save(&mut self, cx: &mut Context<SettingsPage>) -> bool {
        let name = self.name.read(cx).content.trim().to_owned();
        let result = self.themes.borrow_mut().save_custom(&name, self.draft);
        match result {
            Ok(_) => {
                self.error = None;
                cx.emit(ThemeEvent::Changed);
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }
    pub fn choose(&mut self, choice: &str, cx: &mut Context<SettingsPage>) {
        self.themes.borrow_mut().choose(choice);
        self.draft = self.themes.borrow().current();
        self.error = None;
        cx.emit(ThemeEvent::Changed);
        cx.notify();
    }
    pub fn delete(&mut self, cx: &mut Context<SettingsPage>) {
        let result = self.themes.borrow_mut().delete_current();
        match result {
            Ok(()) => {
                self.error = None;
                cx.emit(ThemeEvent::Changed);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub fn import_file(&mut self, path: &std::path::Path, cx: &mut Context<SettingsPage>) {
        let result = self.themes.borrow_mut().import(path);
        match result {
            Ok(_) => {
                self.error = None;
                cx.emit(ThemeEvent::Changed);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub fn export_file(&mut self, path: &std::path::Path, cx: &mut Context<SettingsPage>) {
        self.error = self
            .themes
            .borrow()
            .export(path)
            .err()
            .map(|e| e.to_string());
        cx.notify();
    }
    /// Ask for a theme file and import it.
    pub fn import(&mut self, cx: &mut Context<SettingsPage>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.theme_editor.import_file(&path, cx));
        })
        .detach();
    }
    /// Ask where to write the current theme and export it there.
    pub fn export(&mut self, cx: &mut Context<SettingsPage>) {
        let directory = std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let file_name = self.themes.borrow().export_file_name();
        let receiver = cx.prompt_for_new_path(&directory, Some(&file_name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = receiver.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.theme_editor.export_file(&path, cx));
        })
        .detach();
    }
}

impl SettingsPage {
    pub fn choose_theme(&mut self, choice: &str, cx: &mut Context<Self>) {
        self.theme_editor.choose(choice, cx);
    }
    pub fn open_theme_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_editor.open(cx);
        let focus = self.theme_editor.name.focus_handle(cx);
        self.overlays
            .open_dialog(super::Dialog::Theme, focus, window, cx);
        cx.notify();
    }
    fn close_theme_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_editor.revert(cx);
        self.overlays.dismiss(window, cx);
        cx.notify();
    }
    pub fn save_theme(&mut self, cx: &mut Context<Self>) {
        if self.theme_editor.save(cx) {
            self.overlays.close();
        }
        cx.notify();
    }
    pub(super) fn theme_section(&self, ui: &mut Ui<Self>) -> Div {
        let themes = self.theme_editor.themes.borrow();
        let entries = themes.entries();
        let current = entries.iter().position(|e| e.choice == themes.choice());
        let custom = themes.current_custom().is_some();
        drop(themes);
        let choices: Vec<String> = entries.iter().map(|e| e.choice.clone()).collect();
        let picker = Select::new(
            PICKER,
            entries
                .iter()
                .map(|entry| SelectOption::new(entry.name.clone()).description(entry.group))
                .collect(),
        )
        .value(current)
        .open(self.overlays.popover_open(PICKER))
        .width(SELECT_WIDTH)
        .build(
            ui,
            |this: &mut Self, _, cx| {
                this.overlays.toggle_popover(PICKER);
                cx.notify();
            },
            move |this: &mut Self, index, _, cx| {
                this.overlays.close_popover();
                if let Some(choice) = choices.get(index) {
                    this.choose_theme(choice, cx);
                }
            },
        );
        let error = self.theme_editor.error.clone();
        settings_section(
            "Theme",
            column()
                .child(settings_row(
                    "Theme",
                    "Built-in dark and light themes, or one of your own.",
                    row().gap(px(SPACE_2)).child(picker).child(
                        Button::new("settings.theme.customise", "Customise")
                            .secondary()
                            .build(ui, |this: &mut Self, window, cx| {
                                this.open_theme_editor(window, cx)
                            }),
                    ),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Theme files",
                    "Load a theme from a file, or save the current one to share.",
                    column()
                        .items_end()
                        .gap(px(SPACE_2))
                        .child(
                            row()
                                .gap(px(SPACE_2))
                                .child(
                                    Button::new("settings.theme.import", "Import")
                                        .secondary()
                                        .build(ui, |this: &mut Self, _, cx| {
                                            this.theme_editor.import(cx)
                                        }),
                                )
                                .child(
                                    Button::new("settings.theme.export", "Export")
                                        .secondary()
                                        .build(ui, |this: &mut Self, _, cx| {
                                            this.theme_editor.export(cx)
                                        }),
                                )
                                .when(custom, |s| {
                                    s.child(
                                        Button::new("settings.theme.delete", "Delete")
                                            .destructive()
                                            .build(ui, |this: &mut Self, _, cx| {
                                                this.theme_editor.delete(cx)
                                            }),
                                    )
                                }),
                        )
                        .when_some(error, |s, error| s.child(error_text(error))),
                )),
        )
    }
    pub(super) fn theme_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ui = &mut Ui::new(window, cx);
        let mut body = column_gap(FORM_STACK_GAP).child(
            Field::new(self.theme_editor.name.clone())
                .label("Name")
                .selector("settings.theme.name")
                .build(ui),
        );
        let mut group = "";
        let mut rows = column_gap(SPACE_2);
        for (slot, input) in &self.theme_editor.slots {
            if slot.group != group {
                if !group.is_empty() {
                    body = body.child(rows);
                    rows = column_gap(SPACE_2);
                }
                group = slot.group;
                rows = rows.child(eyebrow(group));
            }
            let text = input.read(ui.cx).content.clone();
            let value = parse_hex(&text, slot.form);
            let swatch = div()
                .size(px(THEME_SWATCH_SIZE))
                .flex_shrink_0()
                .rounded(px(RADIUS_SM))
                .border_1()
                .border_color(rgb(theme().border))
                .when_some(value, |s, value| match slot.form {
                    ColorForm::Rgb => s.bg(rgb(value)),
                    ColorForm::Rgba => s.bg(rgba(value)),
                });
            rows = rows.child(
                row()
                    .items_center()
                    .gap(px(SPACE_3))
                    .child(swatch)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(type_size(BODY_SIZE))
                            .child(slot.label),
                    )
                    .child(
                        div()
                            .w(px(THEME_HEX_WIDTH))
                            .debug_selector(move || format!("settings.theme.{}", slot.key))
                            .child(
                                Field::new(input.clone())
                                    .error(value.is_none().then_some("Use #rrggbb"))
                                    .build(ui),
                            ),
                    ),
            );
        }
        body = body.child(rows);
        if let Some(error) = self.theme_editor.error.clone() {
            body = body.child(error_text(error));
        }
        let footer = DialogFooter::new(Verb::Save)
            .ids("settings.theme.cancel", "settings.theme.save")
            .enabled(self.theme_editor.valid(ui.cx))
            .focus(
                &self.theme_editor.cancel_focus,
                &self.theme_editor.submit_focus,
            )
            .build(
                ui,
                |this: &mut Self, window, cx| this.close_theme_editor(window, cx),
                |this: &mut Self, _, cx| this.save_theme(cx),
            );
        // The shell centres dialogs; a sheet pins itself to the window's right edge.
        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .child(sheet_shell("Customise theme", body, footer))
            .into_any_element()
    }
}
