//! Shared chrome around the native text editing engine.
use super::{
    layout::{column_gap, row},
    tokens::*,
};
use crate::input::TextInput;
use gpui::{prelude::*, *};

/// The labeled single-line field used by Ticket and Automation forms.
pub fn text_field(label: &'static str, input: Entity<TextInput>) -> Div {
    form_field(label, input, label)
}

pub fn form_field(label: &'static str, input: impl IntoElement, selector: &'static str) -> Div {
    column_gap(FIELD_LABEL_GAP)
        .child(
            div()
                .debug_selector(move || format!("{selector}.label"))
                .text_color(rgb(MUTED))
                .text_size(type_size(LABEL_SIZE))
                .child(label),
        )
        .child(
            row()
                .debug_selector(move || format!("{selector}.input"))
                .min_h(type_size(FIELD_HEIGHT))
                .px(px(FIELD_INSET_X))
                .border_1()
                .border_color(rgb(BORDER))
                .rounded(px(FIELD_RADIUS))
                .child(input),
        )
}
