//! One active overlay and one return-focus target per native window, plus the
//! shells for dialogs, sheets, popovers and menus.
use super::{button::*, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

/// Tracks which overlay is open and where focus returns when it closes. The
/// overlay vocabulary belongs to the host; the shell uses `overlay::Overlay`.
/// One popover (a select or menu) may float above the active surface, so a
/// form in a dialog can open its selects; Escape and a click outside close
/// the popover first.
pub struct OverlayHost<O> {
    active: Option<O>,
    popover: Option<O>,
    return_focus: Option<FocusHandle>,
    pending_focus: Option<FocusHandle>,
}
impl<O> Default for OverlayHost<O> {
    fn default() -> Self {
        Self {
            active: None,
            popover: None,
            return_focus: None,
            pending_focus: None,
        }
    }
}
impl<O: Copy + PartialEq> OverlayHost<O> {
    pub fn active(&self) -> Option<O> {
        self.active
    }
    /// The select or menu floating above the active surface, if any.
    pub fn popover(&self) -> Option<O> {
        self.popover
    }
    pub fn open_popover(&mut self, popover: O) {
        self.popover = Some(popover);
    }
    /// Returns whether a popover was open.
    pub fn close_popover(&mut self) -> bool {
        self.popover.take().is_some()
    }
    pub fn open(
        &mut self,
        overlay: O,
        window: &mut Window,
        cx: &mut App,
        initial: Option<FocusHandle>,
    ) {
        self.popover = None;
        if self.active.is_none() {
            self.return_focus = window.focused(cx);
        }
        self.active = Some(overlay);
        if let Some(initial) = initial {
            window.focus(&initial, cx);
        }
    }
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if self.close_popover() {
            return true;
        }
        let was_open = self.active.is_some();
        self.close();
        if let Some(focus) = self.pending_focus.take() {
            window.focus(&focus, cx);
        }
        was_open
    }
    pub fn close(&mut self) {
        self.popover = None;
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

/// What a dialog's confirm button does. Its pending label follows the verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Create,
    Save,
    Delete,
    Add,
}
impl Verb {
    pub fn label(self) -> &'static str {
        match self {
            Self::Create => "Create",
            Self::Save => "Save",
            Self::Delete => "Delete",
            Self::Add => "Add",
        }
    }
    pub fn pending(self) -> &'static str {
        match self {
            Self::Create => "Creating…",
            Self::Save => "Saving…",
            Self::Delete => "Deleting…",
            Self::Add => "Adding…",
        }
    }
}

/// The right-aligned Cancel / confirm row every dialog ends with. The confirm
/// button is destructive for `Verb::Delete`, disabled while pending and reads
/// the verb's pending label meanwhile.
pub struct DialogFooter {
    verb: Verb,
    label: Option<SharedString>,
    ids: (&'static str, &'static str),
    enabled: bool,
    pending: bool,
    focus: Option<(FocusHandle, FocusHandle)>,
}
impl DialogFooter {
    pub fn new(verb: Verb) -> Self {
        Self {
            verb,
            label: None,
            ids: ("dialog.cancel", "dialog.submit"),
            enabled: true,
            pending: false,
            focus: None,
        }
    }
    /// A longer confirm label than the bare verb ("Add relationship").
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
    /// Element ids for the Cancel and confirm buttons.
    pub fn ids(mut self, cancel: &'static str, submit: &'static str) -> Self {
        self.ids = (cancel, submit);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }
    /// The two focus targets the dialog's Tab ring ends with.
    pub fn focus(mut self, cancel: &FocusHandle, submit: &FocusHandle) -> Self {
        self.focus = Some((cancel.clone(), submit.clone()));
        self
    }
    pub fn build<V: 'static>(
        self,
        ui: &mut Ui<V>,
        cancel: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
        submit: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    ) -> Div {
        let label: SharedString = if self.pending {
            self.verb.pending().into()
        } else {
            self.label.unwrap_or_else(|| self.verb.label().into())
        };
        let mut cancel_button = Button::new(self.ids.0, "Cancel").secondary();
        let mut submit_button = Button::new(self.ids.1, label)
            .kind(if self.verb == Verb::Delete {
                ButtonKind::Destructive
            } else {
                ButtonKind::Primary
            })
            .enabled(self.enabled && !self.pending);
        if let Some((cancel_focus, submit_focus)) = &self.focus {
            cancel_button = cancel_button.track_focus(cancel_focus);
            submit_button = submit_button.track_focus(submit_focus);
        }
        row()
            .gap(px(CONTROL_GAP))
            .justify_end()
            .child(cancel_button.build(ui, cancel))
            .child(submit_button.build(ui, submit))
    }
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

/// A floating surface for menus and the user popover.
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
        host.open_popover(7);
        // Escape closes the popover first, then the surface under it.
        assert!(host.close_popover());
        assert_eq!(host.active(), Some(42));
        host.close(); // Escape and Cancel both use this state transition.
        assert_eq!(host.active(), None);
        assert_eq!(host.popover(), None);
    }
}
