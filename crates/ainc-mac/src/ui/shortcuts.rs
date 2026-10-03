//! The one shortcuts table. Each entry carries the action a person reads, the
//! GPUI keystroke the shell binds and the glyphs the UI shows. Bindings, `kbd`
//! pills, the Settings list, accessible labels and the README table all come
//! from here, so they cannot drift apart. Glyph notation has no separators:
//! `⌘K`, `⌘[`, `⌘,`, `⎋`, `↵`, `⇧↵`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub action: &'static str,
    pub keystroke: &'static str,
    pub glyph: &'static str,
}

impl Shortcut {
    /// An accessible label: `Search · ⌘K`.
    pub fn labelled(self, label: &str) -> String {
        format!("{label} · {}", self.glyph)
    }
}

pub const GO_TO: Shortcut = Shortcut {
    action: "Search",
    keystroke: "cmd-k",
    glyph: "⌘K",
};
pub const BACK: Shortcut = Shortcut {
    action: "Back",
    keystroke: "cmd-[",
    glyph: "⌘[",
};
pub const FORWARD: Shortcut = Shortcut {
    action: "Forward",
    keystroke: "cmd-]",
    glyph: "⌘]",
};
pub const NEW_TAB: Shortcut = Shortcut {
    action: "New tab",
    keystroke: "cmd-t",
    glyph: "⌘T",
};
pub const CLOSE_TAB: Shortcut = Shortcut {
    action: "Close tab",
    keystroke: "cmd-shift-w",
    glyph: "⇧⌘W",
};
pub const PREVIOUS_TAB: Shortcut = Shortcut {
    action: "Previous tab",
    keystroke: "cmd-shift-[",
    glyph: "⇧⌘[",
};
pub const NEXT_TAB: Shortcut = Shortcut {
    action: "Next tab",
    keystroke: "cmd-shift-]",
    glyph: "⇧⌘]",
};
pub const CHECK_UPDATES: Shortcut = Shortcut {
    action: "Check for Updates",
    keystroke: "cmd-shift-u",
    glyph: "⇧⌘U",
};
/// The sidebar pages, one digit each; `route(n)` gives one page's binding.
pub const PAGES: Shortcut = Shortcut {
    action: "Tickets, Assistant, Agents…",
    keystroke: "cmd-1…cmd-6",
    glyph: "⌘1–6",
};
pub const SETTINGS: Shortcut = Shortcut {
    action: "Settings",
    keystroke: "cmd-,",
    glyph: "⌘,",
};
pub const TOGGLE_SIDEBAR: Shortcut = Shortcut {
    action: "Toggle sidebar",
    keystroke: "cmd-b",
    glyph: "⌘B",
};
pub const DISMISS: Shortcut = Shortcut {
    action: "Dismiss",
    keystroke: "escape",
    glyph: "⎋",
};
pub const SEND: Shortcut = Shortcut {
    action: "Send",
    keystroke: "enter",
    glyph: "↵",
};
pub const NEW_LINE: Shortcut = Shortcut {
    action: "New line",
    keystroke: "shift-enter",
    glyph: "⇧↵",
};

/// Every shortcut, in the order Settings and the README list them.
pub const ALL: &[Shortcut] = &[
    GO_TO,
    BACK,
    FORWARD,
    NEW_TAB,
    CLOSE_TAB,
    PREVIOUS_TAB,
    NEXT_TAB,
    CHECK_UPDATES,
    PAGES,
    SETTINGS,
    TOGGLE_SIDEBAR,
    DISMISS,
    SEND,
    NEW_LINE,
];

/// The keystroke and glyph for sidebar page `n` (`⌘1` … `⌘9`, `⌘0`).
pub fn route(n: usize) -> (String, String) {
    let digit = n % 10;
    (format!("cmd-{digit}"), format!("⌘{digit}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_have_no_separators() {
        for shortcut in ALL {
            assert!(!shortcut.glyph.contains(' '), "{}", shortcut.glyph);
            assert!(!shortcut.glyph.contains('+'), "{}", shortcut.glyph);
        }
    }

    #[test]
    fn routes_wrap_at_ten() {
        assert_eq!(route(1), ("cmd-1".into(), "⌘1".into()));
        assert_eq!(route(10), ("cmd-0".into(), "⌘0".into()));
        assert_eq!(GO_TO.labelled("Search"), "Search · ⌘K");
    }
}
