//! The persisted UI state: route history, the sidebar, type preferences and
//! recent palette choices, plus recovery of the legacy session files. Saved
//! as `session.json`; the word Session itself belongs to the SDK.
use crate::routes::{Route, Router};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

/// The sidebar's minimum, maximum and default width.
pub const SIDEBAR_MIN: f32 = 150.;
pub const SIDEBAR_MAX: f32 = 320.;
pub const SIDEBAR_DEFAULT: f32 = 216.;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanePreference {
    pub open: bool,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontChoice {
    #[default]
    System,
    HelveticaNeue,
}
impl FontChoice {
    pub fn family(self) -> &'static str {
        match self {
            Self::System => ".AppleSystemUIFont",
            Self::HelveticaNeue => "Helvetica Neue",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontSize {
    Small,
    #[default]
    Default,
    Large,
    Larger,
}
impl FontSize {
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Small, "Small"),
        (Self::Default, "Default"),
        (Self::Large, "Large"),
        (Self::Larger, "Larger"),
    ];

    pub fn scale(self) -> f32 {
        match self {
            Self::Small => 0.9,
            Self::Default => 1.,
            Self::Large => 1.1,
            Self::Larger => 1.2,
        }
    }
}

/// The type preferences the Settings page edits and the shell applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub font: FontChoice,
    pub font_size: FontSize,
}

// The saved file keeps its `panes` array from when there were two panes.
mod panes {
    use super::PanePreference;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(pane: &PanePreference, s: S) -> Result<S::Ok, S::Error> {
        [*pane].serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PanePreference, D::Error> {
        let panes: Vec<PanePreference> = Vec::deserialize(d)?;
        Ok(panes
            .into_iter()
            .next()
            .unwrap_or(super::UiState::default().sidebar))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    pub schema_version: u32,
    router: Router,
    #[serde(rename = "panes", with = "panes")]
    pub sidebar: PanePreference,
    pub font: FontChoice,
    pub font_size: FontSize,
    /// Command palette entries the user chose most recently, newest first.
    pub recent_commands: Vec<String>,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            router: Router::default(),
            sidebar: PanePreference {
                open: true,
                width: SIDEBAR_DEFAULT,
            },
            font: FontChoice::System,
            font_size: FontSize::Default,
            recent_commands: Vec::new(),
        }
    }
}
/// How many palette choices are remembered.
pub const RECENT_COMMANDS: usize = 5;
impl UiState {
    /// Moves `id` to the front of the recent palette choices.
    pub fn remember_command(&mut self, id: &str) {
        self.recent_commands.retain(|recent| recent != id);
        self.recent_commands.insert(0, id.to_owned());
        self.recent_commands.truncate(RECENT_COMMANDS);
    }
    pub fn appearance(&self) -> Appearance {
        Appearance {
            font: self.font,
            font_size: self.font_size,
        }
    }
    pub fn current(&self) -> Route {
        self.router.current()
    }
    pub fn navigate(&mut self, route: Route) {
        self.router.navigate(route);
    }
    pub fn can_go(&self, forward: bool) -> bool {
        self.router.can_go(forward)
    }
    pub fn go(&mut self, forward: bool) {
        self.router.go(forward);
    }
    pub fn from_json(json: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
            return Self::default();
        };
        let mut state = Self::default();
        let clamp = |width: f64| (width as f32).clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        if let Some(panes) = value.get("panes").and_then(|v| v.as_array()) {
            if let Some(saved) = panes.first() {
                if let Some(open) = saved.get("open").and_then(|v| v.as_bool()) {
                    state.sidebar.open = open;
                }
                if let Some(width) = saved.get("width").and_then(|v| v.as_f64()) {
                    state.sidebar.width = clamp(width);
                }
            }
        } else {
            if let Some(open) = value.get("sidebar").and_then(|v| v.as_bool()) {
                state.sidebar.open = open;
            }
            if let Some(width) = value.get("sidebar_width").and_then(|v| v.as_f64()) {
                state.sidebar.width = clamp(width);
            }
        }
        if let Some(font) = value
            .get("font")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            state.font = font;
        }
        if let Some(font_size) = value
            .get("font_size")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            state.font_size = font_size;
        }
        if let Some(recent) = value.get("recent_commands").and_then(|v| v.as_array()) {
            state.recent_commands = recent
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .take(RECENT_COMMANDS)
                .collect();
        }
        if let Some(router) = value.get("router").and_then(|v| v.as_object()) {
            state.router.current = router
                .get("current")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(Route::Assistant);
            for (key, history) in [
                ("back", &mut state.router.back),
                ("forward", &mut state.router.forward),
            ] {
                if let Some(saved) = router.get(key).and_then(|v| v.as_array()) {
                    *history = saved
                        .iter()
                        .filter_map(|v| serde_json::from_value(v.clone()).ok())
                        .collect();
                }
            }
        } else if let Some(tabs) = value.get("tabs").and_then(|v| v.as_array()) {
            let active = value.get("active").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let selected = tabs
                .get(active)
                .and_then(|v| serde_json::from_value::<Route>(v.clone()).ok())
                .map(|route| (active, route))
                .or_else(|| {
                    tabs.iter().enumerate().find_map(|(i, v)| {
                        serde_json::from_value::<Route>(v.clone())
                            .ok()
                            .map(|route| (i, route))
                    })
                });
            if let Some((index, route)) = selected {
                state.router.current = route;
                if let Some(history) = value.get("history").and_then(|v| v.get(index)) {
                    let entries = history.get("entries").and_then(|v| v.as_array());
                    let cursor = history
                        .get("cursor")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as usize);
                    if let (Some(entries), Some(cursor)) = (entries, cursor) {
                        let before: Option<Vec<Route>> = entries[..cursor.min(entries.len())]
                            .iter()
                            .map(|v| serde_json::from_value(v.clone()).ok())
                            .collect();
                        let after: Option<Vec<Route>> = entries
                            .get(cursor.saturating_add(1)..)
                            .unwrap_or(&[])
                            .iter()
                            .map(|v| serde_json::from_value(v.clone()).ok())
                            .collect();
                        if entries
                            .get(cursor)
                            .and_then(|v| serde_json::from_value::<Route>(v.clone()).ok())
                            == Some(route)
                            && let (Some(back), Some(mut forward)) = (before, after)
                        {
                            forward.reverse();
                            state.router.back = back;
                            state.router.forward = forward;
                        }
                    }
                }
            }
        }
        state
    }
    pub fn load_checked(path: &Path) -> io::Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        let value: serde_json::Value = serde_json::from_str(&text)?;
        if value
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 1
        {
            return Err(io::Error::other("UI preferences require a newer app"));
        }
        Ok(Self::from_json(&text))
    }
    #[cfg(test)]
    pub fn load(path: &Path) -> Self {
        Self::load_checked(path).unwrap()
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, data)?;
        fs::rename(temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_round_trips() {
        let mut s = UiState::default();
        s.navigate(Route::Tickets);
        s.navigate(Route::Agents);
        s.go(false);
        assert_eq!(s.current(), Route::Tickets);
        assert_eq!(UiState::from_json(&serde_json::to_string(&s).unwrap()), s);
    }
    #[test]
    fn legacy_session_and_unknown_route_preserve_preferences() {
        let s = UiState::from_json(
            r#"{"tabs":["today","evee","home"],"active":1,"sidebar":false,"font":"helvetica_neue","evee_width":390}"#,
        );
        assert_eq!(s.current(), Route::Assistant);
        assert!(!s.sidebar.open);
        assert_eq!(s.font, FontChoice::HelveticaNeue);
        assert_eq!(
            serde_json::to_string(&Route::Assistant).unwrap(),
            "\"evee\""
        );
        let unknown =
            UiState::from_json(r#"{"tabs":["future"],"font":"helvetica_neue","sidebar":false}"#);
        assert_eq!(unknown.current(), Route::Assistant);
        let removed = UiState::from_json(
            r#"{"router":{"current":"today","back":["home"],"forward":[]},"panes":[{"open":false,"width":286},{"open":true,"width":390}]}"#,
        );
        assert_eq!(removed.current(), Route::Assistant);
        assert_eq!(removed.sidebar.width, 286.);
        assert!(!removed.sidebar.open);
        let saved = serde_json::to_string(&removed).unwrap();
        assert!(!saved.contains("390"));
        assert!(saved.contains(r#""panes":[{"open":false,"width":286.0}]"#));
        let retained = UiState::from_json(
            r#"{"router":{"current":"tickets","back":["today","evee"],"forward":["home","agents"]}}"#,
        );
        assert_eq!(retained.current(), Route::Tickets);
        assert_eq!(retained.router.back, vec![Route::Assistant]);
        assert_eq!(retained.router.forward, vec![Route::Agents]);
        assert_eq!(unknown.font, FontChoice::HelveticaNeue);
        assert!(!unknown.sidebar.open);
    }
    #[test]
    fn future_preferences_are_unavailable_and_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let text = r#"{"schema_version":99,"font":"helvetica_neue"}"#;
        std::fs::write(&path, text).unwrap();
        assert!(UiState::load_checked(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }
    #[test]
    fn sidebar_width_is_ui_state_and_clamped_on_restore() {
        let mut state = UiState::default();
        assert_eq!(state.sidebar.width, SIDEBAR_DEFAULT);
        state.sidebar.width = 286.;
        state.sidebar.open = false;
        let restored = UiState::from_json(&serde_json::to_string(&state).unwrap());
        assert_eq!(restored.sidebar.width, 286.);
        assert!(!restored.sidebar.open);
        assert_eq!(
            UiState::from_json(r#"{"sidebar_width":100}"#).sidebar.width,
            SIDEBAR_MIN
        );
        assert_eq!(
            UiState::from_json(r#"{"sidebar_width":900}"#).sidebar.width,
            SIDEBAR_MAX
        );
    }
    #[test]
    fn font_size_is_a_backward_compatible_ui_preference() {
        let legacy = UiState::from_json(r#"{"font":"helvetica_neue"}"#);
        assert_eq!(legacy.font_size, FontSize::Default);
        let mut sized = legacy.clone();
        sized.font_size = FontSize::Larger;
        assert_eq!(
            UiState::from_json(&serde_json::to_string(&sized).unwrap()),
            sized
        );
        assert_eq!(
            UiState::from_json(r#"{"font_size":"future_size"}"#).font_size,
            FontSize::Default
        );
    }
    #[test]
    fn recent_commands_dedupe_and_persist() {
        let mut state = UiState::default();
        for id in [
            "page.tickets",
            "page.agents",
            "page.tickets",
            "a",
            "b",
            "c",
            "d",
        ] {
            state.remember_command(id);
        }
        assert_eq!(state.recent_commands.len(), RECENT_COMMANDS);
        assert_eq!(state.recent_commands[0], "d");
        assert_eq!(state.recent_commands.last().unwrap(), "page.tickets");
        assert!(!state.recent_commands.contains(&"page.agents".to_owned()));
        let restored = UiState::from_json(&serde_json::to_string(&state).unwrap());
        assert_eq!(restored.recent_commands, state.recent_commands);
        assert!(UiState::from_json("{}").recent_commands.is_empty());
    }
    #[test]
    fn file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut s = UiState::default();
        s.navigate(Route::Agents);
        s.save(&path).unwrap();
        assert_eq!(UiState::load(&path), s);
        fs::remove_file(path).unwrap();
    }
}
