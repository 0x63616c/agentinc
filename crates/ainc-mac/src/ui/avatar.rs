//! Avatars with an initials fallback.
use super::{layout::row, tokens::*};
use gpui::{prelude::*, *};
use std::sync::Arc;

pub fn initials(name: &str) -> String {
    let mut letters: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    if letters.is_empty() {
        letters.push('?');
    }
    letters
}

pub fn avatar(name: &str, photo: Option<Arc<Image>>, size: f32) -> AnyElement {
    match photo {
        Some(photo) => img(photo)
            .size(px(size))
            .flex_shrink_0()
            .rounded_full()
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        None => row()
            .size(px(size))
            .flex_shrink_0()
            .justify_center()
            .rounded_full()
            .bg(rgb(SURFACE_CONTROL))
            .border_1()
            .border_color(rgb(BORDER_STRONG))
            .text_size(px((size * 0.4).round()))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(TEXT))
            .child(initials(name))
            .into_any_element(),
    }
}

/// An agent: the same initials on a rounded square, so agents and people read
/// apart at a glance wherever they appear together.
pub fn agent_avatar(name: &str, size: f32) -> AnyElement {
    row()
        .size(px(size))
        .flex_shrink_0()
        .justify_center()
        .rounded(px((size * 0.3).round()))
        .bg(rgb(SURFACE_CONTROL))
        .border_1()
        .border_color(rgb(BORDER_STRONG))
        .text_size(px((size * 0.4).round()))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(TEXT))
        .child(initials(name))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::initials;

    #[test]
    fn initials_take_two_words_and_fall_back() {
        assert_eq!(initials("Calum Webb"), "CW");
        assert_eq!(initials("calum"), "C");
        assert_eq!(initials("  "), "?");
        assert_eq!(initials("élan vital"), "ÉV");
    }
}
