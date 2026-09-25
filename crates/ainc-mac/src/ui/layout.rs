//! Small layout contracts; callers own screen composition.
use super::{display::PageHeader, tokens::*};
use gpui::{prelude::*, *};

/// The common page frame. Document pages share the same header, full-width
/// content and inset; canvas pages let chat and the terminal use their height.
pub struct Page {
    header: Option<PageHeader>,
    content: Div,
}

impl Page {
    pub fn document(header: PageHeader) -> Self {
        Self {
            header: Some(header),
            content: column().w_full().min_w_0().gap(px(24.)),
        }
    }

    pub fn canvas() -> Self {
        Self {
            header: None,
            content: column().size_full().min_w_0().min_h_0(),
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.content = self.content.child(child);
        self
    }

    pub fn build(self) -> Stateful<Div> {
        let frame = column()
            .id("page")
            .debug_selector(|| "page-frame".into())
            .size_full()
            .min_w_0()
            .min_h_0();
        match self.header {
            Some(header) => frame.overflow_y_scroll().p(px(PAGE_X)).child(
                column()
                    .debug_selector(|| "main-content".into())
                    .w_full()
                    .min_w_0()
                    .gap(px(24.))
                    .child(header.build())
                    .child(self.content),
            ),
            None => frame.child(self.content),
        }
    }
}

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

pub fn status_bar() -> Div {
    panel()
        .debug_selector(|| "status-bar".into())
        .flex_row()
        .items_center()
        .justify_between()
        .h(px(20.))
        .rounded(px(5.))
}
