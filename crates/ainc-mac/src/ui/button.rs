//! Native actions with one focus, disabled, and keyboard contract.
use super::{layout::row, tokens::*};
use gpui::{prelude::*, *};

#[derive(Clone, Copy)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Quiet,
    Destructive,
}

pub struct ButtonSpec {
    pub id: ElementId,
    pub label: SharedString,
    pub kind: ButtonKind,
    pub enabled: bool,
}

/// Common focus, activation and disabled contract; callers own layout and hover fades.
pub fn button_base(spec: ButtonSpec) -> Stateful<Div> {
    // Only authored names/business keys become public IDs, never allocation IDs.
    let author_id = match &spec.id {
        ElementId::Name(name) => Some(name.clone()),
        ElementId::NamedInteger(name, key) => Some(format!("{name}.{key}").into()),
        _ => None,
    };
    let filled = matches!(spec.kind, ButtonKind::Primary | ButtonKind::Destructive);
    row()
        .id(spec.id)
        .role(accesskit::Role::Button)
        .aria_label(spec.label)
        .when_some(author_id, |s, id| s.accessibility_id(id))
        .a11y_synthetic_children(move |builder| {
            if !spec.enabled {
                builder.parent_node().set_disabled();
            }
        })
        .when(spec.enabled, |s| s.tab_index(0).cursor_pointer())
        .rounded(px(CONTROL_RADIUS))
        .opacity(if spec.enabled { 1. } else { DISABLED_OPACITY })
        .focus(move |s| {
            let s = s.border_1().border_color(rgb(FOCUS));
            if filled { s } else { s.bg(rgb(FOCUS_SURFACE)) }
        })
        .when(matches!(spec.kind, ButtonKind::Primary), |s| {
            s.bg(rgb(PRIMARY)).text_color(rgb(PRIMARY_INK))
        })
        .when(matches!(spec.kind, ButtonKind::Destructive), |s| {
            s.bg(rgb(DESTRUCTIVE)).text_color(rgb(TEXT))
        })
}

pub fn action_button<V: 'static>(
    spec: ButtonSpec,
    decorate: impl FnOnce(Stateful<Div>) -> Stateful<Div>,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let enabled = spec.enabled;
    let key_action = action.clone();
    decorate(button_base(spec))
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

pub fn standard_button(button: Stateful<Div>) -> Stateful<Div> {
    button
        .min_h(type_size(CONTROL_HEIGHT))
        .px(px(CONTROL_INSET_X))
}

/// A short text action, used by the Assistant and menu surfaces.
pub fn compact_button(button: Stateful<Div>) -> Stateful<Div> {
    button
        .justify_center()
        .px(px(COMPACT_CONTROL_INSET_X))
        .py(px(COMPACT_CONTROL_INSET_Y))
        .text_size(type_size(CAPTION_SIZE))
}

/// Header icon action with the same hit target and focus contract as text actions.
pub fn icon_control(button: Stateful<Div>) -> Stateful<Div> {
    button.size(px(HEADER_CONTROL)).justify_center()
}
