//! Buttons: one focus, disabled and keyboard contract behind four looks.
use super::{display::icon, layout::row, motion::*, tokens::*};
use gpui::{prelude::*, *};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The one white action on a surface.
    Primary,
    /// A bordered gray action beside a primary one.
    Secondary,
    /// A text action that only shows its surface on hover.
    Ghost,
    /// A solid red action that removes something.
    Destructive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Regular,
    Large,
}

impl ButtonSize {
    fn height(self) -> f32 {
        match self {
            Self::Small => CONTROL_HEIGHT_SM,
            Self::Regular => CONTROL_HEIGHT,
            Self::Large => CONTROL_HEIGHT_LG,
        }
    }
    fn inset(self) -> f32 {
        match self {
            Self::Small => CONTROL_INSET_X_SM,
            Self::Regular => CONTROL_INSET_X,
            Self::Large => SPACE_4,
        }
    }
    fn text(self) -> f32 {
        match self {
            Self::Small => CAPTION_SIZE,
            Self::Regular => LABEL_SIZE,
            Self::Large => BODY_SIZE,
        }
    }
    fn icon(self) -> f32 {
        match self {
            Self::Small => ICON_SIZE_SM,
            _ => ICON_SIZE,
        }
    }
}

pub struct ButtonSpec {
    pub id: ElementId,
    pub label: SharedString,
    pub enabled: bool,
}

/// The accessibility toggled state for switches, checkboxes, radios and tabs.
pub fn toggled(on: bool) -> accesskit::Toggled {
    if on {
        accesskit::Toggled::True
    } else {
        accesskit::Toggled::False
    }
}

/// Common focus, activation and disabled contract; callers own layout and looks.
pub fn button_base(spec: ButtonSpec) -> Stateful<Div> {
    // Only authored names/business keys become public IDs, never allocation IDs.
    let author_id = match &spec.id {
        ElementId::Name(name) => Some(name.clone()),
        ElementId::NamedInteger(name, key) => Some(format!("{name}.{key}").into()),
        _ => None,
    };
    // The same authored id names the control for accessibility and for tests.
    let selector = author_id.clone();
    row()
        .id(spec.id)
        .role(accesskit::Role::Button)
        .aria_label(spec.label)
        .when_some(author_id, |s, id| s.accessibility_id(id))
        .when_some(selector, |s, id| s.debug_selector(move || id.to_string()))
        .a11y_synthetic_children(move |builder| {
            if !spec.enabled {
                builder.parent_node().set_disabled();
            }
        })
        .when(spec.enabled, |s| s.tab_index(0).cursor_pointer())
        .rounded(px(CONTROL_RADIUS))
        .when(!spec.enabled, |s| s.opacity(DISABLED_OPACITY))
        .when(focus_visible(), |s| s.focus(|s| s.shadow(focus_ring())))
}

pub fn action_button<V: 'static>(
    spec: ButtonSpec,
    decorate: impl FnOnce(Stateful<Div>) -> Stateful<Div>,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let enabled = spec.enabled;
    let key_action = action.clone();
    decorate(button_base(spec))
        .on_click(cx.listener(move |view, _, window, cx| {
            cx.stop_propagation();
            if enabled {
                action(view, window, cx);
            }
        }))
        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                cx.stop_propagation();
                if enabled {
                    key_action(view, window, cx);
                }
            }
        }))
}

/// The resolved surfaces for one button look.
struct Look {
    base: u32,
    hover: u32,
    active: u32,
    text: u32,
    text_hover: u32,
    border: Option<u32>,
}

impl ButtonKind {
    fn look(self, selected: bool) -> Look {
        match self {
            Self::Primary => Look {
                base: PRIMARY,
                hover: PRIMARY_HOVER,
                active: PRIMARY_ACTIVE,
                text: TEXT_ON_PRIMARY,
                text_hover: TEXT_ON_PRIMARY,
                border: None,
            },
            Self::Secondary => Look {
                base: if selected {
                    SELECTED_STRONG
                } else {
                    SURFACE_CONTROL
                },
                hover: HOVER_STRONG,
                active: ACTIVE,
                text: TEXT,
                text_hover: TEXT,
                border: Some(if selected { BORDER_STRONG } else { BORDER }),
            },
            // Ghost rests transparent; its hover surface is painted as an alpha
            // overlay in `build`, so it sits on any surface without a rectangle.
            Self::Ghost => Look {
                base: SELECTED,
                hover: HOVER_STRONG,
                active: ACTIVE,
                text: TEXT,
                text_hover: TEXT,
                border: None,
            },
            Self::Destructive => Look {
                base: SURFACE_ERROR,
                hover: DESTRUCTIVE_HOVER,
                active: DESTRUCTIVE_HOVER,
                text: DESTRUCTIVE_TEXT,
                text_hover: DESTRUCTIVE_TEXT,
                border: Some(ERROR_BORDER),
            },
        }
    }
}

/// A button with the shared contract and one of the four looks.
pub struct Button {
    id: ElementId,
    label: SharedString,
    kind: ButtonKind,
    size: ButtonSize,
    icon: Option<&'static str>,
    icon_only: bool,
    enabled: bool,
    selected: bool,
    full_width: bool,
    focus: Option<FocusHandle>,
    trailing: Option<AnyElement>,
    align_start: bool,
    tint: Option<u32>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: ButtonKind::Secondary,
            size: ButtonSize::Regular,
            icon: None,
            icon_only: false,
            enabled: true,
            selected: false,
            full_width: false,
            focus: None,
            trailing: None,
            align_start: false,
            tint: None,
        }
    }
    /// A square control that shows only its icon; the label is still spoken.
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }
    /// Colors the label and icon of a ghost or secondary button, for
    /// destructive actions that are not the primary one.
    pub fn tint(mut self, color: u32) -> Self {
        self.tint = Some(color);
        self
    }
    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }
    pub fn primary(self) -> Self {
        self.kind(ButtonKind::Primary)
    }
    pub fn secondary(self) -> Self {
        self.kind(ButtonKind::Secondary)
    }
    pub fn ghost(self) -> Self {
        self.kind(ButtonKind::Ghost)
    }
    pub fn destructive(self) -> Self {
        self.kind(ButtonKind::Destructive)
    }
    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }
    pub fn small(self) -> Self {
        self.size(ButtonSize::Small)
    }
    pub fn large(self) -> Self {
        self.size(ButtonSize::Large)
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
    /// A trailing element such as a shortcut hint or a chevron.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }
    /// Lead with the label and push any trailing element to the far edge, as
    /// menu items and select triggers do.
    pub fn align_start(mut self) -> Self {
        self.align_start = true;
        self
    }

    /// Builds the control. `hover` is the host view's fade state, read here so
    /// the surface can blend toward its hover look between frames.
    pub fn build<V: HoverHost>(
        self,
        hover: &HoverFade,
        action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
        cx: &mut Context<V>,
    ) -> Stateful<Div> {
        let Self {
            id,
            label,
            kind,
            size,
            icon: icon_name,
            icon_only,
            enabled,
            selected,
            full_width,
            focus,
            trailing,
            align_start,
            tint,
        } = self;
        let mut look = kind.look(selected);
        if !enabled && kind == ButtonKind::Primary {
            // A disabled primary is an outline, not a gray slab.
            look = Look {
                base: SURFACE_CONTROL,
                hover: SURFACE_CONTROL,
                active: SURFACE_CONTROL,
                text: TEXT_TERTIARY,
                text_hover: TEXT_TERTIARY,
                border: Some(BORDER),
            };
        }
        if icon_only && kind == ButtonKind::Secondary && !selected {
            look.base = SURFACE;
        }
        let (progress, on_hover) = hover.track(&id, enabled, cx);
        let text_color = match tint {
            Some(color) => rgb(color),
            None => blend(look.text, look.text_hover, progress),
        };
        let filled = matches!(kind, ButtonKind::Primary | ButtonKind::Destructive);
        action_button(
            ButtonSpec {
                id,
                label: label.clone(),
                enabled,
            },
            |button| {
                button
                    .h(px(size.height()))
                    .flex_shrink_0()
                    .when(icon_only, |s| s.w(px(size.height())))
                    .when(!icon_only, |s| s.px(px(size.inset())))
                    .when(full_width, |s| s.w_full())
                    .when(!align_start, |s| s.justify_center())
                    .gap(px(if size == ButtonSize::Small {
                        6.
                    } else {
                        CONTROL_GAP
                    }))
                    .text_size(type_size(size.text()))
                    .when(filled, |s| s.font_weight(FontWeight::MEDIUM))
                    .text_color(text_color)
                    .bg(if kind == ButtonKind::Ghost && !selected {
                        rgba((look.hover << 8) | (progress * 255.) as u32)
                    } else {
                        blend(look.base, look.hover, progress)
                    })
                    .when_some(look.border, |s, border| {
                        s.border_1().border_color(rgb(border))
                    })
                    .when(enabled, |s| s.active(move |s| s.bg(rgb(look.active))))
                    .when(!enabled && kind == ButtonKind::Primary, |s| s.opacity(1.))
                    .on_hover(on_hover)
                    .when_some(focus, |s, focus| s.track_focus(&focus))
                    .when_some(icon_name, |s, name| {
                        s.child(icon(name, size.icon()).text_color(text_color))
                    })
                    .when(!icon_only, |s| {
                        s.child(
                            div()
                                .when(align_start, |s| s.flex_1().min_w_0().truncate())
                                .when(!align_start, |s| s.whitespace_nowrap())
                                .child(label),
                        )
                    })
                    .when_some(trailing, |s, trailing| s.child(trailing))
            },
            action,
            cx,
        )
    }
}
