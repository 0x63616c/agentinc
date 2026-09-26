//! One active overlay and one return-focus target per native window, plus the
//! shells for dialogs, sheets, popovers and menus.
use super::{layout::*, tokens::*};
use gpui::{prelude::*, *};

/// Tracks which overlay is open and where focus returns when it closes. The
/// overlay vocabulary belongs to the host; the shell uses `model::Overlay`.
pub struct OverlayHost<O> {
    active: Option<O>,
    return_focus: Option<FocusHandle>,
    pending_focus: Option<FocusHandle>,
}
impl<O> Default for OverlayHost<O> {
    fn default() -> Self {
        Self {
            active: None,
            return_focus: None,
            pending_focus: None,
        }
    }
}
impl<O: Copy + PartialEq> OverlayHost<O> {
    pub fn active(&self) -> Option<O> {
        self.active
    }
    pub fn open(
        &mut self,
        overlay: O,
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

/// The right-aligned Cancel / confirm row every dialog ends with.
pub fn dialog_footer(cancel: impl IntoElement, submit: impl IntoElement) -> Div {
    row()
        .gap(px(CONTROL_GAP))
        .justify_end()
        .child(cancel)
        .child(submit)
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
    use super::OverlayHost;
    #[test]
    fn closing_clears_the_topmost_overlay() {
        let mut host = OverlayHost {
            active: Some(42u8),
            ..Default::default()
        };
        host.close(); // Escape and Cancel both use this state transition.
        assert_eq!(host.active(), None);
    }
}
