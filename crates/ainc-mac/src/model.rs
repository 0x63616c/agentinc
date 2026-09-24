//! Single-route navigation and legacy session recovery, independent of GPUI.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

pub const PANE_WIDTHS: [(f32, f32, f32); 1] = [(150., 320., 216.)];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanePreference {
    pub open: bool,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    #[serde(alias = "tasks")]
    Tickets,
    Agents,
    Automations,
    Terminal,
    #[serde(rename = "evee", alias = "assistant")]
    Assistant,
    Settings,
}

pub struct PageSpec {
    pub route: Route,
    pub title: &'static str,
    pub icon: &'static str,
    pub in_sidebar: bool,
}

pub const PAGES: &[PageSpec] = &[
    PageSpec {
        route: Route::Tickets,
        title: "Tickets",
        icon: "tasks",
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Assistant,
        title: "Assistant",
        icon: "spark",
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Agents,
        title: "Agents",
        icon: "agents",
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Automations,
        title: "Automations",
        icon: "refresh",
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Terminal,
        title: "Terminal",
        icon: "terminal",
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Settings,
        title: "Settings",
        icon: "settings",
        in_sidebar: false,
    },
];

impl Route {
    pub fn from_shortcut(number: u8) -> Option<Self> {
        if number > 9 {
            return None;
        }
        let index = if number == 0 {
            9
        } else {
            usize::from(number - 1)
        };
        PAGES
            .iter()
            .filter(|page| page.in_sidebar)
            .nth(index)
            .map(|page| page.route)
    }

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
            current: Route::Assistant,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub schema_version: u32,
    router: Router,
    pub panes: [PanePreference; 1],
    pub font: FontChoice,
    pub font_size: FontSize,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            schema_version: 1,
            router: Router::default(),
            panes: [PanePreference {
                open: true,
                width: PANE_WIDTHS[0].2,
            }],
            font: FontChoice::System,
            font_size: FontSize::Default,
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
        if let Some(panes) = value.get("panes").and_then(|v| v.as_array()) {
            for (index, pane) in session.panes.iter_mut().enumerate() {
                if let Some(saved) = panes.get(index) {
                    if let Some(open) = saved.get("open").and_then(|v| v.as_bool()) {
                        pane.open = open;
                    }
                    if let Some(width) = saved.get("width").and_then(|v| v.as_f64()) {
                        pane.width =
                            (width as f32).clamp(PANE_WIDTHS[index].0, PANE_WIDTHS[index].1);
                    }
                }
            }
        } else {
            if let Some(open) = value.get("sidebar").and_then(|v| v.as_bool()) {
                session.panes[0].open = open;
            }
            if let Some(width) = value.get("sidebar_width").and_then(|v| v.as_f64()) {
                session.panes[0].width = (width as f32).clamp(PANE_WIDTHS[0].0, PANE_WIDTHS[0].1);
            }
        }
        if let Some(font) = value
            .get("font")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            session.font = font;
        }
        if let Some(font_size) = value
            .get("font_size")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            session.font_size = font_size;
        }
        if let Some(router) = value.get("router").and_then(|v| v.as_object()) {
            session.router.current = router
                .get("current")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(Route::Assistant);
            for (key, history) in [
                ("back", &mut session.router.back),
                ("forward", &mut session.router.forward),
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
        assert_eq!(PAGES.len(), 6);
        for route in [
            Route::Tickets,
            Route::Agents,
            Route::Automations,
            Route::Terminal,
            Route::Assistant,
            Route::Settings,
        ] {
            assert_eq!(route.spec().route, route);
        }
        let mut s = Session::default();
        s.navigate(Route::Tickets);
        s.navigate(Route::Agents);
        s.go(false);
        assert_eq!(s.current(), Route::Tickets);
        s.navigate(Route::Automations);
        assert!(!s.can_go(true));
        assert_eq!(Session::from_json(&serde_json::to_string(&s).unwrap()), s);
    }
    #[test]
    fn sidebar_shortcuts_follow_visual_order() {
        let sidebar: Vec<_> = PAGES
            .iter()
            .filter(|page| page.in_sidebar)
            .map(|page| page.route)
            .collect();
        assert_eq!(
            sidebar,
            [
                Route::Tickets,
                Route::Assistant,
                Route::Agents,
                Route::Automations,
                Route::Terminal,
            ]
        );
        for (index, route) in sidebar.into_iter().enumerate() {
            assert_eq!(Route::from_shortcut(((index + 1) % 10) as u8), Some(route));
        }
        assert_eq!(Route::from_shortcut(0), None);
        assert_eq!(Route::from_shortcut(10), None);
    }
    #[test]
    fn legacy_session_and_unknown_route_preserve_preferences() {
        let s = Session::from_json(
            r#"{"tabs":["today","evee","home"],"active":1,"sidebar":false,"font":"helvetica_neue","evee_width":390}"#,
        );
        assert_eq!(s.current(), Route::Assistant);
        assert!(!s.panes[0].open);
        assert_eq!(s.font, FontChoice::HelveticaNeue);
        assert_eq!(s.panes.len(), 1);
        assert_eq!(
            serde_json::to_string(&Route::Assistant).unwrap(),
            "\"evee\""
        );
        let unknown =
            Session::from_json(r#"{"tabs":["future"],"font":"helvetica_neue","sidebar":false}"#);
        assert_eq!(unknown.current(), Route::Assistant);
        let removed = Session::from_json(
            r#"{"router":{"current":"today","back":["home"],"forward":[]},"panes":[{"open":false,"width":286},{"open":true,"width":390}]}"#,
        );
        assert_eq!(removed.current(), Route::Assistant);
        assert_eq!(removed.panes[0].width, 286.);
        assert!(!removed.panes[0].open);
        assert!(!serde_json::to_string(&removed).unwrap().contains("390"));
        let retained = Session::from_json(
            r#"{"router":{"current":"tickets","back":["today","evee"],"forward":["home","agents"]}}"#,
        );
        assert_eq!(retained.current(), Route::Tickets);
        assert_eq!(retained.router.back, vec![Route::Assistant]);
        assert_eq!(retained.router.forward, vec![Route::Agents]);
        assert_eq!(unknown.font, FontChoice::HelveticaNeue);
        assert!(!unknown.panes[0].open);
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
    fn sidebar_width_is_ui_session_state_and_clamped_on_restore() {
        let mut session = Session::default();
        assert_eq!(session.panes[0].width, PANE_WIDTHS[0].2);
        session.panes[0].width = 286.;
        session.panes[0].open = false;
        let restored = Session::from_json(&serde_json::to_string(&session).unwrap());
        assert_eq!(restored.panes[0].width, 286.);
        assert!(!restored.panes[0].open);
        assert_eq!(
            Session::from_json(r#"{"sidebar_width":100}"#).panes[0].width,
            PANE_WIDTHS[0].0
        );
        assert_eq!(
            Session::from_json(r#"{"sidebar_width":900}"#).panes[0].width,
            PANE_WIDTHS[0].1
        );
    }
    #[test]
    fn font_size_is_a_backward_compatible_ui_preference() {
        let legacy = Session::from_json(r#"{"font":"helvetica_neue"}"#);
        assert_eq!(legacy.font_size, FontSize::Default);
        let mut sized = legacy.clone();
        sized.font_size = FontSize::Larger;
        assert_eq!(
            Session::from_json(&serde_json::to_string(&sized).unwrap()),
            sized
        );
        assert_eq!(
            Session::from_json(r#"{"font_size":"future_size"}"#).font_size,
            FontSize::Default
        );
    }
    #[test]
    fn search_and_file_round_trip() {
        assert!(Route::matching(" HOME ").is_empty());
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
