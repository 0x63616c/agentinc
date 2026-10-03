//! The select control. Its menu floats over the page, so opening it never
//! changes the size of the row that owns it.
use super::{
    avatar::*, button::*, display::*, icon::Icon, layout::*, menu::*, motion::*,
    overlay::menu_shell, tokens::*,
};
use gpui::{prelude::*, *};

pub struct SelectOption {
    /// Stable identity for record-backed options; positional menus may omit it.
    pub id: Option<SharedString>,
    pub label: SharedString,
    pub description: Option<SharedString>,
    pub glyph: Option<(Icon, u32)>,
    /// A person or agent shown by their avatar: name, local photo and kind.
    pub avatar: Option<AssigneeFace>,
}

impl SelectOption {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            label: label.into(),
            description: None,
            glyph: None,
            avatar: None,
        }
    }
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = Some(id.into());
        self
    }
    /// Lead with an assignee's avatar: a person's round one or an agent's square one.
    pub fn avatar(mut self, face: AssigneeFace) -> Self {
        self.avatar = Some(face);
        self
    }
    /// A colored leading icon shown on the trigger and in the menu.
    pub fn glyph(mut self, name: Icon, color: u32) -> Self {
        self.glyph = Some((name, color));
        self
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
    quiet: bool,
    below: bool,
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
            quiet: false,
            below: false,
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
    /// A trigger with no surface or chevron at rest, for property rows where a
    /// column of bordered controls would be louder than the values they hold.
    /// Its menu drops below the trigger, right edges aligned.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }
    /// Open the menu below the trigger, left edges aligned, for selects in
    /// dialogs where a menu over the trigger would cover the fields above it.
    pub fn below(mut self) -> Self {
        self.below = true;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn build<V: 'static>(
        self,
        ui: &mut Ui<V>,
        on_toggle: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
        on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
    ) -> Div {
        let Self {
            id,
            options,
            value,
            placeholder,
            open,
            enabled,
            quiet,
            below,
            width,
        } = self;
        let current = value.and_then(|index| options.get(index));
        let label = current.map_or(placeholder, |option| option.label.clone());
        let glyph = current.and_then(|option| option.glyph);
        let face = current.and_then(|option| option.avatar.clone());
        let mut trigger = Button::new(ElementId::Name(id.clone()), label);
        if let Some((name, color)) = glyph {
            trigger = trigger.leading(icon(name, ICON_SIZE_SM).text_color(rgb(color)));
        } else if let Some(face) = face {
            trigger = trigger.leading(assignee_avatar(&face, ICON_SIZE));
        }
        // A quiet select reads as its value; the row itself is the affordance.
        if !quiet {
            trigger = trigger.trailing(icon(Icon::ChevronDown, ICON_SIZE_SM));
        }
        let trigger = trigger
            .kind(if quiet {
                ButtonKind::Ghost
            } else {
                ButtonKind::Secondary
            })
            .full_width()
            .align_start()
            .enabled(enabled)
            .selected(open)
            .build(ui, on_toggle)
            .when(!open && !quiet, |s| s.bg(rgb(SURFACE_INPUT)))
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
                        option.id.map(ElementId::Name).unwrap_or_else(|| {
                            ElementId::NamedInteger(item_name.clone(), index as u64)
                        }),
                        option.label,
                    )
                    .checked(value == Some(index))
                    .enabled(enabled);
                    if let Some((name, color)) = option.glyph {
                        item = item.glyph(name, color);
                    } else if let Some(face) = &option.avatar {
                        item = item.leading(assignee_avatar(face, ICON_SIZE));
                    }
                    if let Some(description) = option.description {
                        item = item.trailing(caption(description));
                    }
                    menu = menu.child(item.build(ui, move |view, window, cx| {
                        on_select(view, index, window, cx)
                    }));
                }
                // Like a macOS pop-up button: the menu opens over the trigger with
                // the current option on the trigger's own line, so it never has to
                // choose between opening downward and being clamped over itself.
                // A quiet select sits at the end of a property row, so its menu
                // drops below it, right edges aligned, and never covers its label.
                if quiet {
                    return s.child(floating(
                        menu,
                        Anchor::TopRight,
                        point(px(width), px(CONTROL_HEIGHT + SPACE_1)),
                    ));
                }
                if below {
                    return s.child(floating(
                        menu,
                        Anchor::TopLeft,
                        point(px(0.), px(CONTROL_HEIGHT + SPACE_1)),
                    ));
                }
                let current = value.unwrap_or(0) as f32;
                s.child(floating(
                    menu,
                    Anchor::TopLeft,
                    point(px(0.), px(-(MENU_INSET + current * MENU_ITEM_HEIGHT))),
                ))
            })
    }
}
