//! Dropdown and context menus: floating placement, rows, labels and dividers.
use super::{button::*, display::*, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

/// Floats `content` above everything else, anchored to where it is placed.
/// Place it inside a `column().relative()` wrapper so its origin is the
/// wrapper's top-left corner rather than the slot after its siblings.
pub fn floating(content: impl IntoElement, anchor: Anchor, offset: Point<Pixels>) -> Deferred {
    // Flip to the other side when there is no room, then snap to the window edge.
    deferred(anchored().anchor(anchor).offset(offset).child(content)).with_priority(1)
}

/// One row inside a dropdown or context menu.
pub struct MenuEntry {
    id: ElementId,
    label: SharedString,
    icon: Option<&'static str>,
    glyph: Option<(&'static str, u32)>,
    leading: Option<AnyElement>,
    shortcut: Option<SharedString>,
    trailing: Option<AnyElement>,
    checked: bool,
    destructive: bool,
    enabled: bool,
    selector: Option<String>,
}

impl MenuEntry {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            glyph: None,
            leading: None,
            shortcut: None,
            trailing: None,
            checked: false,
            destructive: false,
            enabled: true,
            selector: None,
        }
    }
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
    /// A colored leading icon, such as a Ticket status.
    pub fn glyph(mut self, name: &'static str, color: u32) -> Self {
        self.glyph = Some((name, color));
        self
    }
    /// A leading element such as an avatar, in place of an icon.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
    /// A chevron or other trailing mark, for submenus.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn selector(mut self, selector: impl Into<String>) -> Self {
        self.selector = Some(selector.into());
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
            icon: icon_name,
            glyph,
            leading,
            shortcut,
            trailing,
            checked,
            destructive,
            enabled,
            selector,
        } = self;
        let mut button = Button::new(id, label)
            .ghost()
            .full_width()
            .align_start()
            .enabled(enabled);
        if destructive {
            button = button.tint(DESTRUCTIVE_TEXT);
        }
        if let Some(name) = icon_name {
            button = button.icon(name);
        }
        if let Some((name, color)) = glyph {
            button = button.leading(icon(name, ICON_SIZE_SM).text_color(rgb(color)));
        } else if let Some(leading) = leading {
            button = button.leading(leading);
        }
        let mut trail = row().gap(px(SPACE_2));
        if let Some(shortcut) = shortcut {
            trail = trail.child(kbd(shortcut));
        }
        if checked {
            trail = trail.child(icon("check", ICON_SIZE_SM).text_color(rgb(TEXT)));
        }
        if let Some(trailing) = trailing {
            trail = trail.child(trailing);
        }
        button
            .trailing(trail)
            .build(hover, action, cx)
            .h(px(MENU_ITEM_HEIGHT))
            .when_some(selector, |s, selector| {
                s.debug_selector(move || selector.clone())
            })
    }
}

/// A toolbar button that opens a menu below it, left edges aligned, such as a
/// filter. It reads as chosen while its menu is open or a value is active.
pub struct MenuButton {
    id: &'static str,
    label: SharedString,
    icon: Option<&'static str>,
    active: bool,
    open: bool,
    width: f32,
}

impl MenuButton {
    pub fn new(id: &'static str, label: impl Into<SharedString>) -> Self {
        Self {
            id,
            label: label.into(),
            icon: None,
            active: false,
            open: false,
            width: MENU_WIDTH,
        }
    }
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
    /// A value is chosen, so the button stays highlighted while closed.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    /// `items` are the menu's rows; the menu carries the `{id}.menu` selector.
    pub fn build<V: HoverHost>(
        self,
        hover: &HoverFade,
        items: Vec<AnyElement>,
        on_toggle: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
        cx: &mut Context<V>,
    ) -> Div {
        let Self {
            id,
            label,
            icon: icon_name,
            active,
            open,
            width,
        } = self;
        let mut button = Button::new(id, label)
            .secondary()
            .selected(open || active)
            .trailing(icon("chevronDown", ICON_SIZE_SM));
        if let Some(name) = icon_name {
            button = button.icon(name);
        }
        super::layout::column()
            .relative()
            .child(button.build(hover, on_toggle, cx))
            .when(open, |s| {
                s.child(floating(
                    super::overlay::menu_shell(width)
                        .debug_selector(move || format!("{id}.menu"))
                        .children(items),
                    Anchor::TopLeft,
                    point(px(0.), px(CONTROL_HEIGHT + SPACE_1)),
                ))
            })
    }
}

/// A caption row inside a menu.
pub fn menu_label(text: impl Into<SharedString>) -> Div {
    div()
        .px(px(CONTROL_INSET_X))
        .py(px(6.))
        .text_size(type_size(CAPTION_SIZE))
        .text_color(rgb(TEXT_TERTIARY))
        .child(text.into())
}

pub fn menu_divider() -> Div {
    div()
        .my(px(MENU_INSET))
        .mx(px(-MENU_INSET))
        .h(px(1.))
        .bg(rgb(BORDER))
}
