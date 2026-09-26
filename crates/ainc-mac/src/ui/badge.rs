//! Badges and status pills.
use super::{layout::row, tokens::*};
use gpui::{prelude::*, *};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Success,
    Info,
    Warning,
    Danger,
    Accent,
}

impl Tone {
    pub fn colors(self) -> (u32, u32) {
        match self {
            Self::Neutral => (STATUS_NEUTRAL, STATUS_NEUTRAL_SURFACE),
            Self::Success => (STATUS_GREEN, STATUS_GREEN_SURFACE),
            Self::Info => (STATUS_BLUE, STATUS_BLUE_SURFACE),
            Self::Warning => (STATUS_AMBER, STATUS_AMBER_SURFACE),
            Self::Danger => (STATUS_RED, STATUS_RED_SURFACE),
            Self::Accent => (STATUS_PURPLE, STATUS_PURPLE_SURFACE),
        }
    }
}

/// A filled pill for a state, count or category.
pub fn badge(label: impl Into<SharedString>, tone: Tone) -> Div {
    let (foreground, surface) = tone.colors();
    row()
        .h(px(22.))
        .px(px(SPACE_2))
        .flex_shrink_0()
        .rounded_full()
        .bg(rgb(surface))
        .text_size(type_size(CAPTION_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(foreground))
        .whitespace_nowrap()
        .child(label.into())
}

/// A small colored dot.
pub fn status_dot(tone: Tone) -> Div {
    div()
        .size(px(6.))
        .flex_shrink_0()
        .rounded_full()
        .bg(rgb(tone.colors().0))
}

/// A bordered pill with a dot: the quieter status treatment for tables.
pub fn status_pill(label: impl Into<SharedString>, tone: Tone) -> Div {
    row()
        .h(px(22.))
        .px(px(SPACE_2))
        .gap(px(6.))
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(rgb(BORDER_STRONG))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(TEXT))
        .whitespace_nowrap()
        .child(status_dot(tone))
        .child(label.into())
}

/// A label: a bordered pill with a dot of the label's own color.
pub fn tag(label: impl Into<SharedString>, color: u32) -> Div {
    row()
        .h(px(PILL_HEIGHT))
        .px(px(SPACE_2))
        .gap(px(6.))
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(rgb(BORDER_STRONG))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(TEXT))
        .whitespace_nowrap()
        .child(
            div()
                .size(px(6.))
                .flex_shrink_0()
                .rounded_full()
                .bg(rgb(color)),
        )
        .child(label.into())
}

/// A white count bubble, for unread items.
pub fn count_badge(count: usize) -> Div {
    row()
        .h(px(18.))
        .min_w(px(18.))
        .px(px(5.))
        .justify_center()
        .flex_shrink_0()
        .rounded_full()
        .bg(rgb(SELECTED_STRONG))
        .text_size(type_size(MICRO_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(TEXT))
        .child(count.to_string())
}
