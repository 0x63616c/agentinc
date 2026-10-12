//! The chosen theme and the custom themes saved beside the UI state. A choice
//! is a built-in preset id (`dark`, `paper`) or `custom:<slug>` for a theme
//! file in the `themes` folder, each a JSON object with a `name` and its
//! `colors`. The Settings page edits the choice; the shell saves it.
use crate::ui::{Theme, set_theme, themes};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The prefix a custom theme's choice string carries.
const CUSTOM: &str = "custom:";

/// A theme file: a display name and every color it changes from the default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemeFile {
    pub name: String,
    pub colors: Theme,
}

/// A saved custom theme, as loaded from `<dir>/<slug>.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomTheme {
    pub slug: String,
    pub name: String,
    pub theme: Theme,
}

/// One row of the theme picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeEntry {
    /// The choice string that selects it.
    pub choice: String,
    pub name: String,
    pub group: &'static str,
}

pub struct Themes {
    dir: PathBuf,
    choice: String,
    custom: Vec<CustomTheme>,
}

impl Themes {
    /// Reads every custom theme in `dir` (a missing folder is empty) and
    /// applies `choice`, falling back to the default theme when it names
    /// nothing that exists.
    pub fn load(dir: PathBuf, choice: &str) -> Self {
        let mut custom = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                match read_theme_file(&path) {
                    Ok(file) => custom.push(CustomTheme {
                        slug: slug.to_owned(),
                        name: file.name,
                        theme: file.colors,
                    }),
                    Err(error) => tracing::warn!(%error, path = %path.display(), "skipping theme"),
                }
            }
        }
        custom.sort_by_key(|a| a.name.to_lowercase());
        let mut themes = Self {
            dir,
            choice: String::new(),
            custom,
        };
        themes.choose(choice);
        themes
    }
    /// The saved choice string.
    pub fn choice(&self) -> &str {
        &self.choice
    }
    /// The theme the choice names.
    pub fn current(&self) -> Theme {
        self.resolve(&self.choice).unwrap_or_default()
    }
    /// The custom theme currently chosen, if the choice is one.
    pub fn current_custom(&self) -> Option<&CustomTheme> {
        let slug = self.choice.strip_prefix(CUSTOM)?;
        self.custom.iter().find(|theme| theme.slug == slug)
    }
    fn resolve(&self, choice: &str) -> Option<Theme> {
        if let Some(slug) = choice.strip_prefix(CUSTOM) {
            return self
                .custom
                .iter()
                .find(|theme| theme.slug == slug)
                .map(|theme| theme.theme);
        }
        themes::preset(choice).map(|preset| preset.theme)
    }
    /// Selects and applies `choice`; an unknown choice falls back to the default preset.
    pub fn choose(&mut self, choice: &str) {
        self.choice = if self.resolve(choice).is_some() {
            choice.to_owned()
        } else {
            themes::PRESETS[0].id.to_owned()
        };
        set_theme(self.current());
    }
    /// The picker's rows: dark presets, light presets, then custom themes by name.
    pub fn entries(&self) -> Vec<ThemeEntry> {
        let presets = themes::PRESETS.iter().map(|preset| ThemeEntry {
            choice: preset.id.to_owned(),
            name: preset.name.to_owned(),
            group: if preset.dark { "Dark" } else { "Light" },
        });
        let custom = self.custom.iter().map(|theme| ThemeEntry {
            choice: format!("{CUSTOM}{}", theme.slug),
            name: theme.name.clone(),
            group: "Custom",
        });
        presets.chain(custom).collect()
    }
    /// A name for a new custom theme that no saved one uses yet.
    pub fn suggested_name(&self) -> String {
        let base = self
            .current_custom()
            .map_or("My theme", |theme| theme.name.as_str());
        let base = base.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ');
        (1..)
            .map(|n| {
                if n == 1 {
                    base.to_owned()
                } else {
                    format!("{base} {n}")
                }
            })
            .find(|candidate| !self.custom.iter().any(|t| &t.name == candidate))
            .unwrap_or_else(|| base.to_owned())
    }
    /// Writes `theme` as `name`, replacing a custom theme of the same slug,
    /// then chooses it.
    pub fn save_custom(&mut self, name: &str, theme: Theme) -> io::Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(io::Error::other("Give the theme a name"));
        }
        let slug = slug(name);
        fs::create_dir_all(&self.dir)?;
        write_theme_file(
            &self.dir.join(format!("{slug}.json")),
            &ThemeFile {
                name: name.to_owned(),
                colors: theme,
            },
        )?;
        self.custom.retain(|existing| existing.slug != slug);
        self.custom.push(CustomTheme {
            slug: slug.clone(),
            name: name.to_owned(),
            theme,
        });
        self.custom.sort_by_key(|a| a.name.to_lowercase());
        let choice = format!("{CUSTOM}{slug}");
        self.choose(&choice);
        Ok(choice)
    }
    /// Removes the chosen custom theme's file and returns to the default preset.
    pub fn delete_current(&mut self) -> io::Result<()> {
        let Some(current) = self.current_custom() else {
            return Ok(());
        };
        let slug = current.slug.clone();
        match fs::remove_file(self.dir.join(format!("{slug}.json"))) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.custom.retain(|theme| theme.slug != slug);
        self.choose(themes::PRESETS[0].id);
        Ok(())
    }
    /// Copies a theme file from anywhere into the themes folder and chooses it.
    pub fn import(&mut self, path: &Path) -> io::Result<String> {
        let file = read_theme_file(path)?;
        self.save_custom(&file.name, file.colors)
    }
    /// The file name an export suggests.
    pub fn export_file_name(&self) -> String {
        let name = self
            .current_custom()
            .map(|theme| theme.slug.clone())
            .unwrap_or_else(|| self.choice.clone());
        format!("{name}.json")
    }
    /// Writes the current theme to `path` as a theme file.
    pub fn export(&self, path: &Path) -> io::Result<()> {
        let name = self
            .current_custom()
            .map(|theme| theme.name.clone())
            .or_else(|| themes::preset(&self.choice).map(|preset| preset.name.to_owned()))
            .unwrap_or_else(|| "Theme".to_owned());
        write_theme_file(
            path,
            &ThemeFile {
                name,
                colors: self.current(),
            },
        )
    }
}

fn read_theme_file(path: &Path) -> io::Result<ThemeFile> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|error| io::Error::other(format!("not a theme file: {error}")))
}

fn write_theme_file(path: &Path, file: &ThemeFile) -> io::Result<()> {
    let data = serde_json::to_vec_pretty(file)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, data)?;
    fs::rename(temporary, path)
}

/// A file-safe slug: lowercase ASCII letters and digits with single dashes.
fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "theme".to_owned()
    } else {
        slug.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{theme, theme_test_lock as applied};

    #[test]
    fn slugs_are_file_safe() {
        assert_eq!(slug("Sunset  Orange!"), "sunset-orange");
        assert_eq!(slug("  "), "theme");
        assert_eq!(slug("Ünïcode"), "n-code");
    }

    #[test]
    fn choosing_a_preset_applies_it_and_unknown_choices_fall_back() {
        let _applied = applied();
        let dir = tempfile::tempdir().unwrap();
        let mut themes = Themes::load(dir.path().join("themes"), "paper");
        assert_eq!(themes.choice(), "paper");
        assert_eq!(theme(), themes::PAPER);
        themes.choose("custom:missing");
        assert_eq!(themes.choice(), "dark");
        assert_eq!(theme(), themes::DARK);
    }

    #[test]
    fn custom_themes_save_load_delete_and_round_trip_through_files() {
        let _applied = applied();
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("themes");
        let mut themes = Themes::load(folder.clone(), "dark");
        let mut mine = themes::MIDNIGHT;
        mine.shell = 0x102030;
        let choice = themes.save_custom("Deep Sea", mine).unwrap();
        assert_eq!(choice, "custom:deep-sea");
        assert_eq!(themes.choice(), "custom:deep-sea");
        assert_eq!(theme().shell, 0x102030);
        let text = fs::read_to_string(folder.join("deep-sea.json")).unwrap();
        assert!(text.contains("\"name\": \"Deep Sea\""));
        assert!(text.contains("\"shell\": \"#102030\""));

        let reloaded = Themes::load(folder.clone(), "custom:deep-sea");
        assert_eq!(reloaded.current(), mine);
        let entries = reloaded.entries();
        assert_eq!(entries.len(), themes::PRESETS.len() + 1);
        assert_eq!(entries.last().unwrap().name, "Deep Sea");
        assert_eq!(entries.last().unwrap().group, "Custom");
        assert_eq!(reloaded.suggested_name(), "Deep Sea 2");

        let exported = dir.path().join("out.json");
        reloaded.export(&exported).unwrap();
        let mut other = Themes::load(dir.path().join("other"), "dark");
        assert_eq!(other.import(&exported).unwrap(), "custom:deep-sea");
        assert_eq!(other.current(), mine);
        assert!(other.import(&dir.path().join("nope.json")).is_err());

        let mut themes = Themes::load(folder.clone(), "custom:deep-sea");
        themes.delete_current().unwrap();
        assert!(!folder.join("deep-sea.json").exists());
        assert_eq!(themes.choice(), "dark");
        assert!(themes.entries().iter().all(|entry| entry.group != "Custom"));
        set_theme(Theme::default());
    }

    #[test]
    fn a_broken_theme_file_is_skipped() {
        let _applied = applied();
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("themes");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("broken.json"), "{").unwrap();
        fs::write(
            folder.join("ok.json"),
            r##"{"name":"Ok","colors":{"shell":"#111111"}}"##,
        )
        .unwrap();
        let themes = Themes::load(folder, "custom:ok");
        assert_eq!(themes.current().shell, 0x111111);
        assert_eq!(themes.current().text, themes::DARK.text);
        assert_eq!(
            themes
                .entries()
                .iter()
                .filter(|e| e.group == "Custom")
                .count(),
            1
        );
        set_theme(Theme::default());
    }
}
