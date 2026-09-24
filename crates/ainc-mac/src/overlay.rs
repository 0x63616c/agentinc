//! One active overlay and one return-focus target per native window.
use crate::style::*;
use gpui::{prelude::*, *};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Overlay {
    Search,
    Notifications,
    AddTask,
    DeleteTask(i64),
    TaskMenu(i64),
    RenameConversation(i64),
    DeleteConversation(i64),
    ConversationMenu(i64),
}
impl Overlay {
    pub fn is_dialog(self) -> bool {
        matches!(
            self,
            Self::AddTask
                | Self::DeleteTask(_)
                | Self::RenameConversation(_)
                | Self::DeleteConversation(_)
        )
    }
    pub fn is_menu(self) -> bool {
        matches!(self, Self::TaskMenu(_) | Self::ConversationMenu(_))
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
        cx: &App,
        initial: Option<FocusHandle>,
    ) {
        if self.active.is_none() {
            self.return_focus = window.focused(cx);
        }
        self.active = Some(overlay);
        if let Some(initial) = initial {
            window.focus(&initial);
        }
    }
    pub fn dismiss(&mut self, window: &mut Window) -> bool {
        let was_open = self.active.is_some();
        self.close();
        if let Some(focus) = self.pending_focus.take() {
            window.focus(&focus);
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
    pub fn cycle_focus(&self, handles: &[FocusHandle], backwards: bool, window: &mut Window) {
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
        window.focus(&handles[next]);
    }
}

pub fn dialog_shell(
    title: impl Into<SharedString>,
    body: impl IntoElement,
    footer: impl IntoElement,
) -> Stateful<Div> {
    column()
        .id("shared-dialog")
        .w(px(440.))
        .p(px(24.))
        .gap(px(20.))
        .bg(rgb(0x171717))
        .border_1()
        .border_color(rgb(0x353535))
        .rounded(px(12.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .text_size(px(18.))
                .font_weight(FontWeight::MEDIUM)
                .child(title.into()),
        )
        .child(body)
        .child(footer)
}

pub fn menu_shell(content: impl IntoElement) -> Div {
    column()
        .w(px(146.))
        .p(px(4.))
        .bg(rgb(0x1c1c1c))
        .border_1()
        .border_color(rgb(0x353535))
        .rounded(px(6.))
        .child(content)
}

pub fn action_button<V: 'static>(
    button: Stateful<Div>,
    enabled: bool,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let key_action = action.clone();
    button
        .on_click(cx.listener(move |view, _, window, cx| {
            cx.stop_propagation();
            if enabled {
                action(view, window, cx);
            }
        }))
        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                cx.stop_propagation();
                if enabled {
                    key_action(view, window, cx);
                }
            }
        }))
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
}
