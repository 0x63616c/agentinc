//! Single-choice groups: segmented controls, tabs and chips.
use super::{button::*, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

fn toggled(on: bool) -> accesskit::Toggled {
    if on {
        accesskit::Toggled::True
    } else {
        accesskit::Toggled::False
    }
}

/// A compact track of options where exactly one is selected.
pub fn segmented<V: HoverHost>(
    id: &'static str,
    options: impl IntoIterator<Item = impl Into<SharedString>>,
    selected: usize,
    enabled: bool,
    hover: &HoverFade,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Div {
    let name: SharedString = id.into();
    let mut track = row()
        .debug_selector(move || id.into())
        .flex_wrap()
        .p(px(3.))
        .gap(px(2.))
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
                .on_surface(SURFACE_CONTROL)
                .selected(is_selected)
                .enabled(enabled)
                .build(
                    hover,
                    move |view, window, cx| on_select(view, index, window, cx),
                    cx,
                )
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

/// Underlined tabs that switch views inside one page.
pub fn tabs<V: HoverHost>(
    id: &'static str,
    labels: impl IntoIterator<Item = impl Into<SharedString>>,
    selected: usize,
    hover: &HoverFade,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Div {
    let name: SharedString = id.into();
    let mut bar = row()
        .debug_selector(move || id.into())
        .w_full()
        .gap(px(SPACE_1))
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
                        .selected(is_selected)
                        .build(
                            hover,
                            move |view, window, cx| on_select(view, index, window, cx),
                            cx,
                        )
                        .role(accesskit::Role::Tab)
                        .aria_toggled(toggled(is_selected))
                        .when(is_selected, |s| s.bg(rgb(SURFACE)))
                        .mb(px(SPACE_1)),
                )
                .child(
                    div()
                        .h(px(2.))
                        .mb(px(-1.))
                        .rounded_t(px(2.))
                        .bg(rgb(if is_selected { PRIMARY } else { SURFACE }))
                        .when(!is_selected, |s| s.opacity(0.)),
                ),
        );
    }
    bar
}

/// One option in a single-selection chip group, such as a Ticket status.
pub fn chip<V: HoverHost>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    enabled: bool,
    hover: &HoverFade,
    action: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    Button::new(id, label)
        .secondary()
        .small()
        .selected(selected)
        .enabled(enabled)
        .build(hover, action, cx)
        .role(accesskit::Role::RadioButton)
        .aria_toggled(toggled(selected))
        .rounded_full()
        .when(selected, |s| s.border_color(rgb(FOCUS)))
}
