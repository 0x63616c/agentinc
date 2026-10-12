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
            Self::Neutral => (theme().status_neutral, theme().status_neutral_surface),
            Self::Success => (theme().status_green, theme().status_green_surface),
            Self::Info => (theme().status_blue, theme().status_blue_surface),
            Self::Warning => (theme().status_amber, theme().status_amber_surface),
            Self::Danger => (theme().status_red, theme().status_red_surface),
            Self::Accent => (theme().status_purple, theme().status_purple_surface),
        }
    }
}

/// A filled pill for a state, count or category.
pub fn badge(label: impl Into<SharedString>, tone: Tone) -> Div {
    let (foreground, surface) = tone.colors();
    row()
        .h(px(PILL_HEIGHT))
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
        .size(px(STATUS_DOT_SIZE))
        .flex_shrink_0()
        .rounded_full()
        .bg(rgb(tone.colors().0))
}

/// A bordered pill with a dot: the quieter status treatment for tables.
pub fn status_pill(label: impl Into<SharedString>, tone: Tone) -> Div {
    row()
        .h(px(PILL_HEIGHT))
        .px(px(SPACE_2))
        .gap(px(PILL_GAP))
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(rgb(theme().border_strong))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(theme().text))
        .whitespace_nowrap()
        .child(status_dot(tone))
        .child(label.into())
}

/// A label: a bordered pill with a dot of the label's own color.
pub fn tag(label: impl Into<SharedString>, color: u32) -> Div {
    row()
        .h(px(PILL_HEIGHT))
        .px(px(SPACE_2))
        .gap(px(PILL_GAP))
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(rgb(theme().border_strong))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(theme().text))
        .whitespace_nowrap()
        .child(
            div()
                .size(px(STATUS_DOT_SIZE))
                .flex_shrink_0()
                .rounded_full()
                .bg(rgb(color)),
        )
        .child(label.into())
}

/// A white count bubble, for unread items.
pub fn count_badge(count: usize) -> Div {
    row()
        .h(px(COUNT_BADGE_SIZE))
        .min_w(px(COUNT_BADGE_SIZE))
        .px(px(KBD_INSET_X))
        .justify_center()
        .flex_shrink_0()
        .rounded_full()
        .bg(rgb(theme().selected_strong))
        .text_size(type_size(MICRO_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme().text))
        .child(count.to_string())
}
