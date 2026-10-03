//! The shell's command palette items: pages, actions and Tickets, fuzzy
//! matched, grouped and remembered.
use super::*;
use crate::ui_state::{Appearance, FontSize, UiState};
use ainc_client::types::Ticket;

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

/// Everything the palette offers for this state, in canonical order.
fn candidates(
    ui_state: &UiState,
    appearance: Appearance,
    tickets: &[Ticket],
) -> Vec<PaletteCandidate> {
    let mut items = Vec::new();
    for (index, page) in PAGES.iter().enumerate() {
        let mut entry =
            PaletteEntry::new(format!("page.{}", page.title.to_lowercase()), page.title)
                .icon(page.icon);
        if page.in_sidebar {
            entry = entry.shortcut(shortcuts::route(index + 1).1);
        } else if page.route == Route::Settings {
            entry = entry.shortcut(shortcuts::SETTINGS.glyph);
        }
        items.push(PaletteCandidate {
            group: "Pages",
            entry,
            control: Control::Go(Destination::Page(page.route)),
        });
    }
    let sidebar_open = ui_state.sidebar.open;
    items.push(PaletteCandidate {
        group: "Actions",
        entry: PaletteEntry::new(
            "action.toggle-sidebar",
            if sidebar_open {
                "Hide Sidebar"
            } else {
                "Show Sidebar"
            },
        )
        .icon(Icon::Panel)
        .shortcut(shortcuts::TOGGLE_SIDEBAR.glyph),
        control: Control::Sidebar,
    });
    items.push(PaletteCandidate {
        group: "Actions",
        entry: PaletteEntry::new("action.new-ticket", "New Ticket").icon(Icon::Plus),
        control: Control::Go(Destination::NewTicket),
    });
    items.push(PaletteCandidate {
        group: "Actions",
        entry: PaletteEntry::new("action.check-updates", "Check for Updates").icon(Icon::Download),
        control: Control::CheckForUpdates,
    });
    let current = FontSize::ALL
        .iter()
        .position(|(size, _)| *size == appearance.font_size)
        .unwrap_or(1);
    if let Some((size, label)) = FontSize::ALL.get(current + 1) {
        items.push(PaletteCandidate {
            group: "Actions",
            entry: PaletteEntry::new("action.font-size.larger", "Increase Font Size")
                .icon(Icon::Plus)
                .detail(*label),
            control: Control::Appearance(Appearance {
                font_size: *size,
                ..appearance
            }),
        });
    }
    if let Some((size, label)) = current.checked_sub(1).and_then(|i| FontSize::ALL.get(i)) {
        items.push(PaletteCandidate {
            group: "Actions",
            entry: PaletteEntry::new("action.font-size.smaller", "Decrease Font Size")
                .icon(Icon::Settings)
                .detail(*label),
            control: Control::Appearance(Appearance {
                font_size: *size,
                ..appearance
            }),
        });
    }

    for ticket in tickets {
        let status = ticket.status;
        items.push(PaletteCandidate {
            group: "Tickets",
            entry: PaletteEntry::new(
                format!("goto-ticket.{}", ticket.id),
                format!(
                    "{} {}",
                    crate::tickets::model::ticket_key(ticket.id),
                    ticket.title
                ),
            )
            .icon(crate::tickets::model::status_icon(status))
            .icon_color(crate::tickets::model::status_color(status))
            .detail(crate::tickets::model::status_name(status)),
            control: Control::Go(Destination::Ticket(ticket.id)),
        });
    }
    items
}

impl Shell {
    /// Groups matching `query`, with recent choices first when the query is empty.
    pub(super) fn palette_results(&self, query: &str) -> PaletteResults {
        let query = query.trim();
        let candidates = candidates(
            &self.ui_state,
            self.appearance.get(),
            &self.daemon.tickets().tickets,
        );
        let mut groups: Vec<PaletteGroup> = Vec::new();
        let mut choices = Vec::new();
        if query.is_empty() {
            let recent: Vec<&PaletteCandidate> = self
                .ui_state
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
                groups.push(PaletteGroup::new("recent", "Recent", entries));
            }
        }
        for (key, group_name) in [
            ("pages", "Pages"),
            ("actions", "Actions"),
            ("tickets", "Tickets"),
        ] {
            // Tickets are found by searching; an empty palette lists places and actions.
            if key == "tickets" && query.is_empty() {
                continue;
            }
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
            groups.push(PaletteGroup::new(key, group_name, entries));
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
        self.ui_state.remember_command(&id);
        self.dispatch(control, window, cx);
    }

    pub(super) fn command_palette(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let results = self.palette_results(&self.input.read(cx).content);
        self.selected = self.selected.min(results.len().saturating_sub(1));
        self.palette_scroll
            .scroll_to_item(palette_child_index(&results.groups, self.selected));
        Palette::new(
            &results.groups,
            self.input.clone(),
            &self.picker_result_focus,
            &self.picker_close_focus,
            &self.palette_scroll,
        )
        .icon(Icon::Search)
        .selected(self.selected)
        .empty("Try a page, an action or a Ticket.")
        .aria_label("Go to pages, actions and Tickets")
        .build(
            &mut Ui::new(window, cx),
            |this: &mut Self, index, window, cx| this.choose_palette(index, window, cx),
            |this: &mut Self, index, _| this.selected = index,
            |this: &mut Self, window, cx| this.dispatch(Control::Dismiss, window, cx),
        )
        .into_any_element()
    }
}

fn clone_entry(entry: &PaletteEntry, positions: Vec<usize>) -> PaletteEntry {
    let mut copy = PaletteEntry::new(entry.id.clone(), entry.label.clone()).positions(positions);
    if let Some(icon) = entry.icon {
        copy = copy.icon(icon);
    }
    if let Some(color) = entry.icon_color {
        copy = copy.icon_color(color);
    }
    if let Some(detail) = entry.detail.clone().filter(|detail| !detail.is_empty()) {
        copy = copy.detail(detail);
    }
    if let Some(shortcut) = entry.shortcut.clone() {
        copy = copy.shortcut(shortcut);
    }
    copy
}

#[cfg(test)]
mod tests {
    use super::{Control, candidates};
    use crate::ui_state::{Appearance, FontSize, UiState};
    use ainc_client::types::{AssigneeKind, Ticket, TicketPriority, TicketStatus};

    fn ticket(id: i64, title: &str) -> Ticket {
        Ticket {
            id,
            title: title.into(),
            description: String::new(),
            status: TicketStatus::ToDo,
            priority: TicketPriority::None,
            labels: vec![],
            assignee_kind: AssigneeKind::Human,
            assignee_id: "owner".into(),
            position: 0,
            revision: 0,
            generation: 0,
            created_at: 0,
            updated_at: 0,
            conversation_id: None,
        }
    }
    fn ids(items: &[super::PaletteCandidate], group: &str) -> Vec<String> {
        items
            .iter()
            .filter(|item| item.group == group)
            .map(|item| item.entry.id.to_string())
            .collect()
    }

    #[test]
    fn offers_pages_actions_and_one_entry_per_ticket() {
        let items = candidates(
            &UiState::default(),
            Appearance::default(),
            &[ticket(7, "Book dentist"), ticket(9, "Pay rent")],
        );
        assert!(ids(&items, "Pages").contains(&"page.tickets".to_owned()));
        let actions = ids(&items, "Actions");
        for id in [
            "action.toggle-sidebar",
            "action.new-ticket",
            "action.check-updates",
        ] {
            assert!(actions.iter().any(|a| a == id), "{id}");
        }
        assert_eq!(ids(&items, "Tickets"), ["goto-ticket.7", "goto-ticket.9"]);
        let first = items.iter().find(|i| i.group == "Tickets").unwrap();
        assert!(first.entry.label.contains("Book dentist"));
        assert!(matches!(first.control, Control::Go(_)));
    }

    #[test]
    fn sidebar_entry_and_font_steps_follow_the_state() {
        let mut state = UiState::default();
        state.sidebar.open = false;
        let items = candidates(&state, Appearance::default(), &[]);
        let toggle = items
            .iter()
            .find(|i| i.entry.id.as_ref() == "action.toggle-sidebar")
            .unwrap();
        assert_eq!(toggle.entry.label.as_ref(), "Show Sidebar");
        state.sidebar.open = true;
        let items = candidates(&state, Appearance::default(), &[]);
        assert!(
            items
                .iter()
                .any(|i| i.entry.label.as_ref() == "Hide Sidebar")
        );
        let (first, last) = (FontSize::ALL[0].0, FontSize::ALL[FontSize::ALL.len() - 1].0);
        let at = |size| {
            ids(
                &candidates(
                    &state,
                    Appearance {
                        font_size: size,
                        ..Appearance::default()
                    },
                    &[],
                ),
                "Actions",
            )
        };
        assert!(!at(first).contains(&"action.font-size.smaller".to_owned()));
        assert!(at(first).contains(&"action.font-size.larger".to_owned()));
        assert!(!at(last).contains(&"action.font-size.larger".to_owned()));
        assert!(at(last).contains(&"action.font-size.smaller".to_owned()));
    }
}
