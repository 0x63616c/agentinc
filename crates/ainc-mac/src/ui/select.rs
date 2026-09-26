//! Dropdown menus and the select control. Overlays are deferred and anchored,
//! so opening one never changes the size of the row that owns it.
use super::{button::*, display::*, layout::*, motion::*, overlay::menu_shell, tokens::*};
use gpui::{prelude::*, *};

/// Floats `content` above everything else, anchored to where it is placed.
/// Place it inside a `column().relative()` wrapper so its origin is the
/// wrapper's top-left corner rather than the slot after its siblings.
pub fn floating(content: impl IntoElement, anchor: Anchor, offset: Point<Pixels>) -> Deferred {
    deferred(
        anchored()
            .anchor(anchor)
            .offset(offset)
            .snap_to_window_with_margin(px(SPACE_2))
            .child(content),
    )
    .with_priority(1)
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
            .on_surface(SURFACE_OVERLAY)
            .full_width()
            .align_start()
            .enabled(enabled);
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
            .when(destructive, |s| s.text_color(rgb(DESTRUCTIVE_TEXT)))
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

pub struct SelectOption {
    pub label: SharedString,
    pub description: Option<SharedString>,
}

impl SelectOption {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            description: None,
        }
    }
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A single-choice control whose menu floats over the page.
pub struct Select {
    id: SharedString,
    options: Vec<SelectOption>,
    value: Option<usize>,
    placeholder: SharedString,
    open: bool,
    enabled: bool,
    width: f32,
}

impl Select {
    pub fn new(id: impl Into<SharedString>, options: Vec<SelectOption>) -> Self {
        Self {
            id: id.into(),
            options,
            value: None,
            placeholder: "Choose…".into(),
            open: false,
            enabled: true,
            width: MENU_WIDTH,
        }
    }
    pub fn value(mut self, value: Option<usize>) -> Self {
        self.value = value;
        self
    }
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn build<V: HoverHost>(
        self,
        hover: &HoverFade,
        on_toggle: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
        on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
        cx: &mut Context<V>,
    ) -> Div {
        let Self {
            id,
            options,
            value,
            placeholder,
            open,
            enabled,
            width,
        } = self;
        let current = value.and_then(|index| options.get(index));
        let label = current.map_or(placeholder, |option| option.label.clone());
        let trigger_selector = id.to_string();
        let trigger = Button::new(ElementId::Name(id.clone()), label)
            .secondary()
            .full_width()
            .align_start()
            .enabled(enabled)
            .selected(open)
            .trailing(icon("chevronDown", ICON_SIZE_SM))
            .build(hover, on_toggle, cx)
            .debug_selector(move || trigger_selector.clone())
            .when(value.is_none(), |s| s.text_color(rgb(TEXT_SECONDARY)));
        column()
            .relative()
            .w(px(width))
            .child(trigger)
            .when(open, |s| {
                let item_name: SharedString = format!("{id}.option").into();
                let mut menu = menu_shell(width).debug_selector({
                    let selector = format!("{id}.menu");
                    move || selector.clone()
                });
                for (index, option) in options.into_iter().enumerate() {
                    let on_select = on_select.clone();
                    let mut item = MenuEntry::new(
                        ElementId::NamedInteger(item_name.clone(), index as u64),
                        option.label,
                    )
                    .checked(value == Some(index))
                    .enabled(enabled);
                    if let Some(description) = option.description {
                        item = item.trailing(caption(description));
                    }
                    menu = menu.child(item.build(
                        hover,
                        move |view, window, cx| on_select(view, index, window, cx),
                        cx,
                    ));
                }
                s.child(floating(
                    menu,
                    Anchor::TopLeft,
                    point(px(0.), px(CONTROL_HEIGHT + SPACE_1)),
                ))
            })
    }
}
