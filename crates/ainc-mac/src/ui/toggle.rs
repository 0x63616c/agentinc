//! Switches and checkboxes with a white on-state.
use super::{button::*, display::icon, icon::Icon, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

/// Accessible switch. One GPUI spring drives the knob and the track color together, so the
/// track changes color as the knob arrives, and the spring keeps its velocity when retargeted.
pub struct Toggle {
    id: ElementId,
    label: SharedString,
    on: bool,
    enabled: bool,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            on: false,
            enabled: true,
        }
    }
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    pub fn build<V: 'static>(
        self,
        ui: &mut Ui<V>,
        action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    ) -> Stateful<Div> {
        let Self {
            id,
            label,
            on,
            enabled,
        } = self;
        let track_id = ElementId::NamedChild(std::sync::Arc::new(id.clone()), "track".into());
        let (hover, on_hover) = ui.hover(&id, enabled);
        let knob = TOGGLE_HEIGHT - 4.;
        let travel = TOGGLE_WIDTH - TOGGLE_HEIGHT;
        action_button(
            ButtonSpec { id, label, enabled },
            |button| {
                button
                    .role(accesskit::Role::Switch)
                    .aria_toggled(toggled(on))
                    .rounded_full()
                    .w(px(TOGGLE_WIDTH))
                    .h(px(TOGGLE_HEIGHT))
                    .on_hover(on_hover)
                    .child(div().size_full().rounded_full().p(px(2.)).with_spring(
                        track_id,
                        SpringAnimation::new(SPRING_SNAPPY).to(if on { 1f32 } else { 0f32 }),
                        move |track, progress| {
                            // The track brightens on hover in both states.
                            let off = blend(BORDER_STRONG, FOCUS_FIELD, hover);
                            let on = blend(PRIMARY, PRIMARY_HOVER, hover);
                            track.bg(mix(off, on, progress)).child(
                                div()
                                    .size(px(knob))
                                    .rounded_full()
                                    .bg(blend(PRIMARY, TEXT_ON_PRIMARY, progress))
                                    .ml(px(travel * progress)),
                            )
                        },
                    ))
            },
            action,
            ui.cx,
        )
    }
}

/// A checkbox with its label; the whole row toggles.
pub struct Checkbox {
    id: ElementId,
    label: SharedString,
    checked: bool,
    enabled: bool,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked: false,
            enabled: true,
        }
    }
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn build<V: 'static>(
        self,
        ui: &mut Ui<V>,
        action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    ) -> Stateful<Div> {
        let Self {
            id,
            label,
            checked,
            enabled,
        } = self;
        let (hover, on_hover) = ui.hover(&id, enabled);
        let (border, surface) = if checked {
            (
                blend(PRIMARY, PRIMARY_HOVER, hover),
                blend(PRIMARY, PRIMARY_HOVER, hover),
            )
        } else {
            (
                blend(BORDER_STRONG, FOCUS_FIELD, hover),
                blend(SURFACE_INPUT, SURFACE_CONTROL, hover),
            )
        };
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
                    .gap(px(SPACE_2))
                    .rounded(px(RADIUS_XS))
                    .on_hover(on_hover)
                    .child(
                        row()
                            .size(px(CHECKBOX_SIZE))
                            .flex_shrink_0()
                            .justify_center()
                            .rounded(px(RADIUS_XS))
                            .border_1()
                            .border_color(border)
                            .bg(surface)
                            .when(checked, |s| {
                                s.child(
                                    icon(Icon::Check, ICON_SIZE_XS)
                                        .text_color(rgb(TEXT_ON_PRIMARY)),
                                )
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
            ui.cx,
        )
    }
}
