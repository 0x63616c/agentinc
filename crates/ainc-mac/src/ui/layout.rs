//! Small layout contracts; callers own screen composition.
use super::{button::*, display::PageHeader, motion::*, tokens::*};
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
            content: column().w_full().min_w_0().gap(px(SECTION_GAP)),
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
                    .gap(px(SECTION_GAP))
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

/// A non-action list entry. Interactive rows use `ListRow`.
pub fn list_item(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    row()
        .id(id)
        .role(accesskit::Role::ListItem)
        .aria_label(label)
}

/// An interactive list row: optional leading mark, title, subtitle and trailing
/// element, with the shared hover fade and button contract.
pub struct ListRow {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    selected: bool,
    enabled: bool,
}

impl ListRow {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            leading: None,
            trailing: None,
            selected: false,
            enabled: true,
        }
    }
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }
    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn build<V: HoverHost>(
        self,
        hover: &HoverFade,
        action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
        cx: &mut Context<V>,
    ) -> Stateful<Div> {
        let Self {
            id,
            title,
            subtitle,
            leading,
            trailing,
            selected,
            enabled,
        } = self;
        let (progress, on_hover) = hover.track(&id, enabled, cx);
        action_button(
            ButtonSpec {
                id,
                label: title.clone(),
                enabled,
            },
            |button| {
                button
                    .w_full()
                    .min_h(px(LIST_ROW_HEIGHT))
                    .px(px(SPACE_3))
                    .py(px(SPACE_2))
                    .gap(px(SPACE_3))
                    .rounded(px(RADIUS_MD))
                    .bg(if selected {
                        blend(SELECTED, HOVER_STRONG, progress)
                    } else {
                        rgba((HOVER_STRONG << 8) | (progress * 255.) as u32)
                    })
                    .on_hover(on_hover)
                    .when_some(leading, |s, leading| s.child(leading))
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .gap(px(SPACE_HALF))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(type_size(BODY_SIZE))
                                    .text_color(rgb(TEXT))
                                    .child(title),
                            )
                            .when_some(subtitle, |s, subtitle| {
                                s.child(
                                    div()
                                        .truncate()
                                        .text_size(type_size(CAPTION_SIZE))
                                        .text_color(rgb(TEXT_SECONDARY))
                                        .child(subtitle),
                                )
                            }),
                    )
                    .when_some(trailing, |s, trailing| s.child(trailing))
            },
            action,
            cx,
        )
    }
}

/// The content card and other bordered panels on the shell canvas.
pub fn panel() -> Div {
    column()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded(px(PANEL_RADIUS))
}

/// A raised card that groups related content on a page.
pub fn card() -> Div {
    column()
        .bg(rgb(SURFACE_RAISED))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded(px(RADIUS_LG))
        .p(px(SPACE_4))
        .gap(px(SPACE_3))
}

/// A hairline between stacked content.
pub fn divider() -> Div {
    div().w_full().h(px(1.)).bg(rgb(BORDER_SUBTLE))
}

/// The bar inside the bottom of the content card.
pub fn status_bar() -> Div {
    row()
        .debug_selector(|| "status-bar".into())
        .justify_between()
        .h(px(STATUS_BAR_HEIGHT))
        .flex_shrink_0()
        .px(px(PAGE_X))
        .gap(px(SPACE_3))
        .border_t_1()
        .border_color(rgb(BORDER_SUBTLE))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(TEXT_SECONDARY))
}
