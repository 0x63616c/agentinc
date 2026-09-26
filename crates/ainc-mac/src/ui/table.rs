//! Tables: a bordered container, an eyebrow header and aligned cells.
use super::{layout::*, tokens::*};
use gpui::{prelude::*, *};

pub struct TableColumn {
    pub label: SharedString,
    pub width: Option<f32>,
    pub align_right: bool,
}

impl TableColumn {
    /// A flexible column that takes the remaining width.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            width: None,
            align_right: false,
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    pub fn right(mut self) -> Self {
        self.align_right = true;
        self
    }
}

fn cell(column: &TableColumn) -> Div {
    div()
        .min_w_0()
        .when_some(column.width, |s, width| s.w(px(width)).flex_shrink_0())
        .when(column.width.is_none(), |s| s.flex_1())
        .when(column.align_right, |s| s.text_align(TextAlign::Right))
}

pub fn table_container() -> Div {
    column()
        .w_full()
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE_RAISED))
        .overflow_hidden()
}

pub fn table_header(columns: &[TableColumn]) -> Div {
    row()
        .w_full()
        .h(px(TABLE_HEADER_HEIGHT))
        .px(px(SPACE_4))
        .gap(px(SPACE_3))
        .border_b_1()
        .border_color(rgb(BORDER))
        .text_size(type_size(CAPTION_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(TEXT_TERTIARY))
        .children(
            columns
                .iter()
                .map(|column| cell(column).child(column.label.to_uppercase())),
        )
}

/// One row of cells sized by the columns; the caller wraps it in a row control.
pub fn table_cells(columns: &[TableColumn], cells: Vec<AnyElement>) -> Div {
    row()
        .w_full()
        .min_h(px(TABLE_ROW_HEIGHT))
        .px(px(SPACE_4))
        .gap(px(SPACE_3))
        .children(
            columns
                .iter()
                .zip(cells)
                .map(|(column, content)| cell(column).child(content)),
        )
}

/// A clickable row with the shared button contract and hover fade.
#[allow(clippy::too_many_arguments)]
pub fn table_row<V: super::motion::HoverHost>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    columns: &[TableColumn],
    cells: Vec<AnyElement>,
    enabled: bool,
    hover: &super::motion::HoverFade,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    use super::{button::*, motion::blend};
    let id = id.into();
    let progress = if enabled { hover.progress(&id) } else { 0. };
    let hover_id = id.clone();
    let on_hover = cx.listener(move |view: &mut V, over: &bool, _, cx| {
        if enabled {
            view.hover_fade().set(hover_id.clone(), *over);
            cx.notify();
        }
    });
    action_button(
        ButtonSpec {
            id,
            label: label.into(),
            enabled,
        },
        |button| {
            button
                .w_full()
                .h_auto()
                .rounded(px(0.))
                .opacity(1.)
                .bg(blend(SURFACE_RAISED, HOVER_STRONG, progress))
                .border_b_1()
                .border_color(rgb(BORDER_SUBTLE))
                .on_hover(on_hover)
                .child(table_cells(columns, cells))
        },
        action,
        cx,
    )
}
