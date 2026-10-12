//! Navigation vocabulary: the Routes, the page catalogue behind them, the
//! Router's history and the one Destination event pages send the shell.
use crate::ui::Icon;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Overview,
    SmartHome,
    Todos,
    Calendar,
    Settings,
    /// The living component gallery, reachable from the command palette.
    #[serde(alias = "design_system")]
    Components,
}

pub struct PageSpec {
    pub route: Route,
    pub title: &'static str,
    pub icon: Icon,
    pub in_sidebar: bool,
}

pub const PAGES: &[PageSpec] = &[
    PageSpec {
        route: Route::Overview,
        title: "Overview",
        icon: Icon::Home,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::SmartHome,
        title: "Smart Home",
        icon: Icon::SmartHome,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Todos,
        title: "Todos",
        icon: Icon::Todos,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Calendar,
        title: "Calendar",
        icon: Icon::Calendar,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Settings,
        title: "Settings",
        icon: Icon::Settings,
        in_sidebar: false,
    },
    PageSpec {
        route: Route::Components,
        title: "Components",
        icon: Icon::Command,
        in_sidebar: false,
    },
];

impl Route {
    /// The page that renders this route.
    pub fn page(self) -> Self {
        self
    }
    pub fn destination(self) -> Destination {
        Destination::Page(self)
    }
    pub fn from_shortcut(number: u8) -> Option<Self> {
        if !(1..=9).contains(&number) {
            return None;
        }
        let index = usize::from(number - 1);
        PAGES
            .iter()
            .filter(|page| page.in_sidebar)
            .nth(index)
            .map(|page| page.route)
    }

    pub fn spec(self) -> &'static PageSpec {
        PAGES
            .iter()
            .find(|page| page.route == self.page())
            .expect("all routes have page metadata")
    }
    pub fn label(self) -> &'static str {
        self.spec().title
    }
    pub fn icon(self) -> Icon {
        self.spec().icon
    }
    /// The stable lower-case name used in saved files and palette ids.
    pub fn key(self) -> String {
        self.label().to_lowercase().replace(' ', "-")
    }
}

/// Where a page asks the shell to take the person. The shell navigates to
/// [`Destination::route`] and hands the destination to every page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Page(Route),
}
impl Destination {
    pub fn route(&self) -> Route {
        match self {
            Self::Page(route) => *route,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Router {
    pub(crate) current: Route,
    pub(crate) back: Vec<Route>,
    pub(crate) forward: Vec<Route>,
}
impl Default for Router {
    fn default() -> Self {
        Self {
            current: Route::Overview,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_and_history() {
        assert_eq!(PAGES.len(), 6);
        for route in [
            Route::Overview,
            Route::SmartHome,
            Route::Todos,
            Route::Calendar,
            Route::Settings,
            Route::Components,
        ] {
            assert_eq!(route.spec().route, route);
        }
        assert_eq!(Route::SmartHome.key(), "smart-home");
        assert_eq!(
            serde_json::to_string(&Route::SmartHome).unwrap(),
            "\"smart_home\""
        );
        let mut router = Router::default();
        assert_eq!(router.current(), Route::Overview);
        router.navigate(Route::Todos);
        router.navigate(Route::Calendar);
        router.go(false);
        assert_eq!(router.current(), Route::Todos);
        router.navigate(Route::SmartHome);
        assert!(!router.can_go(true));
        assert_eq!(Destination::Page(Route::Todos).route(), Route::Todos);
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
                Route::Overview,
                Route::SmartHome,
                Route::Todos,
                Route::Calendar,
            ]
        );
        for (index, route) in sidebar.into_iter().enumerate() {
            assert_eq!(Route::from_shortcut(((index + 1) % 10) as u8), Some(route));
        }
        assert_eq!(Route::from_shortcut(0), None);
        assert_eq!(Route::from_shortcut(4), Some(Route::Calendar));
        assert_eq!(Route::from_shortcut(5), None);
        assert_eq!(Route::from_shortcut(10), None);
    }
}
