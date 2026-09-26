//! One active overlay and one return-focus target per native window, plus the
//! shells for dialogs, sheets, popovers and menus.
use super::{layout::*, tokens::*};
use gpui::{prelude::*, *};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Overlay {
    Search,
    Notifications,
    UserMenu,
    AddTicket,
    AddAgent,
    DeleteTicket(i64),
    RenameConversation(i64),
    DeleteConversation(i64),
    ConversationMenu(i64),
}
impl Overlay {
    /// Modal surfaces on a scrim that block the page beneath.
    pub fn is_dialog(self) -> bool {
        matches!(
            self,
            Self::AddTicket
                | Self::AddAgent
                | Self::DeleteTicket(_)
                | Self::RenameConversation(_)
                | Self::DeleteConversation(_)
        )
    }
    /// Light surfaces that close when the pointer lands outside them.
    pub fn is_popover(self) -> bool {
        matches!(
            self,
            Self::ConversationMenu(_) | Self::Notifications | Self::UserMenu
        )
    }
}

#[derive(Default)]
pub struct OverlayHost {
    active: Option<Overlay>,
    return_focus: Option<FocusHandle>,
    pending_focus: Option<FocusHandle>,
}
impl OverlayHost {
    pub fn active(&self) -> Option<Overlay> {
        self.active
    }
    pub fn open(
        &mut self,
        overlay: Overlay,
        window: &mut Window,
        cx: &mut App,
        initial: Option<FocusHandle>,
    ) {
        if self.active.is_none() {
            self.return_focus = window.focused(cx);
        }
        self.active = Some(overlay);
        if let Some(initial) = initial {
            window.focus(&initial, cx);
        }
    }
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut App) -> bool {
        let was_open = self.active.is_some();
        self.close();
        if let Some(focus) = self.pending_focus.take() {
            window.focus(&focus, cx);
        }
        was_open
    }
    pub fn close(&mut self) {
        if self.active.take().is_some() {
            self.pending_focus = self.return_focus.take();
        }
    }
    pub fn take_pending_focus(&mut self) -> Option<FocusHandle> {
        self.pending_focus.take()
    }
    pub fn cycle_focus(
        &self,
        handles: &[FocusHandle],
        backwards: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if handles.is_empty() {
            return;
        }
        let current = handles.iter().position(|handle| handle.is_focused(window));
        let next = match (current, backwards) {
            (Some(index), true) => (index + handles.len() - 1) % handles.len(),
            (Some(index), false) => (index + 1) % handles.len(),
            (None, true) => handles.len() - 1,
            (None, false) => 0,
        };
        window.focus(&handles[next], cx);
    }
}

fn overlay_surface(id: &'static str) -> Stateful<Div> {
    column()
        .id(id)
        .bg(rgb(SURFACE_OVERLAY))
        .border_1()
        .border_color(rgb(BORDER_STRONG))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
}

fn overlay_title(title: SharedString) -> Div {
    div()
        .text_size(type_size(DIALOG_TITLE_SIZE))
        .line_height(relative(TITLE_LINE_HEIGHT))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

/// A centered modal with a title, body and footer actions.
pub fn dialog_shell(
    title: impl Into<SharedString>,
    body: impl IntoElement,
    footer: impl IntoElement,
) -> Stateful<Div> {
    let title = title.into();
    overlay_surface("shared-dialog")
        .accessibility_id("dialog")
        .role(accesskit::Role::Dialog)
        .aria_label(title.clone())
        .w(px(DIALOG_WIDTH))
        .p(px(DIALOG_PADDING))
        .gap(px(SPACE_5))
        .rounded(px(DIALOG_RADIUS))
        .shadow(shadow_dialog())
        .child(overlay_title(title))
        .child(body)
        .child(footer)
}

/// A full-height panel that slides in from the right edge of the window.
pub fn sheet_shell(
    title: impl Into<SharedString>,
    body: impl IntoElement,
    footer: impl IntoElement,
) -> Stateful<Div> {
    let title = title.into();
    overlay_surface("shared-sheet")
        .accessibility_id("sheet")
        .role(accesskit::Role::Dialog)
        .aria_label(title.clone())
        .w(px(SHEET_WIDTH))
        .h_full()
        .p(px(DIALOG_PADDING))
        .gap(px(SPACE_5))
        .rounded_l(px(PANEL_RADIUS))
        .shadow(shadow_dialog())
        .child(overlay_title(title))
        .child(
            column()
                .id("sheet-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(body),
        )
        .child(footer)
}

/// A floating surface for menus, user and notification popovers.
pub fn popover_shell(width: f32) -> Stateful<Div> {
    overlay_surface("popover")
        .w(px(width))
        .p(px(MENU_INSET))
        .rounded(px(RADIUS_LG))
        .shadow(shadow_overlay())
}

/// A compact dropdown surface.
pub fn menu_shell(width: f32) -> Stateful<Div> {
    overlay_surface("menu")
        .w(px(width.max(MENU_WIDTH)))
        .p(px(MENU_INSET))
        .rounded(px(MENU_RADIUS))
        .shadow(shadow_overlay())
}

#[cfg(test)]
mod tests {
    use super::{Overlay, OverlayHost};
    #[test]
    fn closing_rename_clears_the_topmost_overlay() {
        let mut host = OverlayHost {
            active: Some(Overlay::RenameConversation(42)),
            ..Default::default()
        };
        host.close(); // Escape and Cancel both use this state transition.
        assert_eq!(host.active(), None);
    }
    #[test]
    fn popovers_and_dialogs_are_distinct() {
        assert!(Overlay::UserMenu.is_popover());
        assert!(!Overlay::UserMenu.is_dialog());
        assert!(Overlay::AddTicket.is_dialog());
        assert!(!Overlay::AddTicket.is_popover());
        assert!(!Overlay::Search.is_dialog());
    }
}
