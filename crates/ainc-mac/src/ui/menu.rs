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
