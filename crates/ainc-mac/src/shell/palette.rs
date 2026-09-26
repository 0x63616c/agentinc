//! The shell's command palette items: pages, actions and workspaces, fuzzy
//! matched, grouped and remembered.
use super::*;

/// Everything the palette can do, in its canonical order.
pub(super) struct PaletteCandidate {
    group: &'static str,
    entry: PaletteEntry,
    control: Control,
}

/// The rendered groups plus the flat list of choices in the same order.
pub(super) struct PaletteResults {
    pub groups: Vec<PaletteGroup>,
    pub choices: Vec<(SharedString, Control)>,
}

impl PaletteResults {
    pub fn len(&self) -> usize {
        self.choices.len()
    }
}

impl Shell {
    fn palette_candidates(&self) -> Vec<PaletteCandidate> {
        let mut items = Vec::new();
        if !self.workspace_only {
            for (index, page) in PAGES.iter().enumerate() {
                let shortcut = if page.in_sidebar {
                    format!("⌘{}", index + 1)
                } else {
                    "⌘,".to_owned()
                };
                items.push(PaletteCandidate {
                    group: "Pages",
                    entry: PaletteEntry::new(
                        format!("page.{}", page.title.to_lowercase()),
                        page.title,
                    )
                    .icon(page.icon)
                    .shortcut(shortcut),
                    control: Control::Open(page.route),
                });
            }
            let sidebar_open = self.session.panes[0].open;
            items.push(PaletteCandidate {
                group: "Actions",
                entry: PaletteEntry::new(
                    "action.toggle-sidebar",
                    if sidebar_open {
                        "Hide sidebar"
                    } else {
                        "Show sidebar"
                    },
                )
                .icon("panel")
                .shortcut("⌘B"),
                control: Control::Sidebar,
            });
            items.push(PaletteCandidate {
                group: "Actions",
                entry: PaletteEntry::new("action.notifications", "Show notifications").icon("bell"),
                control: Control::Notifications,
            });
            items.push(PaletteCandidate {
                group: "Actions",
                entry: PaletteEntry::new("action.check-updates", "Check for updates")
                    .icon("download"),
                control: Control::CheckForUpdates,
            });
            for (size, label) in FontSize::ALL {
                items.push(PaletteCandidate {
                    group: "Actions",
                    entry: PaletteEntry::new(
                        format!("action.font-size.{}", label.to_lowercase()),
                        format!("Font size: {label}"),
                    )
                    .icon("settings")
                    .detail(if self.session.font_size == size {
                        "Current"
                    } else {
                        ""
                    }),
                    control: Control::FontSize(size),
                });
            }
        }
        let state = self.workspace_state();
        for workspace in state.workspaces {
            let current = workspace.id == state.current_id;
            items.push(PaletteCandidate {
                group: "Workspaces",
                entry: PaletteEntry::new(
                    format!("workspace.{}", workspace.id),
                    format!("Switch to {}", workspace.name),
                )
                .icon("agents")
                .detail(if current { "Current" } else { "" }),
                control: Control::SwitchWorkspace(workspace.id),
            });
        }
        items.push(PaletteCandidate {
            group: "Workspaces",
            entry: PaletteEntry::new("action.create-workspace", "Create workspace").icon("plus"),
            control: Control::NewWorkspace,
        });
        items
    }

    /// Groups matching `query`, with recent choices first when the query is empty.
    pub(super) fn palette_results(&self, query: &str) -> PaletteResults {
        let query = query.trim();
        let candidates = self.palette_candidates();
        let mut groups: Vec<PaletteGroup> = Vec::new();
        let mut choices = Vec::new();
        if query.is_empty() && !self.workspace_only {
            let recent: Vec<&PaletteCandidate> = self
                .session
                .recent_commands
                .iter()
                .filter_map(|id| candidates.iter().find(|item| item.entry.id.as_ref() == id))
                .collect();
            if !recent.is_empty() {
                let mut entries = Vec::new();
                for item in recent {
                    entries.push(clone_entry(&item.entry, vec![]));
                    choices.push((item.entry.id.clone(), item.control.clone()));
                }
                groups.push(PaletteGroup::new("Recent", entries));
            }
        }
        for group_name in ["Pages", "Actions", "Workspaces"] {
            let mut matched: Vec<(i32, &PaletteCandidate, Vec<usize>)> = candidates
                .iter()
                .filter(|item| item.group == group_name)
                .filter_map(|item| {
                    fuzzy_match(query, &item.entry.label)
                        .map(|found| (found.score, item, found.positions))
                })
                .collect();
            if !query.is_empty() {
                matched.sort_by_key(|(score, _, _)| std::cmp::Reverse(*score));
            }
            if matched.is_empty() {
                continue;
            }
            let mut entries = Vec::new();
            for (_, item, positions) in matched {
                entries.push(clone_entry(&item.entry, positions));
                choices.push((item.entry.id.clone(), item.control.clone()));
            }
            groups.push(PaletteGroup::new(group_name, entries));
        }
        PaletteResults { groups, choices }
    }

    pub(super) fn choose_palette(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let results = self.palette_results(&self.input.read(cx).content);
        let Some((id, control)) = results.choices.get(index).cloned() else {
            return;
        };
        self.session.remember_command(&id);
        self.dispatch(control, window, cx);
    }

    pub(super) fn command_palette(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.creating_workspace {
            return self.create_workspace_form(window, cx);
        }
        let results = self.palette_results(&self.input.read(cx).content);
        self.selected = self.selected.min(results.len().saturating_sub(1));
        self.palette_scroll
            .scroll_to_item(palette_child_index(&results.groups, self.selected));
        let empty: SharedString = if self.workspace_only {
            "Try another workspace name.".into()
        } else {
            "Try a page, an action or a workspace.".into()
        };
        render_palette(
            PaletteView {
                input: self.input.clone().into_any_element(),
                icon: "search",
                groups: &results.groups,
                selected: self.selected,
                empty,
                result_focus: &self.picker_result_focus,
                close_focus: &self.picker_close_focus,
                scroll: &self.palette_scroll,
                aria_label: if self.workspace_only {
                    "Switch workspace"
                } else {
                    "Go to pages, actions and workspaces"
                },
            },
            &self.hover,
            |this: &mut Self, index, window, cx| this.choose_palette(index, window, cx),
            |this: &mut Self, index, _| this.selected = index,
            |this: &mut Self, window, cx| this.dispatch(Control::Dismiss, window, cx),
            window,
            cx,
        )
        .into_any_element()
    }

    fn create_workspace_form(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        column()
            .id("search.dialog")
            .accessibility_id("search.dialog")
            .role(accesskit::Role::Dialog)
            .aria_label("Create workspace")
            .w(px(PALETTE_WIDTH))
            .bg(rgb(SURFACE_OVERLAY))
            .border_1()
            .border_color(rgb(BORDER_STRONG))
            .rounded(px(DIALOG_RADIUS))
            .shadow(shadow_dialog())
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                row()
                    .h(px(56.))
                    .px(px(SPACE_4))
                    .gap(px(SPACE_3))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(icon("plus", ICON_SIZE_LG))
                    .child(
                        div()
                            .flex_1()
                            .text_size(type_size(HEADING_SIZE))
                            .font_weight(FontWeight::MEDIUM)
                            .child("New workspace"),
                    )
                    .child(
                        Button::new("palette-close", "Close")
                            .ghost()
                            .small()
                            .on_surface(SURFACE_OVERLAY)
                            .track_focus(&self.picker_close_focus)
                            .trailing(kbd("esc"))
                            .build(
                                &self.hover,
                                |this: &mut Self, window, cx| {
                                    this.dispatch(Control::Dismiss, window, cx)
                                },
                                cx,
                            )
                            .px(px(SPACE_1)),
                    ),
            )
            .child(
                column()
                    .p(px(SPACE_4))
                    .gap(px(FORM_STACK_GAP))
                    .child(
                        Field::new(self.workspace_name.clone())
                            .label("Name")
                            .selector("workspace.name")
                            .build(window, cx),
                    )
                    .child(
                        row()
                            .gap(px(SPACE_3))
                            .items_start()
                            .child(
                                div().w(px(140.)).child(
                                    Field::new(self.workspace_icon.clone())
                                        .label("Icon")
                                        .hint("Up to four characters.")
                                        .selector("workspace.icon")
                                        .build(window, cx),
                                ),
                            )
                            .child(
                                div().flex_1().child(
                                    Field::new(self.workspace_color.clone())
                                        .label("Color")
                                        .hint("Optional, as #RRGGBB.")
                                        .selector("workspace.color")
                                        .build(window, cx),
                                ),
                            ),
                    )
                    .when_some(self.workspace_error.as_ref(), |form, error| {
                        form.child(
                            div()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(ERROR))
                                .child(error.clone()),
                        )
                    }),
            )
            .child(
                row()
                    .h(px(56.))
                    .px(px(SPACE_4))
                    .gap(px(CONTROL_GAP))
                    .justify_end()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(
                        Button::new("create-workspace-cancel", "Cancel")
                            .secondary()
                            .build(
                                &self.hover,
                                |this: &mut Self, window, cx| {
                                    this.dispatch(Control::Dismiss, window, cx)
                                },
                                cx,
                            ),
                    )
                    .child(
                        Button::new("create-workspace", "Create workspace")
                            .primary()
                            .enabled(!self.workspace_pending)
                            .build(
                                &self.hover,
                                |this: &mut Self, window, cx| {
                                    this.dispatch(Control::CreateWorkspace, window, cx)
                                },
                                cx,
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn clone_entry(entry: &PaletteEntry, positions: Vec<usize>) -> PaletteEntry {
    let mut copy = PaletteEntry::new(entry.id.clone(), entry.label.clone()).positions(positions);
    if let Some(icon) = entry.icon {
        copy = copy.icon(icon);
    }
    if let Some(detail) = entry.detail.clone().filter(|detail| !detail.is_empty()) {
        copy = copy.detail(detail);
    }
    if let Some(shortcut) = entry.shortcut.clone() {
        copy = copy.shortcut(shortcut);
    }
    copy
}
