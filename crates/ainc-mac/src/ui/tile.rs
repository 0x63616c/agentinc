//! Glanceable controls: switch tiles that turn white when on, steppers for a
//! bounded number, and hero numerals.
use super::{button::*, display::icon, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

/// A large switch for one light or group: white with black ink when on, a
/// quiet raised surface when off, and outlined when a group is partly on.
pub struct SwitchTile {
    id: ElementId,
    label: SharedString,
    glyph: &'static str,
    on: bool,
    mixed: bool,
    pending: bool,
    enabled: bool,
    detail: Option<SharedString>,
}
impl SwitchTile {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        glyph: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            glyph,
            on: false,
            mixed: false,
            pending: false,
            enabled: true,
            detail: None,
        }
    }
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    /// Some, not all, of a group's lights are on.
    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }
    /// A change is travelling to the lights.
    pub fn pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    /// Replaces the On / Off line, such as "2 of 4 on".
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
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
            label,
            glyph,
            on,
            mixed,
            pending,
            enabled,
            detail,
        } = self;
        let (progress, on_hover) = hover.track(&id, enabled, cx);
        // A light still on its way reads as outlined, not yet lit.
        let lit = on && !pending;
        let (surface, surface_hover, ink, secondary) = if lit {
            (
                PRIMARY,
                PRIMARY_HOVER,
                TEXT_ON_PRIMARY,
                TEXT_ON_PRIMARY_SECONDARY,
            )
        } else {
            (SURFACE_RAISED, HOVER_STRONG, TEXT, TEXT_SECONDARY)
        };
        let state: SharedString = match (on, pending, detail) {
            (true, true, _) => "Turning on…".into(),
            (false, true, _) => "Turning off…".into(),
            (_, false, Some(detail)) => detail,
            (true, false, None) => "On".into(),
            (false, false, None) => "Off".into(),
        };
        action_button(
            ButtonSpec {
                id,
                label: label.clone(),
                enabled,
            },
            |button| {
                button
                    .role(accesskit::Role::Switch)
                    .aria_toggled(toggled(on))
                    .flex_col()
                    .items_start()
                    .justify_between()
                    .flex_1()
                    .min_w(px(TILE_MIN_WIDTH))
                    .h(px(TILE_HEIGHT))
                    .p(px(CARD_INSET))
                    .rounded(px(RADIUS_LG))
                    .border_1()
                    .border_color(rgb(if on || pending {
                        PRIMARY
                    } else if mixed {
                        FOCUS_FIELD
                    } else {
                        BORDER
                    }))
                    .bg(blend(surface, surface_hover, progress))
                    .on_hover(on_hover)
                    .child(icon(glyph, ICON_SIZE_LG).text_color(rgb(if mixed {
                        TEXT
                    } else {
                        ink
                    })))
                    .child(
                        column()
                            .w_full()
                            .gap(px(SPACE_HALF))
                            .child(
                                div()
                                    .w_full()
                                    .truncate()
                                    .text_size(type_size(BODY_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(ink))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_size(type_size(CAPTION_SIZE))
                                    .text_color(rgb(secondary))
                                    .when(pending, |s| s.opacity(0.7))
                                    .child(state),
                            ),
                    )
            },
            action,
            cx,
        )
    }
}

/// A bounded number with − and + beside it, such as a thermostat target.
#[allow(clippy::too_many_arguments)]
pub fn stepper<V: HoverHost>(
    id: &str,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    can_lower: bool,
    can_raise: bool,
    hover: &HoverFade,
    on_step: impl Fn(&mut V, i64, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Div {
    let label = label.into();
    let lower = on_step.clone();
    row()
        .debug_selector({
            let id = id.to_owned();
            move || id.clone()
        })
        .gap(px(SPACE_1))
        .child(
            Button::new(
                SharedString::from(format!("{id}.lower")),
                format!("Lower {label}"),
            )
            .secondary()
            .icon("minus")
            .icon_only()
            .enabled(can_lower)
            .build(
                hover,
                move |view, window, cx| lower(view, -1, window, cx),
                cx,
            ),
        )
        .child(
            div()
                .w(px(STEPPER_VALUE_WIDTH))
                .flex()
                .justify_center()
                .text_size(type_size(TITLE_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(TEXT))
                .child(value.into()),
        )
        .child(
            Button::new(
                SharedString::from(format!("{id}.raise")),
                format!("Raise {label}"),
            )
            .secondary()
            .icon("plus")
            .icon_only()
            .enabled(can_raise)
            .build(
                hover,
                move |view, window, cx| on_step(view, 1, window, cx),
                cx,
            ),
        )
}

/// A glanceable numeral set light and large.
pub fn hero(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(type_size(HERO_SIZE))
        .line_height(relative(1.))
        .font_weight(FontWeight::LIGHT)
        .text_color(rgb(TEXT))
        .child(text.into())
}
