//! Reusable controls for preference pages and other native surfaces.
use super::{button::*, layout::*, tokens::*};
use gpui::{prelude::*, *};

/// A titled group with the same inset surface and row rhythm throughout Settings.
pub fn settings_section(title: &'static str, rows: impl IntoElement) -> Div {
    column()
        .gap(px(9.))
        .child(
            div()
                .px(px(SETTINGS_INSET))
                .text_size(type_size(LABEL_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(MUTED))
                .child(title),
        )
        .child(
            column()
                .rounded(px(PANEL_RADIUS))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE_SETTINGS_ROW))
                .child(rows),
        )
}

/// One label and description on the left, with its control aligned on the right.
pub fn settings_row(
    label: &'static str,
    description: impl Into<SharedString>,
    control: impl IntoElement,
) -> Div {
    row()
        .debug_selector(move || format!("settings.row.{label}"))
        .min_h(px(SETTINGS_ROW_HEIGHT))
        .px(px(SETTINGS_INSET))
        .py(px(SETTINGS_INSET))
        .gap(px(16.))
        .justify_between()
        .child(
            column()
                .debug_selector(move || format!("settings.row.{label}.label"))
                .flex_1()
                .min_w_0()
                .gap(px(3.))
                .child(div().font_weight(FontWeight::MEDIUM).child(label))
                .child(
                    div()
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(MUTED))
                        .child(description.into()),
                ),
        )
        .child(
            div()
                .debug_selector(move || format!("settings.row.{label}.control"))
                .max_w(px(450.))
                .flex_shrink_0()
                .child(control),
        )
}

pub fn settings_divider() -> Div {
    div().ml(px(SETTINGS_INSET)).h(px(1.)).bg(rgb(BORDER))
}

/// A compact action with the shared focus, disabled and keyboard behavior.
pub fn settings_button<V: 'static>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let label = label.into();
    action_button(
        ButtonSpec {
            id: id.into(),
            label: label.clone(),
            kind: ButtonKind::Quiet,
            enabled,
        },
        |button| {
            button
                .min_h(px(CONTROL_HEIGHT))
                .px(px(12.))
                .justify_center()
                .border_1()
                .border_color(rgb(BORDER_OVERLAY))
                .bg(rgb(SURFACE_SEGMENT))
                .hover(|s| s.bg(rgb(HOVER_CONTROL)))
                .text_size(type_size(CAPTION_SIZE))
                .child(label)
        },
        action,
        cx,
    )
}

/// Compact action with an SVG mark before its text.
pub fn settings_icon_button<V: 'static>(
    id: impl Into<ElementId>,
    label: &'static str,
    icon_name: &'static str,
    enabled: bool,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    action_button(
        ButtonSpec {
            id: id.into(),
            label: label.into(),
            kind: ButtonKind::Quiet,
            enabled,
        },
        |button| {
            button
                .min_h(px(CONTROL_HEIGHT))
                .px(px(12.))
                .gap(px(7.))
                .justify_center()
                .border_1()
                .border_color(rgb(BORDER_OVERLAY))
                .bg(rgb(SURFACE_SEGMENT))
                .hover(|s| s.bg(rgb(HOVER_CONTROL)))
                .text_size(type_size(CAPTION_SIZE))
                .child(
                    svg()
                        .path(format!("{icon_name}.svg"))
                        .size(px(19.))
                        .text_color(rgb(TEXT)),
                )
                .child(label)
        },
        action,
        cx,
    )
}

/// A choice within a compact segmented control.
pub fn settings_segment<V: 'static>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    enabled: bool,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let label = label.into();
    action_button(
        ButtonSpec {
            id: id.into(),
            label: label.clone(),
            kind: ButtonKind::Quiet,
            enabled,
        },
        |button| {
            button
                .role(accesskit::Role::RadioButton)
                .aria_toggled(if selected {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                })
                .min_h(px(CONTROL_HEIGHT))
                .px(px(11.))
                .justify_center()
                .bg(rgb(if selected {
                    SELECTED_SEGMENT
                } else {
                    SURFACE_SEGMENT
                }))
                .hover(|s| s.bg(rgb(HOVER_CONTROL)))
                .text_size(type_size(CAPTION_SIZE))
                .child(label)
        },
        action,
        cx,
    )
}

pub fn settings_segments(children: impl IntoIterator<Item = impl IntoElement>) -> Div {
    row()
        .flex_wrap()
        .max_w(px(450.))
        .p(px(3.))
        .gap(px(2.))
        .rounded(px(8.))
        .bg(rgb(SURFACE_SEGMENT))
        .border_1()
        .border_color(rgb(BORDER))
        .children(children)
}

/// One option in a single-selection chip group.
pub fn choice_button(button: Stateful<Div>, selected: bool) -> Stateful<Div> {
    button
        .role(accesskit::Role::RadioButton)
        .aria_toggled(if selected {
            accesskit::Toggled::True
        } else {
            accesskit::Toggled::False
        })
        .border_1()
        .border_color(rgb(if selected { SELECTED_BORDER } else { BORDER }))
}

/// Accessible switch with a GPUI spring that keeps its velocity when retargeted.
pub fn settings_switch<V: 'static>(
    id: &'static str,
    label: &'static str,
    on: bool,
    enabled: bool,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    action_button(
        ButtonSpec {
            id: id.into(),
            label: label.into(),
            kind: ButtonKind::Quiet,
            enabled,
        },
        |button| {
            button
                .role(accesskit::Role::Switch)
                .aria_toggled(if on {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                })
                .rounded_full()
                .size(px(40.))
                .h(px(24.))
                .p(px(3.))
                .bg(rgb(switch_track(on)))
                .focus(|s| s.border_1().border_color(rgb(FOCUS)))
                .child(
                    div()
                        .size(px(18.))
                        .rounded_full()
                        .bg(rgb(switch_knob(on)))
                        .with_spring(
                            format!("{id}.knob"),
                            SpringAnimation::new(SpringConfig::new(260., 27., 1.)).to(px(if on {
                                16.
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

fn switch_track(on: bool) -> u32 {
    if on { PRIMARY } else { SELECTED_BORDER }
}

fn switch_knob(on: bool) -> u32 {
    if on { TEXT_ON_PRIMARY } else { PRIMARY }
}

#[cfg(test)]
mod tests {
    use super::{switch_knob, switch_track};
    use crate::ui::{PRIMARY, SELECTED_BORDER, TEXT_ON_PRIMARY};

    #[test]
    fn switch_colors_follow_both_states() {
        for on in [false, true, false, true] {
            assert_eq!(switch_track(on), if on { PRIMARY } else { SELECTED_BORDER });
            assert_eq!(switch_knob(on), if on { TEXT_ON_PRIMARY } else { PRIMARY });
        }
    }
}
