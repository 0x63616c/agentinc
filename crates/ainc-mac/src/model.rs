//! Single-route navigation and legacy session recovery, independent of GPUI.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Today,
    Tasks,
    Agents,
    Home,
    Calendar,
    Library,
    Apps,
    #[serde(rename = "evee", alias = "assistant")]
    Assistant,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    Ready,
    Planned,
}

pub struct PageSpec {
    pub route: Route,
    pub title: &'static str,
    pub icon: &'static str,
    pub shortcut: Option<u8>,
    pub in_sidebar: bool,
    pub availability: Availability,
}

pub const PAGES: &[PageSpec] = &[
    PageSpec {
        route: Route::Today,
        title: "Today",
        icon: "sun",
        shortcut: Some(1),
        in_sidebar: true,
        availability: Availability::Ready,
    },
    PageSpec {
        route: Route::Tasks,
        title: "Tasks",
        icon: "tasks",
        shortcut: Some(2),
        in_sidebar: true,
        availability: Availability::Ready,
    },
    PageSpec {
        route: Route::Agents,
        title: "Agents",
        icon: "agents",
        shortcut: Some(3),
        in_sidebar: true,
        availability: Availability::Planned,
    },
    PageSpec {
        route: Route::Home,
        title: "Home",
        icon: "home",
        shortcut: Some(4),
        in_sidebar: true,
        availability: Availability::Planned,
    },
    PageSpec {
        route: Route::Calendar,
        title: "Calendar",
        icon: "calendar",
        shortcut: Some(5),
        in_sidebar: true,
        availability: Availability::Planned,
    },
    PageSpec {
        route: Route::Library,
        title: "Library",
        icon: "photos",
        shortcut: Some(6),
        in_sidebar: true,
        availability: Availability::Planned,
    },
    PageSpec {
        route: Route::Apps,
        title: "My apps",
        icon: "grid",
        shortcut: Some(7),
        in_sidebar: true,
        availability: Availability::Planned,
    },
    PageSpec {
        route: Route::Assistant,
        title: "Assistant",
        icon: "spark",
        shortcut: Some(8),
        in_sidebar: true,
        availability: Availability::Ready,
    },
    PageSpec {
        route: Route::Settings,
        title: "Settings",
        icon: "settings",
        shortcut: None,
        in_sidebar: false,
        availability: Availability::Ready,
    },
];

impl Route {
    pub fn spec(self) -> &'static PageSpec {
        PAGES
            .iter()
            .find(|page| page.route == self)
            .expect("all routes have page metadata")
    }
    pub fn label(self) -> &'static str {
        self.spec().title
    }
    pub fn icon(self) -> &'static str {
        self.spec().icon
    }
    pub fn matching(query: &str) -> Vec<Self> {
        let query = query.trim().to_lowercase();
        PAGES
            .iter()
            .filter(|page| page.title.to_lowercase().contains(&query))
            .map(|page| page.route)
            .collect()
    }
    pub fn empty(self) -> (&'static str, &'static str) {
        match self {
            Self::Today => (
                "Your day starts here",
                "Choose a destination from the sidebar.",
            ),
            Self::Tasks => ("No tasks yet", "Add a task to get started."),
            Self::Agents => (
                "No agents connected",
                "Agent runs and reviews will appear here.",
            ),
            Self::Home => (
                "Home is not connected",
                "Home controls are not connected yet.",
            ),
            Self::Calendar => (
                "No calendars connected",
                "Calendar accounts are not connected yet.",
            ),
            Self::Library => (
                "No photos yet",
                "Photos and library search are coming later.",
            ),
            Self::Apps => ("No apps yet", "Personal apps will appear here."),
            Self::Assistant => ("No conversations yet", "Start a conversation with Evee."),
            Self::Settings => ("Settings", "Manage your account and preferences."),
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
            Self::Assistant => &[
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Router {
    current: Route,
    back: Vec<Route>,
    forward: Vec<Route>,
}
impl Default for Router {
    fn default() -> Self {
        Self {
            current: Route::Today,
            back: vec![],
            forward: vec![],
        }
    }
}
impl Router {
    pub fn current(&self) -> Route {
        self.current
    }
    pub fn navigate(&mut self, to: Route) {
        if to != self.current {
            self.back.push(self.current);
            self.current = to;
            self.forward.clear();
        }
    }
    pub fn can_go(&self, forward: bool) -> bool {
        if forward {
            !self.forward.is_empty()
        } else {
            !self.back.is_empty()
        }
    }
    pub fn go(&mut self, forward: bool) {
        if forward {
            if let Some(next) = self.forward.pop() {
                self.back.push(self.current);
                self.current = next;
            }
        } else if let Some(previous) = self.back.pop() {
            self.forward.push(self.current);
            self.current = previous;
        }
    }
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub schema_version: u32,
    router: Router,
    pub sidebar: bool,
    pub evee: bool,
    pub evee_width: f32,
    pub font: FontChoice,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            schema_version: 1,
            router: Router::default(),
            sidebar: true,
            evee: true,
            evee_width: 258.,
            font: FontChoice::System,
        }
    }
}
impl Session {
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
        let mut session = Self::default();
        if let Some(sidebar) = value.get("sidebar").and_then(|v| v.as_bool()) {
            session.sidebar = sidebar;
        }
        if let Some(evee) = value.get("evee").and_then(|v| v.as_bool()) {
            session.evee = evee;
        }
        if let Some(width) = value.get("evee_width").and_then(|v| v.as_f64()) {
            session.evee_width = (width as f32).clamp(220., 480.);
        }
        if let Some(font) = value
            .get("font")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            session.font = font;
        }
        if let Some(router) = value
            .get("router")
            .and_then(|v| serde_json::from_value::<Router>(v.clone()).ok())
        {
            session.router = router;
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
                session.router.current = route;
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
                            session.router.back = back;
                            session.router.forward = forward;
                        }
                    }
                }
            }
        }
        session
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
    fn catalogue_and_history() {
        assert_eq!(PAGES.len(), 9);
        for route in [
            Route::Today,
            Route::Tasks,
            Route::Agents,
            Route::Home,
            Route::Calendar,
            Route::Library,
            Route::Apps,
            Route::Assistant,
            Route::Settings,
        ] {
            assert_eq!(route.spec().route, route);
        }
        let shortcuts: Vec<_> = PAGES.iter().filter_map(|p| p.shortcut).collect();
        assert_eq!(shortcuts, (1..=8).collect::<Vec<_>>());
        let mut s = Session::default();
        s.navigate(Route::Tasks);
        s.navigate(Route::Home);
        s.go(false);
        assert_eq!(s.current(), Route::Tasks);
        s.navigate(Route::Library);
        assert!(!s.can_go(true));
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
    }
    #[test]
    fn legacy_session_and_unknown_route_preserve_preferences() {
        let s = Session::from_json(
            r#"{"tabs":["today","evee","home"],"active":1,"sidebar":false,"font":"helvetica_neue","evee_width":390}"#,
        );
        assert_eq!(s.current(), Route::Assistant);
        assert!(!s.sidebar);
        assert_eq!(s.font, FontChoice::HelveticaNeue);
        assert_eq!(s.evee_width, 390.);
        assert_eq!(
            serde_json::to_string(&Route::Assistant).unwrap(),
            "\"evee\""
        );
        let unknown =
            Session::from_json(r#"{"tabs":["future"],"font":"helvetica_neue","sidebar":false}"#);
        assert_eq!(unknown.current(), Route::Today);
        assert_eq!(unknown.font, FontChoice::HelveticaNeue);
        assert!(!unknown.sidebar);
    }
    #[test]
    fn future_preferences_are_unavailable_and_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let text = r#"{"schema_version":99,"font":"helvetica_neue"}"#;
        std::fs::write(&path, text).unwrap();
        assert!(Session::load_checked(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }
    #[test]
    fn search_and_file_round_trip() {
        assert_eq!(Route::matching(" HOME "), vec![Route::Home]);
        assert!(Route::matching("zzz").is_empty());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut s = Session::default();
        s.navigate(Route::Agents);
        s.save(&path).unwrap();
        assert_eq!(Session::load(&path), s);
        fs::remove_file(path).unwrap();
    }
}
