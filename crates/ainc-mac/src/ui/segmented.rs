//! Single-choice groups: segmented controls, tabs and chips.
use super::{button::*, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

/// A compact track of options where exactly one is selected.
pub struct Segmented {
    id: &'static str,
    options: Vec<SharedString>,
    selected: usize,
    enabled: bool,
}

impl Segmented {
    pub fn new(
        id: &'static str,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id,
            options: options.into_iter().map(Into::into).collect(),
            selected: 0,
            enabled: true,
        }
    }
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn build<V: 'static>(
        self,
        ui: &mut Ui<V>,
        on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
    ) -> Div {
        let Self {
            id,
            options,
            selected,
            enabled,
        } = self;
        let name: SharedString = id.into();
        let mut track = row()
            .debug_selector(move || id.into())
            .flex_wrap()
            .p(px(SEGMENT_TRACK_INSET))
            .gap(px(SEGMENT_GAP))
            .rounded(px(RADIUS_MD))
            .bg(rgb(SURFACE_CONTROL))
            .border_1()
            .border_color(rgb(BORDER));
        for (index, option) in options.into_iter().enumerate() {
            let on_select = on_select.clone();
            let is_selected = index == selected;
            track = track.child(
                Button::new(ElementId::NamedInteger(name.clone(), index as u64), option)
                    .ghost()
                    .small()
                    .selected(is_selected)
                    .enabled(enabled)
                    .build(ui, move |view, window, cx| {
                        on_select(view, index, window, cx)
                    })
                    .role(accesskit::Role::RadioButton)
                    .aria_toggled(toggled(is_selected))
                    .rounded(px(RADIUS_SM))
                    .when(is_selected, |s| {
                        s.bg(rgb(SELECTED_STRONG)).text_color(rgb(TEXT))
                    }),
            );
        }
        track
    }
}

/// Underlined tabs that switch views inside one page.
pub struct Tabs {
    id: &'static str,
    labels: Vec<SharedString>,
    selected: usize,
}

impl Tabs {
    pub fn new(
        id: &'static str,
        labels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id,
            labels: labels.into_iter().map(Into::into).collect(),
            selected: 0,
        }
    }
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }
    pub fn build<V: 'static>(
        self,
        ui: &mut Ui<V>,
        on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
    ) -> Div {
        let Self {
            id,
            labels,
            selected,
        } = self;
        let name: SharedString = id.into();
        let mut bar = row()
            .debug_selector(move || id.into())
            .w_full()
            .gap(px(SPACE_5))
            .border_b_1()
            .border_color(rgb(BORDER));
        for (index, label) in labels.into_iter().enumerate() {
            let on_select = on_select.clone();
            let is_selected = index == selected;
            bar = bar.child(
                column()
                    .child(
                        Button::new(ElementId::NamedInteger(name.clone(), index as u64), label)
                            .ghost()
                            .build(ui, move |view, window, cx| {
                                on_select(view, index, window, cx)
                            })
                            .role(accesskit::Role::Tab)
                            .aria_toggled(toggled(is_selected))
                            .px(px(0.))
                            .when(!is_selected, |s| s.text_color(rgb(TEXT_SECONDARY)))
                            .mb(px(SPACE_1)),
                    )
                    .child(
                        // The underline sits on the bar's hairline, not above it.
                        div()
                            .h(px(TAB_UNDERLINE))
                            .mb(px(-1.))
                            .rounded_t(px(TAB_UNDERLINE))
                            .bg(rgb(PRIMARY))
                            .when(!is_selected, |s| s.opacity(0.)),
                    ),
            );
        }
        bar
    }
}

/// One option in a single-selection chip group, such as a Ticket status.
pub struct Chip {
    id: ElementId,
    label: SharedString,
    selected: bool,
    enabled: bool,
}

impl Chip {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            enabled: true,
        }
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn build<V: 'static>(
        self,
        ui: &mut Ui<V>,
        action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    ) -> Stateful<Div> {
        let Self {
            id,
            label,
            selected,
            enabled,
        } = self;
        Button::new(id, label)
            .secondary()
            .small()
            .selected(selected)
            .enabled(enabled)
            .build(ui, action)
            .role(accesskit::Role::RadioButton)
            .aria_toggled(toggled(selected))
            .rounded_full()
            .when(selected, |s| s.border_color(rgb(FOCUS)))
    }
}
