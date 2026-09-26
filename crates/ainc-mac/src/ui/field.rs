//! Shared chrome around the native text editing engine.
use super::{
    display::{caption, icon},
    layout::{column_gap, row},
    tokens::*,
};
use crate::input::TextInput;
use gpui::{prelude::*, *};

/// A labeled text field or text area with hint, error and focus states.
pub struct Field {
    input: Entity<TextInput>,
    label: Option<&'static str>,
    hint: Option<SharedString>,
    error: Option<SharedString>,
    leading: Option<&'static str>,
    selector: Option<&'static str>,
    multiline: bool,
}

impl Field {
    pub fn new(input: Entity<TextInput>) -> Self {
        Self {
            input,
            label: None,
            hint: None,
            error: None,
            leading: None,
            selector: None,
            multiline: false,
        }
    }
    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        if self.selector.is_none() {
            self.selector = Some(label);
        }
        self
    }
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn error(mut self, error: Option<impl Into<SharedString>>) -> Self {
        self.error = error.map(Into::into);
        self
    }
    pub fn leading_icon(mut self, name: &'static str) -> Self {
        self.leading = Some(name);
        self
    }
    pub fn selector(mut self, selector: &'static str) -> Self {
        self.selector = Some(selector);
        self
    }
    /// A text area: taller, top-aligned and growing with its content.
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    pub fn build(self, window: &Window, cx: &App) -> Div {
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let selector = self.selector;
        let has_error = self.error.is_some();
        let border = if has_error {
            ERROR_BORDER
        } else if focused {
            FOCUS_FIELD
        } else {
            BORDER
        };
        column_gap(FIELD_LABEL_GAP)
            .when_some(self.label, |s, label| {
                s.child(
                    div()
                        .when_some(selector, |s, selector| {
                            s.debug_selector(move || format!("{selector}.label"))
                        })
                        .text_color(rgb(TEXT_SECONDARY))
                        .text_size(type_size(LABEL_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
            })
            .child(
                row()
                    .when_some(selector, |s, selector| {
                        s.debug_selector(move || format!("{selector}.input"))
                    })
                    .w_full()
                    .min_h(px(FIELD_HEIGHT))
                    .when(self.multiline, |s| {
                        s.items_start().min_h(px(96.)).py(px(SPACE_2))
                    })
                    .px(px(FIELD_INSET_X))
                    .gap(px(SPACE_2))
                    .bg(rgb(SURFACE_INPUT))
                    .border_1()
                    .border_color(rgb(border))
                    .rounded(px(FIELD_RADIUS))
                    .when_some(self.leading, |s, name| s.child(icon(name, ICON_SIZE_SM)))
                    .child(div().flex_1().min_w_0().child(self.input)),
            )
            .when_some(self.error, |s, error| {
                s.child(
                    div()
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(ERROR))
                        .child(error),
                )
            })
            .when(!has_error, |s| {
                s.when_some(self.hint, |s, hint| s.child(caption(hint)))
            })
    }
}

/// The labeled single-line field used by Ticket and Automation forms.
pub fn text_field(label: &'static str, input: Entity<TextInput>, window: &Window, cx: &App) -> Div {
    Field::new(input).label(label).build(window, cx)
}
