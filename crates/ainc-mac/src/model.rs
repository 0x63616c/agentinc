//! Navigation is independent of GPUI so session invariants can be checked without a GPU.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Space {
    Today,
    Tasks,
    Agents,
    Home,
    Calendar,
    Library,
    Apps,
    Evee,
    Settings,
}
impl Space {
    pub const ALL: [Self; 9] = [
        Self::Today,
        Self::Tasks,
        Self::Agents,
        Self::Home,
        Self::Calendar,
        Self::Library,
        Self::Apps,
        Self::Evee,
        Self::Settings,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Tasks => "Tasks",
            Self::Agents => "Agents",
            Self::Home => "Home",
            Self::Calendar => "Calendar",
            Self::Library => "Library",
            Self::Apps => "My apps",
            Self::Evee => "Assistant",
            Self::Settings => "Settings",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Today => "sun",
            Self::Tasks => "tasks",
            Self::Agents => "agents",
            Self::Home => "home",
            Self::Calendar => "calendar",
            Self::Library => "photos",
            Self::Apps => "grid",
            Self::Evee => "spark",
            Self::Settings => "settings",
        }
    }
    pub fn empty(self) -> (&'static str, &'static str) {
        match self {
            Self::Settings => (
                "Make this space yours",
                "Account and app settings are coming in a future increment.",
            ),
            Self::Today => (
                "Your space starts here",
                "Open a space from the sidebar or search.",
            ),
            Self::Tasks => (
                "No tasks yet",
                "Task management is coming in a future increment.",
            ),
            Self::Agents => (
                "No agents connected",
                "Agent runs and reviews will appear here.",
            ),
            Self::Home => (
                "Your home, in one place",
                "Home controls are not connected yet.",
            ),
            Self::Calendar => (
                "Room for your plans",
                "Calendar accounts are not connected yet.",
            ),
            Self::Library => (
                "A place for what you keep",
                "Photos and library search are coming later.",
            ),
            Self::Apps => ("Your personal toolkit", "Personal apps will live here."),
            Self::Evee => (
                "Evee is not connected yet",
                "Your assistant will be available here.",
            ),
        }
    }
    pub fn planned(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Tasks => &[
                (
                    "Capture & organize",
                    "A place for tasks, lists and projects.",
                ),
                ("Priorities & due dates", "Keep upcoming work in view."),
                ("Reviews", "Review work completed with your agents."),
            ],
            Self::Agents => &[
                ("Your agents", "Connected agents and their capabilities."),
                ("Runs", "Follow active work and inspect its results."),
                ("Approvals", "A place for decisions that need you."),
            ],
            Self::Home => &[
                ("Rooms & devices", "Your connected home, organized by room."),
                ("Scenes", "Bring familiar device settings together."),
                ("Activity", "Recent changes from your home."),
            ],
            Self::Calendar => &[
                ("Agenda", "Upcoming events from connected calendars."),
                ("Accounts", "Choose which calendars appear here."),
                ("Planning", "Make room for focused work and everyday life."),
            ],
            Self::Library => &[
                ("Collections", "Organize the things you want to keep."),
                ("Photos", "Browse connected photo libraries."),
                ("Search", "Find items across your library."),
            ],
            Self::Apps => &[
                ("Your apps", "Open the tools you use most."),
                ("Personal tools", "A place for small apps made for you."),
                ("Connections", "Manage the services those tools use."),
            ],
            Self::Evee => &[
                (
                    "Conversations",
                    "Return to conversations with your assistant.",
                ),
                ("Context", "Choose what Evee can help you with."),
                ("Actions", "Review proposed actions before they run."),
            ],
            _ => &[],
        }
    }
    pub fn matching(query: &str) -> Vec<Self> {
        let query = query.trim().to_lowercase();
        Self::ALL
            .into_iter()
            .filter(|s| s.label().to_lowercase().contains(&query))
            .collect()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    entries: Vec<Space>,
    cursor: usize,
}
impl History {
    fn at(space: Space) -> Self {
        Self {
            entries: vec![space],
            cursor: 0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<Option<Space>>,
    history: Vec<History>,
    pub active: usize,
    pub sidebar: bool,
    pub evee: bool,
    pub evee_width: f32,
    pub font: FontChoice,
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
impl Default for Session {
    fn default() -> Self {
        Self {
            tabs: vec![Some(Space::Today)],
            history: vec![History::at(Space::Today)],
            active: 0,
            sidebar: true,
            evee: true,
            evee_width: 258.,
            font: FontChoice::System,
        }
    }
}
impl Session {
    pub fn current(&self) -> Option<Space> {
        self.tabs.get(self.active).copied().flatten()
    }
    /// Sidebar and page links navigate the active tab in place.
    pub fn navigate(&mut self, space: Space) {
        if self.current() == Some(space) {
            return;
        }
        let history = &mut self.history[self.active];
        history.entries.truncate(history.cursor + 1);
        history.entries.push(space);
        history.cursor = history.entries.len() - 1;
        self.tabs[self.active] = Some(space);
    }
    /// Search replaces the destination in the one tab.
    pub fn open(&mut self, space: Space) {
        self.navigate(space);
    }
    pub fn can_go(&self, forward: bool) -> bool {
        let h = &self.history[self.active];
        if forward {
            h.cursor + 1 < h.entries.len()
        } else {
            h.cursor > 0
        }
    }
    pub fn go(&mut self, forward: bool) {
        if !self.can_go(forward) {
            return;
        }
        let h = &mut self.history[self.active];
        if forward {
            h.cursor += 1;
        } else {
            h.cursor -= 1;
        }
        self.tabs[self.active] = Some(h.entries[h.cursor]);
    }
    pub fn from_json(json: &str) -> Self {
        let Ok(saved) = serde_json::from_str::<Self>(json) else {
            return Self::default();
        };
        // Older sessions may have many tabs. Keep the active destination and its history.
        let selected = saved
            .tabs
            .get(saved.active)
            .copied()
            .flatten()
            .map(|space| (saved.active, space))
            .or_else(|| {
                saved
                    .tabs
                    .iter()
                    .enumerate()
                    .find_map(|(i, tab)| tab.map(|space| (i, space)))
            });
        let mut clean = Self {
            evee_width: if saved.evee_width.is_finite() {
                saved.evee_width.clamp(220., 480.)
            } else {
                258.
            },
            sidebar: saved.sidebar,
            evee: saved.evee,
            font: saved.font,
            ..Self::default()
        };
        if let Some((index, space)) = selected {
            clean.tabs[0] = Some(space);
            clean.history[0] = saved
                .history
                .get(index)
                .filter(|h| h.entries.get(h.cursor) == Some(&space))
                .cloned()
                .unwrap_or_else(|| History::at(space));
        }
        clean
    }
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .map(|s| Self::from_json(&s))
            .unwrap_or_default()
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
    fn one_tab_navigation_and_history() {
        let mut s = Session::default();
        s.navigate(Space::Tasks);
        s.open(Space::Home);
        assert_eq!(s.tabs, vec![Some(Space::Home)]);
        s.go(false);
        assert_eq!(s.current(), Some(Space::Tasks));
        s.navigate(Space::Library);
        assert!(!s.can_go(true));
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
    }
    #[test]
    fn legacy_tabs_restore_active_into_one_tab() {
        let s =
            Session::from_json(r#"{"tabs":["today","tasks","home"],"active":1,"sidebar":false}"#);
        assert_eq!(s.tabs, vec![Some(Space::Tasks)]);
        assert_eq!(s.active, 0);
        assert!(!s.sidebar);
        assert_eq!(
            Session::from_json(r#"{"tabs":["today",null],"active":1}"#).current(),
            Some(Space::Today)
        );
        for bad in ["", "garbage", "null", r#"{"tabs":["future"]}"#] {
            assert_eq!(Session::from_json(bad), Session::default());
        }
    }
    #[test]
    fn font_and_panel_settings_round_trip() {
        let s = Session {
            font: FontChoice::HelveticaNeue,
            evee: false,
            evee_width: 390.,
            ..Session::default()
        };
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
        assert_eq!(
            Session::from_json(r#"{"evee_width":9000}"#).evee_width,
            480.
        );
    }
    #[test]
    fn search_and_file_round_trip() {
        assert_eq!(Space::matching(" HOME "), vec![Space::Home]);
        assert!(Space::matching("zzz").is_empty());
        let dir = std::env::current_dir().unwrap().join("target/session-test");
        let path = dir.join(format!("{}.json", std::process::id()));
        assert_eq!(Session::load(&path), Session::default());
        let mut s = Session::default();
        s.open(Space::Agents);
        s.save(&path).unwrap();
        assert_eq!(Session::load(&path), s);
        fs::write(&path, "{partial").unwrap();
        assert_eq!(Session::load(&path), Session::default());
        fs::remove_file(path).unwrap();
    }
}
