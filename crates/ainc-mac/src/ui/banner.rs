//! Inline notices: a toned banner with an icon, and plain error text.
use super::{badge::Tone, display::icon, layout::row, tokens::*};
use gpui::{prelude::*, *};

/// A full-width notice inside a page. Danger banners report failures; info
/// and warning banners explain a limitation the user can do nothing about.
pub fn banner(tone: Tone, text: impl Into<SharedString>) -> Div {
    let (foreground, border, surface, glyph) = match tone {
        Tone::Danger => (DESTRUCTIVE_TEXT, ERROR_BORDER, SURFACE_ERROR, "warning"),
        Tone::Warning => (STATUS_AMBER, BORDER, SURFACE_RAISED, "warning"),
        _ => (TEXT, BORDER, SURFACE_RAISED, "info"),
    };
    row()
        .w_full()
        .gap(px(SPACE_3))
        .px(px(SPACE_4))
        .py(px(SPACE_3))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(surface))
        .text_size(type_size(LABEL_SIZE))
        .text_color(rgb(foreground))
        .child(icon(glyph, ICON_SIZE).text_color(rgb(foreground)))
        .child(div().flex_1().min_w_0().child(text.into()))
}

/// Short error copy under a field or inside a dialog.
pub fn error_text(text: impl Into<SharedString>) -> Div {
    row()
        .gap(px(SPACE_2))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(ERROR))
        .child(icon("warning", ICON_SIZE_SM).text_color(rgb(ERROR)))
        .child(text.into())
}
