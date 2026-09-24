//! Small layout contracts; callers own screen composition.
use super::tokens::*;
use gpui::{prelude::*, *};

pub fn row() -> Div {
    div().flex().items_center()
}
pub fn column() -> Div {
    div().flex().flex_col()
}
pub fn row_gap(gap: f32) -> Div {
    row().gap(px(gap))
}
pub fn column_gap(gap: f32) -> Div {
    column().gap(px(gap))
}

/// A non-action list entry. Interactive rows use the shared Button contract.
pub fn list_item(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    row()
        .id(id)
        .role(accesskit::Role::ListItem)
        .aria_label(label)
}

/// The regular list rhythm, including selection and a subtle separator.
pub fn list_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> Stateful<Div> {
    list_item(id, label)
        .min_h(px(LIST_ROW_HEIGHT))
        .border_b_1()
        .border_color(rgb(BORDER))
        .when(selected, |row| row.bg(rgb(SELECTED)))
}

pub fn panel() -> Div {
    column()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded(px(PANEL_RADIUS))
}
