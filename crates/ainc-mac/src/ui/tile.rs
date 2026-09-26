//! Glanceable controls: switch tiles that turn white when on, steppers for a
//! bounded number, and hero numerals.
use super::{button::*, display::icon, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

/// A large switch for one light or group: white with black ink when on, a
/// quiet raised surface when off. `pending` shows the change is travelling.
#[allow(clippy::too_many_arguments)]
pub fn switch_tile<V: HoverHost>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    glyph: &'static str,
    on: bool,
    pending: bool,
    enabled: bool,
    hover: &HoverFade,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let id = id.into();
    let label = label.into();
    let (progress, on_hover) = hover.track(&id, enabled, cx);
    let (surface, surface_hover, ink, secondary) = if on {
        (
            PRIMARY,
            PRIMARY_HOVER,
            TEXT_ON_PRIMARY,
            TEXT_ON_PRIMARY_SECONDARY,
        )
    } else {
        (SURFACE_RAISED, HOVER_STRONG, TEXT, TEXT_SECONDARY)
    };
    let state = match (on, pending) {
        (true, true) => "Turning on…",
        (false, true) => "Turning off…",
        (true, false) => "On",
        (false, false) => "Off",
    };
    action_button(
        ButtonSpec {
            id,
            label: label.clone(),
            enabled,
        },
        |button| {
            button
                .role(accesskit::Role::Switch)
                .aria_toggled(toggled(on))
                .flex_col()
                .items_start()
                .justify_between()
                .flex_1()
                .min_w(px(TILE_MIN_WIDTH))
                .h(px(TILE_HEIGHT))
                .p(px(SPACE_4))
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(rgb(if on { PRIMARY } else { BORDER }))
                .bg(blend(surface, surface_hover, progress))
                .on_hover(on_hover)
                .child(
                    row()
                        .w_full()
                        .justify_between()
                        .child(icon(glyph, ICON_SIZE_LG).text_color(rgb(ink)))
                        .child(
                            // The lamp's own indicator: filled when lit, a ring when dark.
                            div()
                                .size(px(SPACE_2))
                                .rounded_full()
                                .border_1()
                                .border_color(rgb(if on { TEXT_ON_PRIMARY } else { BORDER_STRONG }))
                                .when(on, |s| s.bg(rgb(TEXT_ON_PRIMARY)))
                                .when(pending, |s| s.opacity(0.5)),
                        ),
                )
                .child(
                    column()
                        .w_full()
                        .gap(px(SPACE_HALF))
                        .child(
                            div()
                                .w_full()
                                .truncate()
                                .text_size(type_size(BODY_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(ink))
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(secondary))
                                .child(state),
                        ),
                )
        },
        action,
        cx,
    )
}

/// A bounded number with − and + beside it, such as a thermostat target.
#[allow(clippy::too_many_arguments)]
pub fn stepper<V: HoverHost>(
    id: &str,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    can_lower: bool,
    can_raise: bool,
    hover: &HoverFade,
    on_step: impl Fn(&mut V, i64, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Div {
    let label = label.into();
    let lower = on_step.clone();
    row()
        .debug_selector({
            let id = id.to_owned();
            move || id.clone()
        })
        .gap(px(SPACE_1))
        .child(
            Button::new(
                SharedString::from(format!("{id}.lower")),
                format!("Lower {label}"),
            )
            .secondary()
            .icon("minus")
            .icon_only()
            .enabled(can_lower)
            .build(
                hover,
                move |view, window, cx| lower(view, -1, window, cx),
                cx,
            ),
        )
        .child(
            div()
                .w(px(STEPPER_VALUE_WIDTH))
                .flex()
                .justify_center()
                .text_size(type_size(TITLE_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(TEXT))
                .child(value.into()),
        )
        .child(
            Button::new(
                SharedString::from(format!("{id}.raise")),
                format!("Raise {label}"),
            )
            .secondary()
            .icon("plus")
            .icon_only()
            .enabled(can_raise)
            .build(
                hover,
                move |view, window, cx| on_step(view, 1, window, cx),
                cx,
            ),
        )
}

/// A glanceable numeral set light and large.
pub fn hero(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(type_size(HERO_SIZE))
        .line_height(relative(1.))
        .font_weight(FontWeight::LIGHT)
        .text_color(rgb(TEXT))
        .child(text.into())
}
