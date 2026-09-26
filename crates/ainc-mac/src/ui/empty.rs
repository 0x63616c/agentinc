//! Empty states: an icon, a short title, one line of guidance, one action.
use super::{display::icon, layout::*, tokens::*};
use gpui::{prelude::*, *};

pub struct EmptyState {
    icon: &'static str,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
    selector: Option<&'static str>,
}

impl EmptyState {
    pub fn new(icon: &'static str, title: impl Into<SharedString>) -> Self {
        Self {
            icon,
            title: title.into(),
            description: None,
            action: None,
            selector: None,
        }
    }
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
    pub fn selector(mut self, selector: &'static str) -> Self {
        self.selector = Some(selector);
        self
    }
    pub fn build(self) -> Div {
        column()
            .when_some(self.selector, |s, selector| {
                s.debug_selector(move || selector.into())
            })
            .w_full()
            .items_center()
            .justify_center()
            .py(px(SPACE_10))
            .px(px(SPACE_6))
            .gap(px(SPACE_3))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .child(
                row()
                    .size(px(40.))
                    .justify_center()
                    .rounded(px(RADIUS_MD))
                    .bg(rgb(SURFACE_RAISED))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(icon(self.icon, ICON_SIZE_LG)),
            )
            .child(
                column()
                    .items_center()
                    .gap(px(SPACE_1))
                    .child(
                        div()
                            .text_size(type_size(HEADING_SIZE))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.title),
                    )
                    .when_some(self.description, |s, description| {
                        s.child(
                            div()
                                .max_w(px(440.))
                                .text_align(TextAlign::Center)
                                .text_size(type_size(LABEL_SIZE))
                                .text_color(rgb(TEXT_SECONDARY))
                                .child(description),
                        )
                    }),
            )
            .when_some(self.action, |s, action| {
                s.child(div().mt(px(SPACE_1)).child(action))
            })
    }
}
