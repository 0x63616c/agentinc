//! The one surface that can float over the shell at a time. The shell owns
//! the palette, notifications and the user menu; a page's dialogs and
//! popovers are registered under its route and drawn by the page.
use crate::routes::Route;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Overlay {
    Search,
    Notifications,
    UserMenu {
        support: bool,
    },
    /// A page's modal dialog on the scrim; the page draws it through `Page::overlay`.
    Dialog(Route),
    /// A page's light surface that closes when the pointer lands outside it.
    Popover(Route),
}
impl Overlay {
    /// Modal surfaces on a scrim that block the page beneath.
    pub fn is_dialog(self) -> bool {
        matches!(self, Self::Dialog(_))
    }
    /// Light surfaces that close when the pointer lands outside them.
    pub fn is_popover(self) -> bool {
        matches!(
            self,
            Self::Popover(_) | Self::Notifications | Self::UserMenu { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Overlay, Route};
    #[test]
    fn popovers_and_dialogs_are_distinct() {
        assert!(Overlay::UserMenu { support: true }.is_popover());
        assert!(!Overlay::UserMenu { support: false }.is_dialog());
        assert!(Overlay::Dialog(Route::Tickets).is_dialog());
        assert!(!Overlay::Dialog(Route::Tickets).is_popover());
        assert!(Overlay::Popover(Route::Assistant).is_popover());
        assert!(!Overlay::Search.is_dialog());
    }
}
