//! The select control. Its menu floats over the page, so opening it never
//! changes the size of the row that owns it.
use super::{button::*, display::*, layout::*, menu::*, motion::*, overlay::menu_shell, tokens::*};
use gpui::{prelude::*, *};

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
            .when(!open, |s| s.bg(rgb(SURFACE_INPUT)))
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
