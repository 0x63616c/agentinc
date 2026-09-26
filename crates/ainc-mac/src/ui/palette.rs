//! The command palette surface: grouped, fuzzy-matched results with keyboard
//! navigation, highlighted matches and shortcut hints. Hosts own the items.
use super::{button::*, display::*, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};

pub struct PaletteEntry {
    pub id: SharedString,
    pub icon: Option<&'static str>,
    pub label: SharedString,
    /// Character positions in `label` matched by the query.
    pub positions: Vec<usize>,
    pub detail: Option<SharedString>,
    pub shortcut: Option<SharedString>,
}

impl PaletteEntry {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            label: label.into(),
            positions: vec![],
            detail: None,
            shortcut: None,
        }
    }
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn positions(mut self, positions: Vec<usize>) -> Self {
        self.positions = positions;
        self
    }
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

pub struct PaletteGroup {
    pub title: SharedString,
    pub entries: Vec<PaletteEntry>,
}

impl PaletteGroup {
    pub fn new(title: impl Into<SharedString>, entries: Vec<PaletteEntry>) -> Self {
        Self {
            title: title.into(),
            entries,
        }
    }
}

/// Total entries across groups.
pub fn palette_len(groups: &[PaletteGroup]) -> usize {
    groups.iter().map(|group| group.entries.len()).sum()
}

/// The index of the scroll container child holding entry `index`: each group
/// contributes one header child before its entries.
pub fn palette_child_index(groups: &[PaletteGroup], index: usize) -> usize {
    let mut remaining = index;
    let mut child = 0;
    for group in groups.iter().filter(|group| !group.entries.is_empty()) {
        child += 1;
        if remaining < group.entries.len() {
            return child + remaining;
        }
        remaining -= group.entries.len();
        child += group.entries.len();
    }
    child
}

/// The label with matched characters drawn in full-strength text.
fn highlighted(label: &SharedString, positions: &[usize], window: &Window) -> AnyElement {
    if positions.is_empty() {
        return div().truncate().child(label.clone()).into_any_element();
    }
    let mut style = window.text_style();
    style.color = rgb(TEXT_SECONDARY).into();
    let highlight = HighlightStyle {
        color: Some(rgb(TEXT).into()),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    let byte_offsets: Vec<(usize, usize)> = label
        .char_indices()
        .map(|(offset, c)| (offset, offset + c.len_utf8()))
        .collect();
    let highlights = positions.iter().filter_map(|&position| {
        byte_offsets
            .get(position)
            .map(|&(start, end)| (start..end, highlight))
    });
    StyledText::new(label.clone())
        .with_default_highlights(&style, highlights)
        .into_any_element()
}

pub struct PaletteView<'a> {
    pub input: AnyElement,
    pub icon: &'static str,
    pub groups: &'a [PaletteGroup],
    pub selected: usize,
    pub empty: SharedString,
    pub result_focus: &'a [FocusHandle],
    pub close_focus: &'a FocusHandle,
    pub scroll: &'a ScrollHandle,
    pub aria_label: &'static str,
}

/// Renders the palette; the host supplies the items and answers choices.
pub fn render_palette<V: HoverHost>(
    view: PaletteView<'_>,
    hover: &HoverFade,
    on_choose: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
    on_hover: impl Fn(&mut V, usize, &mut Context<V>) + Clone + 'static,
    on_close: impl Fn(&mut V, &mut Window, &mut Context<V>) + Clone + 'static,
    window: &Window,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let PaletteView {
        input,
        icon: leading,
        groups,
        selected,
        empty,
        result_focus,
        close_focus,
        scroll,
        aria_label,
    } = view;
    let total = palette_len(groups);
    let mut results = column()
        .id("palette-results")
        .track_scroll(scroll)
        .p(px(SPACE_2))
        .max_h(px(420.))
        .overflow_y_scroll();
    if total == 0 {
        results = results.child(
            column()
                .debug_selector(|| "palette.empty".into())
                .items_center()
                .py(px(SPACE_8))
                .gap(px(SPACE_1))
                .child(div().text_color(rgb(TEXT)).child("No matches"))
                .child(caption(empty)),
        );
    }
    let mut flat = 0;
    for group in groups {
        if group.entries.is_empty() {
            continue;
        }
        results = results.child(
            div()
                .px(px(SPACE_2))
                .pt(px(SPACE_2))
                .pb(px(SPACE_1))
                .child(eyebrow(group.title.clone())),
        );
        for entry in &group.entries {
            let index = flat;
            flat += 1;
            let is_selected = index == selected;
            let on_choose = on_choose.clone();
            let on_hover = on_hover.clone();
            let selector = format!("palette.result.{}", entry.id);
            let label = highlighted(&entry.label, &entry.positions, window);
            let on_hover = cx.listener(move |view: &mut V, over: &bool, _, cx| {
                if *over {
                    on_hover(view, index, cx);
                    cx.notify();
                }
            });
            results = results.child(action_button(
                ButtonSpec {
                    id: ElementId::Name(entry.id.clone()),
                    label: entry.label.clone(),
                    enabled: true,
                },
                |button| {
                    button
                        .debug_selector(move || selector.clone())
                        .when_some(result_focus.get(index), |s, focus| s.track_focus(focus))
                        .on_hover(on_hover)
                        .w_full()
                        .h(px(PALETTE_ROW_HEIGHT))
                        .px(px(SPACE_2))
                        .gap(px(SPACE_3))
                        .rounded(px(RADIUS_MD))
                        .when(is_selected, |s| s.bg(rgb(SELECTED)))
                        .child(
                            row()
                                .size(px(24.))
                                .justify_center()
                                .flex_shrink_0()
                                .rounded(px(RADIUS_SM))
                                .when(is_selected, |s| s.bg(rgb(SELECTED_STRONG)))
                                .child(match entry.icon {
                                    Some(name) => icon(name, ICON_SIZE)
                                        .when(is_selected, |s| s.text_color(rgb(TEXT)))
                                        .into_any_element(),
                                    None => div().into_any_element(),
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(type_size(BODY_SIZE))
                                .text_color(rgb(TEXT))
                                .child(label),
                        )
                        .when_some(entry.detail.clone(), |s, detail| {
                            s.child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(type_size(CAPTION_SIZE))
                                    .text_color(rgb(TEXT_TERTIARY))
                                    .child(detail),
                            )
                        })
                        .when_some(entry.shortcut.clone(), |s, shortcut| {
                            s.child(
                                row()
                                    .gap(px(SPACE_1))
                                    .children(shortcut.split(' ').map(|key| kbd(key.to_owned()))),
                            )
                        })
                        .child(
                            div()
                                .w(px(24.))
                                .flex_shrink_0()
                                .when(is_selected, |s| s.child(kbd("↵"))),
                        )
                },
                move |view, window, cx| on_choose(view, index, window, cx),
                cx,
            ));
        }
    }
    overlay_surface_for_palette()
        .accessibility_id("search.dialog")
        .role(accesskit::Role::Dialog)
        .aria_label(aria_label)
        .w(px(PALETTE_WIDTH))
        .overflow_hidden()
        .child(
            row()
                .h(px(56.))
                .px(px(SPACE_4))
                .gap(px(SPACE_3))
                .border_b_1()
                .border_color(rgb(BORDER))
                .text_size(type_size(HEADING_SIZE))
                .child(icon(leading, ICON_SIZE_LG))
                .child(div().flex_1().min_w_0().child(input))
                .child(
                    Button::new("palette-close", "Close")
                        .ghost()
                        .small()
                        .on_surface(SURFACE_OVERLAY)
                        .track_focus(close_focus)
                        .trailing(kbd("esc"))
                        .build(hover, on_close, cx)
                        .px(px(SPACE_1)),
                ),
        )
        .child(results)
        .child(
            row()
                .h(px(40.))
                .px(px(SPACE_4))
                .gap(px(SPACE_4))
                .border_t_1()
                .border_color(rgb(BORDER))
                .child(kbd_hint("↑ ↓", "Navigate"))
                .child(kbd_hint("↵", "Open"))
                .child(div().flex_1())
                .child(kbd_hint("esc", "Close")),
        )
}

fn overlay_surface_for_palette() -> Stateful<Div> {
    column()
        .id("search.dialog")
        .bg(rgb(SURFACE_OVERLAY))
        .border_1()
        .border_color(rgb(BORDER_STRONG))
        .rounded(px(DIALOG_RADIUS))
        .shadow(shadow_dialog())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
}

#[cfg(test)]
mod tests {
    use super::{PaletteEntry, PaletteGroup, palette_child_index, palette_len};

    fn groups() -> Vec<PaletteGroup> {
        vec![
            PaletteGroup::new(
                "Pages",
                vec![PaletteEntry::new("a", "A"), PaletteEntry::new("b", "B")],
            ),
            PaletteGroup::new("Empty", vec![]),
            PaletteGroup::new("Actions", vec![PaletteEntry::new("c", "C")]),
        ]
    }

    #[test]
    fn flat_indices_skip_group_headers() {
        let groups = groups();
        assert_eq!(palette_len(&groups), 3);
        assert_eq!(palette_child_index(&groups, 0), 1);
        assert_eq!(palette_child_index(&groups, 1), 2);
        // Empty groups render no header, so they occupy no child slot.
        assert_eq!(palette_child_index(&groups, 2), 4);
    }
}
