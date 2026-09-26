//! Shared page heading, marks, and small identity pieces.
use super::{
    layout::{column, row},
    tokens::*,
};
use gpui::{prelude::*, *};
use std::borrow::Cow;

/// Shared heading for a page's title, short description, and title-row actions.
pub struct PageHeader {
    title: SharedString,
    description: Option<SharedString>,
    actions: Option<AnyElement>,
}

impl PageHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            actions: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }

    pub fn build(self) -> Div {
        column()
            .mt(px(-5.))
            .gap(px(6.))
            .child(
                row()
                    .w_full()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .debug_selector(|| "page-title".into())
                            .text_size(type_size(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.title),
                    )
                    .when_some(self.actions, |s, actions| s.child(actions)),
            )
            .when_some(self.description, |s, description| {
                s.child(
                    div()
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(MUTED))
                        .child(description),
                )
            })
    }
}

pub fn icon(name: &'static str, size: f32) -> Svg {
    svg()
        .path(format!("{name}.svg"))
        .size(px(size))
        .text_color(rgb(MUTED))
        .flex_shrink_0()
}
pub fn nav_icon(
    name: &'static str,
    selected: bool,
    tint: u32,
    hover_group: impl Into<SharedString>,
) -> Svg {
    svg()
        .path(if selected {
            format!("selected/{name}.svg")
        } else {
            format!("{name}.svg")
        })
        .size(px(17.))
        .text_color(rgb(tint))
        .group_hover(hover_group, |s| s.text_color(rgb(TEXT)))
        .flex_shrink_0()
}
pub fn shortcut_badge(label: impl Into<SharedString>) -> Div {
    row()
        .min_h(type_size(20.))
        .min_w(px(20.))
        .px(px(4.))
        .justify_center()
        .rounded(px(4.))
        .bg(rgb(PRIMARY_INK))
        .text_size(type_size(10.))
        .text_color(rgb(MUTED))
        .child(label.into())
}
pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path.strip_prefix("selected/") {
            return self.load(name).map(|data| {
                data.map(|data| {
                    Cow::Owned(
                        String::from_utf8_lossy(&data)
                            .replace("stroke-width=\"1.5\"", "stroke-width=\"2\"")
                            .into_bytes(),
                    )
                })
            });
        }
        let data: &'static [u8] = match path {
            "refresh.svg" => include_bytes!("../../assets/icons/refresh.svg"),
            "temporal.svg" => include_bytes!("../../assets/icons/temporal.svg"),
            "send.svg" => include_bytes!("../../assets/send.svg"),
            "openai.svg" => include_bytes!("../../assets/openai.svg"),
            "agents.svg" => include_bytes!("../../assets/agents.svg"),
            "arrowRight.svg" => include_bytes!("../../assets/arrowRight.svg"),
            "arrowUpRight.svg" => include_bytes!("../../assets/arrowUpRight.svg"),
            "bell.svg" => include_bytes!("../../assets/bell.svg"),
            "chevronLeft.svg" => include_bytes!("../../assets/chevronLeft.svg"),
            "chevronRight.svg" => include_bytes!("../../assets/chevronRight.svg"),
            "chevronDown.svg" => include_bytes!("../../assets/chevronDown.svg"),
            "close.svg" => include_bytes!("../../assets/close.svg"),
            "evee.png" => include_bytes!("../../assets/evee.png"),
            "evee-outline.svg" => include_bytes!("../../assets/evee-outline.svg"),
            "panel.svg" => include_bytes!("../../assets/panel.svg"),
            "plus.svg" => include_bytes!("../../assets/plus.svg"),
            "search.svg" => include_bytes!("../../assets/search.svg"),
            "settings.svg" => include_bytes!("../../assets/settings.svg"),
            "spark.svg" => include_bytes!("../../assets/spark.svg"),
            "tasks.svg" => include_bytes!("../../assets/tasks.svg"),
            "terminal.svg" => include_bytes!("../../assets/terminal.svg"),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(data)))
    }
    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
