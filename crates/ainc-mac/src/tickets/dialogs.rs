//! The Tickets page's dialogs: create, rename, relate, delete and register an
//! agent. Each ends with the shared Cancel / confirm footer.
use super::*;

/// Candidates shown in the relationship picker.
const LINK_CANDIDATES: usize = 5;

impl TicketsPage {
    pub fn overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let active = self.overlays.borrow().active()?;
        let (title, body, enabled, submit): (String, AnyElement, bool, &str) = match active {
            Overlay::AddTicket => (
                "New Ticket".into(),
                self.create_form(window, cx).into_any_element(),
                !self.input.read(cx).content.trim().is_empty()
                    && Self::title_error(&self.input.read(cx).content).is_none(),
                "Create",
            ),
            Overlay::AddAgent => (
                "New Agent".into(),
                column_gap(FORM_STACK_GAP)
                    .child(text_field("Name", self.agent_name.clone(), window, cx))
                    .child(
                        Field::new(self.agent_instructions.clone())
                            .label("Instructions")
                            .multiline()
                            .build(window, cx),
                    )
                    .child(
                        Field::new(self.agent_model.clone())
                            .label("Model")
                            .hint("Leave empty to use the connection default.")
                            .build(window, cx),
                    )
                    .when_some(self.form_error.clone(), |s, error| {
                        s.child(error_text(error))
                    })
                    .into_any_element(),
                !self.agent_name.read(cx).content.trim().is_empty(),
                "Create",
            ),
            Overlay::RenameTicket(_) => (
                "Rename Ticket".into(),
                Field::new(self.rename.clone())
                    .label("Title")
                    .error(self.form_error.clone())
                    .build(window, cx)
                    .into_any_element(),
                !self.rename.read(cx).content.trim().is_empty()
                    && Self::title_error(&self.rename.read(cx).content).is_none(),
                "Save",
            ),
            Overlay::LinkTicket(id) => (
                format!("Relate {}", ticket_key(id)),
                self.link_form(id, window, cx).into_any_element(),
                self.link_target.is_some(),
                "Add relationship",
            ),
            Overlay::DeleteTicket(id) => {
                let ticket = self.ticket(id)?;
                (
                    format!("Delete “{}”?", ticket.title),
                    column()
                        .gap(px(SPACE_2))
                        .child(caption(
                            "This Ticket, its Comments and its relationships will be removed.",
                        ))
                        .when_some(self.form_error.clone(), |s, error| {
                            s.child(error_text(error))
                        })
                        .into_any_element(),
                    true,
                    "Delete",
                )
            }
            _ => return None,
        };
        let deleting = matches!(active, Overlay::DeleteTicket(_));
        let submit_label = if self.pending { "Saving…" } else { submit };
        let footer = dialog_footer(
            Button::new("tickets.cancel", "Cancel")
                .secondary()
                .track_focus(&self.cancel_focus)
                .build(
                    &self.hover,
                    |this: &mut Self, window, cx| {
                        this.menu = None;
                        this.overlays.borrow_mut().dismiss(window, cx);
                        cx.notify();
                    },
                    cx,
                ),
            Button::new("tickets.submit", submit_label)
                .kind(if deleting {
                    ButtonKind::Destructive
                } else {
                    ButtonKind::Primary
                })
                .enabled(enabled && !self.pending)
                .track_focus(&self.submit_focus)
                .build(
                    &self.hover,
                    move |this: &mut Self, _, cx| this.submit(active, cx),
                    cx,
                ),
        );
        Some(dialog_shell(title, body, footer).into_any_element())
    }

    fn submit(&mut self, active: Overlay, cx: &mut Context<Self>) {
        self.menu = None;
        match active {
            Overlay::AddTicket => self.create(cx),
            Overlay::AddAgent => {
                let name = self.agent_name.read(cx).content.trim().to_owned();
                let instructions = self.agent_instructions.read(cx).content.trim().to_owned();
                let model = self.agent_model.read(cx).content.trim().to_owned();
                self.command(
                    TicketCommand::RegisterAgent {
                        name,
                        instructions,
                        model: if model.is_empty() {
                            "connection-default".into()
                        } else {
                            model
                        },
                    },
                    cx,
                );
            }
            Overlay::RenameTicket(_) => self.rename_ticket(cx),
            Overlay::LinkTicket(id) => {
                if let Some(target) = self.link_target {
                    let (from_id, to_id, link) = self.link_relation.link(id, target);
                    self.command(
                        TicketCommand::Link {
                            from_id,
                            to_id,
                            link,
                        },
                        cx,
                    );
                }
            }
            Overlay::DeleteTicket(id) => {
                if let Some(ticket) = self.ticket(id) {
                    let revision = ticket.revision;
                    self.command(TicketCommand::Delete { id, revision }, cx);
                }
            }
            _ => {}
        }
    }

    fn create_form(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let half = (DIALOG_WIDTH - 2. * DIALOG_PADDING - SPACE_3) / 2.;
        let full = DIALOG_WIDTH - 2. * DIALOG_PADDING;
        let status = Select::new(
            "tickets.draft.status",
            STATUSES
                .map(|s| SelectOption::new(status_name(s)).glyph(status_icon(s), status_color(s)))
                .into(),
        )
        .value(STATUSES.iter().position(|s| *s == self.draft.status))
        .open(self.menu == Some(Menu::DraftStatus))
        .width(half)
        .build(
            &self.hover,
            |this: &mut Self, _, cx| this.toggle_menu(Menu::DraftStatus, cx),
            |this: &mut Self, index, _, cx| {
                this.menu = None;
                if let Some(status) = STATUSES.get(index) {
                    this.draft.status = *status;
                }
                cx.notify();
            },
            cx,
        );
        let priority = Select::new(
            "tickets.draft.priority",
            PRIORITIES
                .map(|p| {
                    SelectOption::new(priority_name(p)).glyph(priority_icon(p), priority_color(p))
                })
                .into(),
        )
        .value(PRIORITIES.iter().position(|p| *p == self.draft.priority))
        .open(self.menu == Some(Menu::DraftPriority))
        .width(half)
        .build(
            &self.hover,
            |this: &mut Self, _, cx| this.toggle_menu(Menu::DraftPriority, cx),
            |this: &mut Self, index, _, cx| {
                this.menu = None;
                if let Some(priority) = PRIORITIES.get(index) {
                    this.draft.priority = *priority;
                }
                cx.notify();
            },
            cx,
        );
        let assignees = self.state.assignees.clone();
        let chosen = self.draft.assignee.as_deref().unwrap_or("owner");
        let assignee = Select::new(
            "tickets.draft.assignee",
            assignees
                .iter()
                .map(|a| {
                    let option = SelectOption::new(self.assignee_name(&a.id)).glyph(
                        if a.kind == AssigneeKind::Agent {
                            "agents"
                        } else {
                            "user"
                        },
                        TEXT_SECONDARY,
                    );
                    if a.kind == AssigneeKind::Agent {
                        option.description("Agent")
                    } else {
                        option
                    }
                })
                .collect(),
        )
        .value(assignees.iter().position(|a| a.id == chosen))
        .open(self.menu == Some(Menu::DraftAssignee))
        .width(full)
        .build(
            &self.hover,
            |this: &mut Self, _, cx| this.toggle_menu(Menu::DraftAssignee, cx),
            move |this: &mut Self, index, _, cx| {
                this.menu = None;
                this.draft.assignee = assignees.get(index).map(|a| a.id.clone());
                cx.notify();
            },
            cx,
        );
        let starts_work = actionable(self.draft.status)
            && self
                .draft
                .assignee
                .as_deref()
                .is_some_and(|id| self.is_agent(id));
        column_gap(FORM_STACK_GAP)
            .child(
                Field::new(self.input.clone())
                    .label("Title")
                    .error(self.form_error.clone())
                    .build(window, cx),
            )
            .child(
                Field::new(self.draft_description.clone())
                    .label("Description")
                    .multiline()
                    .build(window, cx),
            )
            .child(
                row()
                    .items_start()
                    .gap(px(SPACE_3))
                    .child(
                        column_gap(FIELD_LABEL_GAP)
                            .child(field_label("Status"))
                            .child(status),
                    )
                    .child(
                        column_gap(FIELD_LABEL_GAP)
                            .child(field_label("Priority"))
                            .child(priority),
                    ),
            )
            .child(
                column_gap(FIELD_LABEL_GAP)
                    .child(field_label("Assignee"))
                    .child(assignee)
                    .when(starts_work, |s| {
                        s.child(hint("The agent starts work as soon as you create it."))
                    }),
            )
            .child({
                let known = all_labels(&self.state.tickets);
                column_gap(FIELD_LABEL_GAP)
                    .child(field_label("Labels"))
                    .when(!known.is_empty(), |s| {
                        s.child(row().flex_wrap().gap(px(CHIP_GAP)).children(
                            known.into_iter().map(|label| {
                                let chosen = self.draft.labels.contains(&label);
                                let toggled = label.clone();
                                chip(
                                    SharedString::from(format!("tickets.draft.label.{label}")),
                                    label,
                                    chosen,
                                    true,
                                    &self.hover,
                                    move |this: &mut Self, _, cx| {
                                        if let Some(index) =
                                            this.draft.labels.iter().position(|l| *l == toggled)
                                        {
                                            this.draft.labels.remove(index);
                                        } else {
                                            this.draft.labels.push(toggled.clone());
                                        }
                                        cx.notify();
                                    },
                                    cx,
                                )
                            }),
                        ))
                    })
                    .child(
                        Field::new(self.draft_labels.clone())
                            .selector("Labels")
                            .hint("New labels, separated by commas.")
                            .build(window, cx),
                    )
            })
    }

    /// Tickets the relationship can point at: not this one, not already
    /// related, matching the search, the chosen one first, then open work.
    fn link_candidates(&self, id: i64, cx: &App) -> Vec<&Ticket> {
        let related: Vec<i64> = relations(&self.state.links, id)
            .into_iter()
            .map(|(_, other)| other)
            .collect();
        let query = Filters {
            query: self.link_search.read(cx).content.to_string(),
            ..Filters::default()
        };
        let mut candidates: Vec<&Ticket> = self
            .state
            .tickets
            .iter()
            .filter(|t| t.id != id && !related.contains(&t.id))
            .filter(|t| query.matches(t))
            .collect();
        candidates.sort_by_key(|t| {
            (
                Some(t.id) != self.link_target,
                matches!(t.status, TicketStatus::Done | TicketStatus::Cancelled),
                std::cmp::Reverse(t.updated_at),
            )
        });
        candidates.truncate(LINK_CANDIDATES);
        candidates
    }

    fn link_form(&self, id: i64, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let full = DIALOG_WIDTH - 2. * DIALOG_PADDING;
        let relation = Select::new(
            "tickets.link.relation",
            Relation::ALL.map(|r| SelectOption::new(r.name())).into(),
        )
        .value(Relation::ALL.iter().position(|r| *r == self.link_relation))
        .open(self.menu == Some(Menu::Relation))
        .width(full)
        .build(
            &self.hover,
            |this: &mut Self, _, cx| this.toggle_menu(Menu::Relation, cx),
            |this: &mut Self, index, _, cx| {
                this.menu = None;
                if let Some(relation) = Relation::ALL.get(index) {
                    this.link_relation = *relation;
                    this.link_target = None;
                }
                cx.notify();
            },
            cx,
        );
        let candidates = self.link_candidates(id, cx);
        let count = candidates.len();
        column_gap(FORM_STACK_GAP)
            .child(
                column_gap(FIELD_LABEL_GAP)
                    .child(field_label("Relationship"))
                    .child(relation),
            )
            .child(
                Field::new(self.link_search.clone())
                    .label("Ticket")
                    .leading_icon("search")
                    .build(window, cx),
            )
            .child(if candidates.is_empty() {
                hint("No other Tickets match.").into_any_element()
            } else {
                card()
                    .debug_selector(|| "tickets.link.candidates".into())
                    .p(px(SPACE_1))
                    .gap_0()
                    .children(candidates.into_iter().enumerate().map(|(index, ticket)| {
                        let other = ticket.id;
                        column()
                            .when(index + 1 < count, |s| {
                                s.border_b_1().border_color(rgb(BORDER_SUBTLE))
                            })
                            .child(
                                ListRow::new(
                                    ElementId::NamedInteger(
                                        "tickets.link.target".into(),
                                        other as u64,
                                    ),
                                    ticket.title.clone(),
                                )
                                .leading(
                                    icon(status_icon(ticket.status), ICON_SIZE_SM)
                                        .text_color(rgb(status_color(ticket.status))),
                                )
                                .subtitle(format!(
                                    "{} · {}",
                                    ticket_key(other),
                                    status_name(ticket.status)
                                ))
                                .selected(self.link_target == Some(other))
                                .build(
                                    &self.hover,
                                    move |this: &mut Self, _, cx| {
                                        this.link_target = Some(other);
                                        cx.notify();
                                    },
                                    cx,
                                ),
                            )
                    }))
                    .into_any_element()
            })
            .when_some(self.form_error.clone(), |s, error| {
                s.child(error_text(error))
            })
    }
}
