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
            Self::Evee => "Evee",
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
        }
    }
}
impl Session {
    pub fn current(&self) -> Option<Space> {
        self.tabs.get(self.active).copied().flatten()
    }
    pub fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }
    pub fn new_tab(&mut self) {
        if let Some(at) = self.tabs.iter().position(Option::is_none) {
            self.active = at;
        } else {
            self.tabs.push(None);
            self.history.push(History::default());
            self.active = self.tabs.len() - 1;
        }
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
    /// Picker/search selects an existing destination, otherwise replaces this tab.
    pub fn open(&mut self, space: Space) {
        let blank = self.current().is_none().then_some(self.active);
        if let Some(at) = self.tabs.iter().position(|tab| *tab == Some(space)) {
            self.active = at;
            if let Some(blank) = blank {
                self.tabs.remove(blank);
                self.history.remove(blank);
                if blank < at {
                    self.active -= 1;
                }
            }
        } else {
            self.navigate(space);
        }
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
    pub fn close(&mut self, index: usize) {
        if index >= self.tabs.len() || self.tabs.len() == 1 {
            return;
        }
        self.tabs.remove(index);
        self.history.remove(index);
        if index < self.active {
            self.active -= 1;
        } else if index == self.active {
            self.active = index.saturating_sub(1);
        }
    }
    pub fn from_json(json: &str) -> Self {
        let Ok(saved) = serde_json::from_str::<Self>(json) else {
            return Self::default();
        };
        let active = saved
            .tabs
            .iter()
            .take(saved.active)
            .filter(|tab| tab.is_some())
            .count();
        let mut clean = Self {
            tabs: vec![],
            history: vec![],
            evee_width: if saved.evee_width.is_finite() {
                saved.evee_width.clamp(220., 480.)
            } else {
                258.
            },
            sidebar: saved.sidebar,
            evee: saved.evee,
            ..Self::default()
        };
        for (index, tab) in saved.tabs.into_iter().enumerate() {
            if let Some(space) = tab {
                let history = saved
                    .history
                    .get(index)
                    .filter(|h| h.entries.get(h.cursor) == Some(&space))
                    .cloned()
                    .unwrap_or_else(|| History::at(space));
                clean.tabs.push(Some(space));
                clean.history.push(history);
            }
        }
        if clean.tabs.is_empty() {
            clean.tabs.push(Some(Space::Today));
            clean.history.push(History::at(Space::Today));
        }
        clean.active = active.min(clean.tabs.len() - 1);
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
    fn blank_replaces_and_deduplicates() {
        let mut s = Session::default();
        s.new_tab();
        s.new_tab();
        assert_eq!(s.tabs.len(), 2);
        s.open(Space::Home);
        assert_eq!(s.tabs, vec![Some(Space::Today), Some(Space::Home)]);
        s.new_tab();
        s.open(Space::Home);
        assert_eq!(s.tabs.len(), 2);
        assert_eq!(s.current(), Some(Space::Home));
    }
    #[test]
    fn closing_preserves_selection_and_falls_back_left() {
        let mut s = Session::default();
        s.new_tab();
        s.open(Space::Tasks);
        s.new_tab();
        s.open(Space::Home);
        s.close(1);
        assert_eq!(s.current(), Some(Space::Home));
        s.close(1);
        assert_eq!(s.current(), Some(Space::Today));
        s.close(0);
        assert_eq!(s.tabs.len(), 1);
        s.new_tab();
        s.close(1);
        assert_eq!(s.current(), Some(Space::Today));
    }
    #[test]
    fn sidebar_replaces_active_without_creating_or_switching_tabs() {
        let mut s = Session::default();
        s.new_tab();
        s.open(Space::Home);
        s.select(0);
        s.navigate(Space::Home);
        assert_eq!(s.active, 0);
        assert_eq!(s.tabs, vec![Some(Space::Home), Some(Space::Home)]);
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
        s.navigate(Space::Settings);
        assert_eq!(s.tabs.len(), 2);
        assert_eq!(s.current(), Some(Space::Settings));
    }
    #[test]
    fn selection_does_not_create_tabs() {
        let mut s = Session::default();
        s.new_tab();
        s.open(Space::Tasks);
        s.select(0);
        s.select(99);
        assert_eq!(s.current(), Some(Space::Today));
        assert_eq!(s.tabs.len(), 2);
    }
    #[test]
    fn restores_state_and_recovers_invalid_input() {
        let mut s = Session::default();
        s.open(Space::Library);
        s.sidebar = false;
        s.evee = false;
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
        for bad in ["", "garbage", "null", r#"{"tabs":["future"]}"#] {
            assert_eq!(Session::from_json(bad), Session::default());
        }
        let cleaned = Session::from_json(r#"{"tabs":["home","home",null],"active":99}"#);
        assert_eq!(cleaned.tabs, vec![Some(Space::Home), Some(Space::Home)]);
        assert_eq!(cleaned.active, 1);
    }
    #[test]
    fn transient_blank_restores_to_today() {
        let mut s = Session::default();
        s.new_tab();
        assert_eq!(
            Session::from_json(&serde_json::to_string(&s).unwrap()),
            Session::default()
        );
    }
    #[test]
    fn every_tab_can_close_except_the_last() {
        let mut s = Session::default();
        s.new_tab();
        s.open(Space::Home);
        s.close(0);
        assert_eq!(s.tabs, vec![Some(Space::Home)]);
        s.close(0);
        assert_eq!(s.current(), Some(Space::Home));
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
        s.new_tab();
        s.close(0);
        s.close(0);
        assert_eq!(s.tabs, vec![None]);
    }
    #[test]
    fn history_is_per_tab_and_new_navigation_truncates_forward() {
        let mut s = Session::default();
        s.navigate(Space::Tasks);
        s.navigate(Space::Home);
        s.go(false);
        assert_eq!(s.current(), Some(Space::Tasks));
        s.go(true);
        assert_eq!(s.current(), Some(Space::Home));
        s.go(false);
        s.navigate(Space::Library);
        assert!(!s.can_go(true));
        s.new_tab();
        s.open(Space::Agents);
        assert!(!s.can_go(false));
        s.close(1);
        s.go(false);
        assert_eq!(s.current(), Some(Space::Tasks));
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
        let clean =
            Session::from_json(r#"{"tabs":["home"],"history":[{"entries":[],"cursor":99}]}"#);
        assert!(!clean.can_go(false));
    }
    #[test]
    fn panel_width_restores_with_safe_bounds() {
        let s = Session::from_json(r#"{"evee_width": 390}"#);
        assert_eq!(s.evee_width, 390.);
        assert_eq!(
            Session::from_json(r#"{"evee_width": -20}"#).evee_width,
            220.
        );
        assert_eq!(
            Session::from_json(r#"{"evee_width": 9000}"#).evee_width,
            480.
        );
    }
    #[test]
    fn search_is_case_insensitive_and_handles_no_results() {
        assert_eq!(Space::matching(" HOME "), vec![Space::Home]);
        assert!(Space::matching("zzz").is_empty());
        assert_eq!(Space::matching("").len(), 9);
    }
    #[test]
    fn file_round_trip_and_missing_file() {
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
