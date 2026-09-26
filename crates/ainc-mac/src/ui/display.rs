//! Page headings, icons, keyboard hints and the embedded asset catalogue.
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
            .mt(px(-TITLE_OPTICAL_LIFT))
            .gap(px(SPACE_1))
            .child(
                row()
                    .w_full()
                    .min_h(px(CONTROL_HEIGHT))
                    .justify_between()
                    .gap(px(SPACE_4))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .debug_selector(|| "page-title".into())
                            .text_size(type_size(DISPLAY_SIZE))
                            // The title shares the control row's line box, so its cap
                            // height lands on `PAGE_X` and actions centre on it exactly.
                            .line_height(px(CONTROL_HEIGHT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.title),
                    )
                    .when_some(self.actions, |s, actions| s.child(actions)),
            )
            .when_some(self.description, |s, description| {
                s.child(
                    div()
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(TEXT_SECONDARY))
                        .child(description),
                )
            })
    }
}

/// A small uppercase section label.
pub fn eyebrow(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(type_size(CAPTION_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(TEXT_TERTIARY))
        .child(text.into().to_uppercase())
}

/// A heading inside a page, above a card or list.
pub fn heading(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(type_size(HEADING_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .child(text.into())
}

/// Secondary body copy.
pub fn caption(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(TEXT_SECONDARY))
        .child(text.into())
}

pub fn icon(name: &'static str, size: f32) -> Svg {
    svg()
        .path(format!("{name}.svg"))
        .size(px(size))
        .text_color(rgb(TEXT_SECONDARY))
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

/// A keyboard shortcut hint: `⌘K`, `↵`, `esc`.
pub fn kbd(label: impl Into<SharedString>) -> Div {
    row()
        .h(px(20.))
        .min_w(px(20.))
        .px(px(5.))
        .justify_center()
        .flex_shrink_0()
        .rounded(px(RADIUS_XS))
        .bg(rgb(SURFACE_CONTROL))
        .border_1()
        .border_color(rgb(BORDER))
        .text_size(type_size(MICRO_SIZE))
        .line_height(relative(1.))
        .text_color(rgb(TEXT_SECONDARY))
        .whitespace_nowrap()
        .child(label.into())
}

/// A row of hints such as `↑↓ Navigate  ↵ Open`.
pub fn kbd_hint(keys: impl Into<SharedString>, action: impl Into<SharedString>) -> Div {
    row()
        .gap(px(6.))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(TEXT_TERTIARY))
        .child(kbd(keys))
        .child(action.into())
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
            "check.svg" => include_bytes!("../../assets/icons/check.svg"),
            "chevronDown.svg" => include_bytes!("../../assets/icons/chevronDown.svg"),
            "chevronUpDown.svg" => include_bytes!("../../assets/icons/chevronUpDown.svg"),
            "more.svg" => include_bytes!("../../assets/icons/more.svg"),
            "user.svg" => include_bytes!("../../assets/icons/user.svg"),
            "users.svg" => include_bytes!("../../assets/icons/users.svg"),
            "help.svg" => include_bytes!("../../assets/icons/help.svg"),
            "feedback.svg" => include_bytes!("../../assets/icons/feedback.svg"),
            "download.svg" => include_bytes!("../../assets/icons/download.svg"),
            "inbox.svg" => include_bytes!("../../assets/icons/inbox.svg"),
            "warning.svg" => include_bytes!("../../assets/icons/warning.svg"),
            "info.svg" => include_bytes!("../../assets/icons/info.svg"),
            "copy.svg" => include_bytes!("../../assets/icons/copy.svg"),
            "trash.svg" => include_bytes!("../../assets/icons/trash.svg"),
            "edit.svg" => include_bytes!("../../assets/icons/edit.svg"),
            "play.svg" => include_bytes!("../../assets/icons/play.svg"),
            "pause.svg" => include_bytes!("../../assets/icons/pause.svg"),
            "stop.svg" => include_bytes!("../../assets/icons/stop.svg"),
            "history.svg" => include_bytes!("../../assets/icons/history.svg"),
            "command.svg" => include_bytes!("../../assets/icons/command.svg"),
            "send.svg" => include_bytes!("../../assets/send.svg"),
            "openai.svg" => include_bytes!("../../assets/openai.svg"),
            "agents.svg" => include_bytes!("../../assets/agents.svg"),
            "arrowRight.svg" => include_bytes!("../../assets/arrowRight.svg"),
            "arrowUpRight.svg" => include_bytes!("../../assets/arrowUpRight.svg"),
            "bell.svg" => include_bytes!("../../assets/bell.svg"),
            "chevronLeft.svg" => include_bytes!("../../assets/chevronLeft.svg"),
            "chevronRight.svg" => include_bytes!("../../assets/chevronRight.svg"),
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
