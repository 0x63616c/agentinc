//! The one label picker: applied labels as removable tags and a menu that finds,
//! toggles and creates labels. The detail page edits a Ticket with it; the
//! create dialog fills its draft with it.
use super::*;

impl TicketsPage {
    /// Where the picker writes: the create dialog's draft while it is open,
    /// otherwise the open Ticket.
    fn drafting(&self) -> bool {
        self.overlays.borrow().active() == Some(Overlay::AddTicket)
    }

    fn applied_labels(&self) -> Vec<String> {
        if self.drafting() {
            return self.draft.labels.clone();
        }
        self.selected
            .and_then(|id| self.ticket(id))
            .map(|t| t.labels.clone())
            .unwrap_or_default()
    }

    /// Apply a label, or remove it when it is already applied.
    pub(super) fn toggle_label(&mut self, label: String, cx: &mut Context<Self>) {
        let label = label.trim().to_owned();
        if label.is_empty() || label.contains(',') || label.chars().count() > 32 {
            return;
        }
        let mut labels = self.applied_labels();
        match labels.iter().position(|l| l.eq_ignore_ascii_case(&label)) {
            Some(index) => {
                labels.remove(index);
            }
            None if labels.len() < 10 => labels.push(label),
            None => return,
        }
        self.label_input.update(cx, |input, _| input.reset());
        if self.drafting() {
            self.draft.labels = labels;
            cx.notify();
            return;
        }
        self.menu = None;
        let Some(ticket) = self.selected.and_then(|id| self.ticket(id)) else {
            return;
        };
        let (id, revision) = (ticket.id, ticket.revision);
        self.command(
            TicketCommand::SetLabels {
                id,
                revision,
                labels,
            },
            cx,
        );
    }

    pub(super) fn label_picker(
        &self,
        applied: &[String],
        menu_kind: Menu,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let open = self.menu == Some(menu_kind);
        let typed = self.label_input.read(cx).content.trim().to_owned();
        let known = all_labels(&self.state.tickets);
        let exists = known
            .iter()
            .chain(applied)
            .any(|label| label.eq_ignore_ascii_case(&typed));
        let mut menu = menu_shell(MENU_WIDTH)
            .debug_selector(|| "tickets.labels.menu".into())
            .child(
                div().p(px(SPACE_1)).child(
                    Field::new(self.label_input.clone())
                        .leading_icon("tag")
                        .selector("tickets.label")
                        .build(window, cx),
                ),
            );
        let mut offered = known;
        for label in applied {
            if !offered.iter().any(|l| l.eq_ignore_ascii_case(label)) {
                offered.push(label.clone());
            }
        }
        for label in offered
            .into_iter()
            .filter(|label| label.to_lowercase().contains(&typed.to_lowercase()))
        {
            let chosen = label.clone();
            let checked = applied.iter().any(|l| l.eq_ignore_ascii_case(&label));
            menu = menu.child(
                MenuEntry::new(
                    SharedString::from(format!("tickets.labels.add.{label}")),
                    label.clone(),
                )
                .glyph("dot", label_color(&label))
                .checked(checked)
                .build(
                    &self.hover,
                    move |this: &mut Self, _, cx| this.toggle_label(chosen.clone(), cx),
                    cx,
                ),
            );
        }
        if !typed.is_empty() && !exists {
            let created = typed.clone();
            menu = menu.child(
                MenuEntry::new("tickets.labels.create", format!("Create “{typed}”"))
                    .icon("plus")
                    .build(
                        &self.hover,
                        move |this: &mut Self, _, cx| this.toggle_label(created.clone(), cx),
                        cx,
                    ),
            );
        }
        row()
            .w_full()
            .flex_wrap()
            .gap(px(CHIP_GAP))
            .py(px(SPACE_1))
            .children(applied.iter().map(|label| {
                let removed = label.clone();
                tag(label.clone(), label_color(label))
                    .pr(px(SPACE_HALF))
                    .child(
                        Button::new(
                            SharedString::from(format!("tickets.labels.remove.{label}")),
                            format!("Remove {label}"),
                        )
                        .ghost()
                        .small()
                        .icon("close")
                        .icon_only()
                        .tint(TEXT_TERTIARY)
                        .enabled(!self.pending)
                        .build(
                            &self.hover,
                            move |this: &mut Self, _, cx| this.toggle_label(removed.clone(), cx),
                            cx,
                        )
                        .size(px(PILL_REMOVE_SIZE))
                        .rounded_full(),
                    )
            }))
            // The trigger's wrapper takes the rest of the line, so the menu can
            // end on the value column's right edge and never leave the window.
            .child(
                div()
                    .relative()
                    .flex_1()
                    .child(
                        Button::new("tickets.labels.open", "Add label")
                            .ghost()
                            .small()
                            .icon("plus")
                            .tint(TEXT_SECONDARY)
                            .selected(open)
                            .enabled(!self.pending && applied.len() < 10)
                            .build(
                                &self.hover,
                                move |this: &mut Self, window, cx| {
                                    this.toggle_menu(menu_kind, cx);
                                    if this.menu == Some(menu_kind) {
                                        window.focus(&this.label_input.focus_handle(cx), cx);
                                    }
                                },
                                cx,
                            )
                            .ml(px(-CONTROL_INSET_X_SM)),
                    )
                    .when(open, |s| {
                        s.child(
                            deferred(
                                div()
                                    .absolute()
                                    .top(px(CONTROL_HEIGHT_SM + SPACE_1))
                                    .right_0()
                                    .child(menu),
                            )
                            .with_priority(1),
                        )
                    }),
            )
    }
}
