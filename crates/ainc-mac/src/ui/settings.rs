//! Settings surfaces: titled sections of label/description/control rows.
use super::{display::eyebrow, layout::*, tokens::*};
use gpui::{prelude::*, *};

/// A titled group with the same inset surface and row rhythm throughout Settings.
pub fn settings_section(title: &'static str, rows: impl IntoElement) -> Div {
    column()
        .gap(px(SPACE_2))
        .debug_selector(move || format!("settings.section.{title}"))
        .child(div().px(px(SPACE_HALF)).child(eyebrow(title)))
        .child(
            column()
                .overflow_hidden()
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE_RAISED))
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
        .py(px(SPACE_3))
        .gap(px(SPACE_4))
        .justify_between()
        .child(
            column()
                .debug_selector(move || format!("settings.row.{label}.label"))
                .flex_1()
                .min_w_0()
                .gap(px(SPACE_HALF))
                .child(
                    div()
                        .text_size(type_size(BODY_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(TEXT_SECONDARY))
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
    div()
        .ml(px(SETTINGS_INSET))
        .h(px(1.))
        .bg(rgb(BORDER_SUBTLE))
}
