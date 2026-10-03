//! Navigation vocabulary: the Routes, the page catalogue behind them, the
//! Router's history and the one Destination event pages send the shell.
use crate::ui::Icon;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    #[serde(alias = "tasks")]
    Tickets,
    Agents,
    Automations,
    Terminal,
    Temporal,
    #[serde(rename = "evee", alias = "assistant")]
    Assistant,
    Settings,
    Connections,
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
        route: Route::Tickets,
        title: "Tickets",
        icon: Icon::Tasks,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Assistant,
        title: "Assistant",
        icon: Icon::EveeOutline,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Agents,
        title: "Agents",
        icon: Icon::Agents,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Automations,
        title: "Automations",
        icon: Icon::Repeat,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Terminal,
        title: "Terminal",
        icon: Icon::Terminal,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Temporal,
        title: "Temporal",
        icon: Icon::Temporal,
        in_sidebar: true,
    },
    PageSpec {
        route: Route::Settings,
        title: "Settings",
        icon: Icon::Settings,
        in_sidebar: false,
    },
    PageSpec {
        route: Route::Connections,
        title: "Connections",
        icon: Icon::Link,
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
            .find(|page| page.route == self)
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
        self.label().to_lowercase()
    }
}

/// Where a page asks the shell to take the person. The shell navigates to
/// [`Destination::route`] and hands the destination to every page, so the
/// page that owns the record can select it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Page(Route),
    /// A Ticket's detail.
    Ticket(i64),
    /// The create dialog on the Tickets page.
    NewTicket,
    /// A Conversation, open in the Assistant.
    Conversation(i64),
}
impl Destination {
    pub fn route(&self) -> Route {
        match self {
            Self::Page(route) => *route,
            Self::Ticket(_) | Self::NewTicket => Route::Tickets,
            Self::Conversation(_) => Route::Assistant,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_and_history() {
        assert_eq!(PAGES.len(), 9);
        for route in [
            Route::Tickets,
            Route::Agents,
            Route::Automations,
            Route::Terminal,
            Route::Temporal,
            Route::Assistant,
            Route::Settings,
            Route::Connections,
            Route::Components,
        ] {
            assert_eq!(route.spec().route, route);
        }
        let mut router = Router::default();
        router.navigate(Route::Tickets);
        router.navigate(Route::Agents);
        router.go(false);
        assert_eq!(router.current(), Route::Tickets);
        router.navigate(Route::Automations);
        assert!(!router.can_go(true));
        assert_eq!(Destination::Ticket(4).route(), Route::Tickets);
        assert_eq!(Destination::Conversation(1).route(), Route::Assistant);
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
                Route::Temporal,
            ]
        );
        for (index, route) in sidebar.into_iter().enumerate() {
            assert_eq!(Route::from_shortcut(((index + 1) % 10) as u8), Some(route));
        }
        assert_eq!(Route::from_shortcut(0), None);
        assert_eq!(Route::from_shortcut(6), Some(Route::Temporal));
        assert_eq!(Route::from_shortcut(7), None);
        assert_eq!(Route::from_shortcut(10), None);
    }
}
