//! Switches and checkboxes with a white on-state.
use super::{button::*, display::icon, layout::*, tokens::*};
use gpui::{prelude::*, *};

/// Accessible switch with a GPUI spring that keeps its velocity when retargeted.
pub fn toggle<V: 'static>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    enabled: bool,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let id = id.into();
    let knob_id = ElementId::NamedChild(std::sync::Arc::new(id.clone()), "knob".into());
    let selector = format!("{id:?}");
    let knob = TOGGLE_HEIGHT - 4.;
    let travel = TOGGLE_WIDTH - TOGGLE_HEIGHT;
    action_button(
        ButtonSpec {
            id,
            label: label.into(),
            enabled,
        },
        |button| {
            button
                .role(accesskit::Role::Switch)
                .aria_toggled(toggled(on))
                .debug_selector(move || selector.clone())
                .rounded_full()
                .w(px(TOGGLE_WIDTH))
                .h(px(TOGGLE_HEIGHT))
                .p(px(2.))
                .bg(rgb(if on { PRIMARY } else { BORDER_STRONG }))
                .child(
                    div()
                        .size(px(knob))
                        .rounded_full()
                        .bg(rgb(if on { TEXT_ON_PRIMARY } else { PRIMARY }))
                        .with_spring(
                            knob_id,
                            SpringAnimation::new(SPRING_SNAPPY).to(px(if on {
                                travel
                            } else {
                                0.
                            })),
                            |knob, offset| knob.ml(offset),
                        ),
                )
        },
        action,
        cx,
    )
}

/// A checkbox with its label; the whole row toggles.
pub fn checkbox<V: 'static>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    enabled: bool,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let label = label.into();
    let id = id.into();
    let selector = format!("{id:?}");
    action_button(
        ButtonSpec {
            id,
            label: label.clone(),
            enabled,
        },
        |button| {
            button
                .role(accesskit::Role::CheckBox)
                .aria_toggled(toggled(checked))
                .debug_selector(move || selector.clone())
                .gap(px(SPACE_2))
                .rounded(px(RADIUS_XS))
                .child(
                    row()
                        .size(px(CHECKBOX_SIZE))
                        .flex_shrink_0()
                        .justify_center()
                        .rounded(px(RADIUS_XS))
                        .border_1()
                        .border_color(rgb(if checked { PRIMARY } else { BORDER_STRONG }))
                        .bg(rgb(if checked { PRIMARY } else { SURFACE_INPUT }))
                        .when(checked, |s| {
                            s.child(icon("check", 12.).text_color(rgb(TEXT_ON_PRIMARY)))
                        }),
                )
                .child(
                    div()
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(TEXT))
                        .child(label),
                )
        },
        action,
        cx,
    )
}
