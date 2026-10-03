//! One Ticket: its title as the page heading, the description, a timeline of
//! history and Comments with the composer, and a properties column for status,
//! priority, assignee, labels, relationships, work and where it came from.
use super::*;
use ainc_client::types::Comment;

/// Work listed under the Work heading, newest first.
const RECENT_WORK: usize = 3;

/// One timeline row, in time order.
enum Moment<'a> {
    Change(&'a TicketActivity),
    Comment(&'a Comment),
}
impl Moment<'_> {
    fn at(&self) -> i64 {
        match self {
            Self::Change(entry) => entry.created_at,
            Self::Comment(comment) => comment.created_at,
        }
    }
}

impl TicketsPage {
    pub(super) fn detail(&mut self, ticket: &Ticket, ui: &mut Ui<Self>) -> Stateful<Div> {
        let beside = ui.window.viewport_size().width >= px(PROPERTIES_BESIDE_MIN_WIDTH);
        let header = self.detail_header(ticket, ui);
        // Lower the main column so its first heading shares a line with the first
        // property label inside the card beside it.
        let main = column()
            .flex_1()
            .min_w_0()
            .pt(px(SPACE_2))
            .gap(px(SECTION_GAP))
            .child(self.description_section(ticket, ui))
            .child(self.timeline(ticket, ui));
        let properties = self.properties(ticket, ui);
        let body = if beside {
            row()
                .items_start()
                .gap(px(SECTION_GAP))
                .child(main)
                .child(properties.w(px(PROPERTIES_WIDTH)).flex_shrink_0())
        } else {
            column()
                .gap(px(SECTION_GAP))
                .child(properties.w_full())
                .child(main)
        };
        PageFrame::document(header)
            .child(
                column()
                    .id("tickets-page")
                    .track_focus(&self.page_focus)
                    .gap(px(SPACE_4))
                    .when_some(self.sync.read(ui.cx).message(), |s, error| {
                        s.child(banner(Tone::Danger, error))
                    })
                    .when_some(
                        self.form_error
                            .clone()
                            .filter(|_| self.overlays.active().is_none()),
                        |s, error| s.child(banner(Tone::Danger, error)),
                    )
                    .child(body),
            )
            .build()
    }

    /// A way back, the title as the page's one heading, a meta line and the
    /// actions that apply to this Ticket.
    fn detail_header(&self, ticket: &Ticket, ui: &mut Ui<Self>) -> PageHeader {
        let id = ticket.id;
        let revision = ticket.revision;
        let running = self.running(ticket);
        let has_runs = self.state.runs.iter().any(|r| r.ticket_id == id);
        let enabled = !self.pending.busy();
        // The properties column holds status and times; the line under the
        // title says only which Ticket this is and who holds it.
        let meta = format!(
            "{} · Assigned to {}",
            ticket_key(id),
            self.assignee_name(&ticket.assignee_id)
        );
        PageHeader::new(ticket.title.clone())
            .leading(
                Button::new("tickets.back", "Tickets")
                    .ghost()
                    .small()
                    .icon(Icon::ChevronLeft)
                    .tint(TEXT_SECONDARY)
                    .build(ui, |this, _, cx| this.close_detail(cx))
                    .ml(px(-CONTROL_INSET_X_SM)),
            )
            .description(meta)
            .actions(
                // The row ends with a ghost, so its label, not its padding,
                // lands on the content's right edge.
                row_gap(CONTROL_GAP)
                    .mr(px(-CONTROL_INSET_X))
                    .when(running, |s| {
                        s.child(
                            Button::new("tickets.stop", "Stop Work")
                                .secondary()
                                .icon(Icon::Stop)
                                .enabled(enabled)
                                .build(ui, move |this, _, cx| {
                                    this.command(TicketCommand::Cancel { id, revision }, cx)
                                }),
                        )
                    })
                    .child({
                        // Deleting stays visible once Work exists, disabled; the Work
                        // section says why.
                        Button::new("tickets.delete", "Delete")
                            .ghost()
                            .icon(Icon::Trash)
                            .tint(TEXT_SECONDARY)
                            .enabled(enabled && !has_runs)
                            .build(ui, move |this, window, cx| {
                                this.overlays.open_dialog(
                                    Dialog::Delete(id),
                                    this.cancel_focus.clone(),
                                    window,
                                    cx,
                                );
                                cx.notify();
                            })
                    })
                    .child(
                        Button::new("tickets.rename", "Rename")
                            .ghost()
                            .icon(Icon::Edit)
                            .tint(TEXT_SECONDARY)
                            .enabled(enabled)
                            .build(ui, move |this: &mut Self, window, cx| {
                                let title = this.ticket(id).map(|t| t.title.clone());
                                this.rename.update(cx, |input, cx| {
                                    input.set_text(&title.unwrap_or_default(), cx)
                                });
                                let focus = this.rename.focus_handle(cx);
                                this.form_error = None;
                                this.overlays
                                    .open_dialog(Dialog::Rename(id), focus, window, cx);
                                cx.notify();
                            }),
                    ),
            )
    }

    fn description_section(&self, ticket: &Ticket, ui: &mut Ui<Self>) -> Div {
        let id = ticket.id;
        let revision = ticket.revision;
        let body: AnyElement = if self.editing_description {
            column()
                .gap(px(SPACE_3))
                .child(
                    Field::new(self.description.clone())
                        .multiline()
                        .selector("tickets.description")
                        .build(ui),
                )
                .child(
                    row()
                        .gap(px(CONTROL_GAP))
                        .justify_end()
                        .child(
                            Button::new("tickets.description.cancel", "Cancel")
                                .secondary()
                                .small()
                                .build(ui, |this: &mut Self, _, cx| {
                                    this.editing_description = false;
                                    cx.notify();
                                }),
                        )
                        .child(
                            Button::new("tickets.description.save", "Save")
                                .primary()
                                .small()
                                .enabled(!self.pending.busy())
                                .build(ui, move |this: &mut Self, _, cx| {
                                    let description =
                                        this.description.read(cx).content.trim().to_owned();
                                    this.command(
                                        TicketCommand::Describe {
                                            id,
                                            revision,
                                            description,
                                        },
                                        cx,
                                    );
                                }),
                        ),
                )
                .into_any_element()
        } else if ticket.description.is_empty() {
            caption("No description yet. Add the context an agent or future you will need.")
                .into_any_element()
        } else {
            column()
                .debug_selector(|| "tickets.description.text".into())
                .gap(px(SPACE_2))
                .text_size(type_size(BODY_SIZE))
                .line_height(relative(BODY_LINE_HEIGHT))
                .children(
                    ticket
                        .description
                        .split("\n\n")
                        .map(|paragraph| div().child(paragraph.to_owned())),
                )
                .into_any_element()
        };
        let text = ticket.description.clone();
        column()
            .gap(px(SPACE_2))
            .child(
                row()
                    .h(px(CONTROL_HEIGHT_SM))
                    .justify_between()
                    .child(eyebrow("Description"))
                    .when(!self.editing_description, |s| {
                        s.child(
                            Button::new("tickets.description.edit", "Edit")
                                .ghost()
                                .small()
                                .tint(TEXT_SECONDARY)
                                .enabled(!self.pending.busy())
                                .build(ui, move |this: &mut Self, window, cx| {
                                    this.description
                                        .update(cx, |input, cx| input.set_text(&text, cx));
                                    this.editing_description = true;
                                    window.focus(&this.description.focus_handle(cx), cx);
                                    cx.notify();
                                })
                                .mr(px(-CONTROL_INSET_X_SM)),
                        )
                    }),
            )
            .child(body)
    }

    fn timeline(&self, ticket: &Ticket, ui: &mut Ui<Self>) -> Div {
        let now = time::now();
        let history: &[TicketActivity] =
            if self.activity_for.as_ref().map(|s| s.0) == Some(ticket.id) {
                &self.activity
            } else {
                &[]
            };
        let mut moments: Vec<Moment> = history
            .iter()
            .map(Moment::Change)
            .chain(
                self.state
                    .comments
                    .iter()
                    .filter(|c| c.ticket_id == ticket.id)
                    .map(Moment::Comment),
            )
            .collect();
        // Stable: history before Comments made in the same second.
        moments.sort_by_key(Moment::at);
        let comments = self.comment_count(ticket.id);
        column()
            .gap(px(SPACE_3))
            .child(
                row()
                    .h(px(CONTROL_HEIGHT_SM))
                    .gap(px(SPACE_2))
                    .child(eyebrow("Activity"))
                    .when(comments > 0, |s| {
                        s.child(hint(match comments {
                            1 => "1 Comment".to_owned(),
                            n => format!("{n} Comments"),
                        }))
                    }),
            )
            .child(
                column()
                    .debug_selector(|| "tickets.timeline".into())
                    .gap(px(SPACE_4))
                    .children(moments.into_iter().map(|moment| match moment {
                        Moment::Change(entry) => self.change_row(entry, now).into_any_element(),
                        Moment::Comment(comment) => {
                            self.comment_row(comment, now).into_any_element()
                        }
                    })),
            )
            .child(
                row()
                    .mt(px(SPACE_2))
                    .gap(px(SPACE_3))
                    .child(assignee_avatar(&self.assignee_face("owner"), AVATAR_SIZE))
                    .child(
                        div().flex_1().min_w_0().child(
                            Field::new(self.comment.clone())
                                .selector("Comment")
                                .build(ui),
                        ),
                    )
                    .child(
                        Button::new("tickets.post", "Post")
                            .primary()
                            .enabled(
                                !self.pending.busy()
                                    && !self.comment.read(ui.cx).content.trim().is_empty(),
                            )
                            .build(ui, |this, _, cx| this.add_comment(cx)),
                    ),
            )
    }

    /// Who made a change: Evee when a Conversation did it, otherwise the actor.
    fn actor(&self, entry: &TicketActivity) -> SharedString {
        if entry.conversation_id.is_some() {
            "Evee".into()
        } else {
            self.assignee_name(&entry.actor_id)
        }
    }

    fn change_row(&self, entry: &TicketActivity, now: i64) -> Div {
        let actor = self.actor(entry);
        let (glyph, action) = describe(entry, |id| self.assignee_name(id).to_string());
        let text = format!("{actor} {action}");
        let emphasis = HighlightStyle {
            color: Some(rgb(TEXT).into()),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        };
        row()
            .items_start()
            .gap(px(SPACE_3))
            .child(
                row()
                    .size(px(AVATAR_SIZE))
                    .flex_shrink_0()
                    .justify_center()
                    .child(icon(glyph, ICON_SIZE_SM).text_color(rgb(TEXT_TERTIARY))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pt(px(SPACE_HALF))
                    .text_size(type_size(LABEL_SIZE))
                    .text_color(rgb(TEXT_SECONDARY))
                    .child(StyledText::new(text).with_highlights([(0..actor.len(), emphasis)])),
            )
            .child(
                div()
                    .pt(px(SPACE_HALF))
                    .child(hint(time::relative(entry.created_at, now))),
            )
    }

    fn comment_row(&self, comment: &Comment, now: i64) -> Stateful<Div> {
        let author = self.assignee_name(&comment.author_id);
        list_item(("comment", comment.id as u64), comment.body.clone())
            .items_start()
            .accessibility_id(format!("comment.{}", comment.id))
            .gap(px(SPACE_3))
            .child(assignee_avatar(
                &self.assignee_face(&comment.author_id),
                AVATAR_SIZE,
            ))
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(SPACE_1))
                    .child(
                        row()
                            .h(px(AVATAR_SIZE))
                            .gap(px(SPACE_2))
                            .child(
                                div()
                                    .text_size(type_size(LABEL_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(author),
                            )
                            .child(hint(time::relative(comment.created_at, now))),
                    )
                    .child(
                        div()
                            .px(px(SPACE_3))
                            .py(px(SPACE_2))
                            .rounded(px(RADIUS_MD))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .text_size(type_size(BODY_SIZE))
                            .line_height(relative(BODY_LINE_HEIGHT))
                            .child(comment.body.clone()),
                    ),
            )
    }

    fn properties(&self, ticket: &Ticket, ui: &mut Ui<Self>) -> Div {
        let id = ticket.id;
        let revision = ticket.revision;
        let enabled = !self.pending.busy();
        // Quiet selects bleed left by their inset, so their glyph starts on the
        // value column's edge, and end on the content edge with their fill.
        let width =
            PROPERTIES_WIDTH - 2. * SPACE_4 - PROPERTY_LABEL_WIDTH - SPACE_3 + CONTROL_INSET_X;
        let current_status = ticket.status;
        let current_priority = ticket.priority;
        let current_assignee = ticket.assignee_id.clone();
        let status = Select::new(
            "tickets.detail.status",
            STATUSES
                .map(|s| SelectOption::new(status_name(s)).glyph(status_icon(s), status_color(s)))
                .into(),
        )
        .value(STATUSES.iter().position(|s| *s == ticket.status))
        .open(self.overlays.popover_open("tickets.detail.status"))
        .enabled(enabled)
        .quiet()
        .width(width)
        .build(
            ui,
            |this: &mut Self, _, cx| this.toggle_menu("tickets.detail.status", cx),
            move |this: &mut Self, index, _, cx| {
                this.overlays.close_popover();
                if let Some(status) = STATUSES
                    .get(index)
                    .copied()
                    .filter(|s| *s != current_status)
                {
                    this.command(
                        TicketCommand::SetStatus {
                            id,
                            revision,
                            status,
                        },
                        cx,
                    );
                }
            },
        );
        let priority = Select::new(
            "tickets.detail.priority",
            PRIORITIES
                .map(|p| {
                    SelectOption::new(priority_name(p)).glyph(priority_icon(p), priority_color(p))
                })
                .into(),
        )
        .value(PRIORITIES.iter().position(|p| *p == ticket.priority))
        .open(self.overlays.popover_open("tickets.detail.priority"))
        .enabled(enabled)
        .quiet()
        .width(width)
        .build(
            ui,
            |this: &mut Self, _, cx| this.toggle_menu("tickets.detail.priority", cx),
            move |this: &mut Self, index, _, cx| {
                this.overlays.close_popover();
                if let Some(priority) = PRIORITIES
                    .get(index)
                    .copied()
                    .filter(|p| *p != current_priority)
                {
                    this.command(
                        TicketCommand::SetPriority {
                            id,
                            revision,
                            priority,
                        },
                        cx,
                    );
                }
            },
        );
        let assignees = self.state.assignees.clone();
        let assignee = Select::new(
            "tickets.detail.assignee",
            assignees
                .iter()
                .map(|a| self.assignee_option(&a.id))
                .collect(),
        )
        .value(assignees.iter().position(|a| a.id == ticket.assignee_id))
        .open(self.overlays.popover_open("tickets.detail.assignee"))
        .enabled(enabled)
        .quiet()
        .width(width)
        .build(
            ui,
            |this: &mut Self, _, cx| this.toggle_menu("tickets.detail.assignee", cx),
            move |this: &mut Self, index, _, cx| {
                this.overlays.close_popover();
                // Choosing the current assignee again would restart their work.
                if let Some(chosen) = assignees.get(index).filter(|a| a.id != current_assignee) {
                    this.command(
                        TicketCommand::Assign {
                            id,
                            revision,
                            assignee_kind: chosen.kind,
                            assignee_id: chosen.id.clone(),
                        },
                        cx,
                    );
                }
            },
        );
        let bleed = |select: Div| select.ml(px(-CONTROL_INSET_X)).flex_shrink_0();
        card()
            .debug_selector(|| "tickets.properties".into())
            .px(px(SPACE_4))
            .py(px(SPACE_1))
            .gap(px(SPACE_1))
            .child(property_row("Status", bleed(status)))
            .child(property_row("Priority", bleed(priority)))
            .child(property_row("Assignee", bleed(assignee)))
            .child(property_row(
                "Labels",
                self.label_picker(&ticket.labels, "tickets.labels", ui),
            ))
            .child(divider().my(px(SPACE_3)))
            .child(self.relationships(ticket, ui))
            .child(divider().my(px(SPACE_3)))
            .child(self.work(ticket, ui))
            .child(divider().my(px(SPACE_3)))
            .child(property_row(
                "Created",
                caption(time::absolute(ticket.created_at)),
            ))
            .child(property_row(
                "Updated",
                caption(time::absolute(ticket.updated_at)),
            ))
    }

    fn relationships(&self, ticket: &Ticket, ui: &mut Ui<Self>) -> Div {
        let id = ticket.id;
        let found = relations(&self.state.links, id);
        column()
            .gap(px(SPACE_1))
            .child(
                row()
                    .h(px(CONTROL_HEIGHT_SM))
                    .justify_between()
                    .child(eyebrow("Relationships"))
                    .child(
                        Button::new("tickets.link", "Add Relationship")
                            .ghost()
                            .small()
                            .icon(Icon::Plus)
                            .icon_only()
                            .tint(TEXT_SECONDARY)
                            .enabled(!self.pending.busy() && self.state.tickets.len() > 1)
                            .build(ui, move |this: &mut Self, window, cx| {
                                this.link_relation = Relation::BlockedBy;
                                this.link_target = None;
                                this.form_error = None;
                                this.link_search.update(cx, |input, _| input.reset());
                                let focus = this.link_search.focus_handle(cx);
                                this.overlays
                                    .open_dialog(Dialog::Link(id), focus, window, cx);
                                cx.notify();
                            })
                            .mr(px(-TRAILING_ICON_BLEED)),
                    ),
            )
            .when(found.is_empty(), |s| {
                s.child(hint(
                    "Nothing blocks, relates to or duplicates this Ticket.",
                ))
            })
            .children({
                // Grouped under their relation, so each Ticket gets the full row.
                let mut rows: Vec<AnyElement> = Vec::new();
                let mut previous = None;
                for (relation, other) in found {
                    if previous != Some(relation) {
                        previous = Some(relation);
                        let open_blocker = relation == Relation::BlockedBy
                            && !matches!(
                                self.ticket(other).map(|t| t.status),
                                Some(TicketStatus::Done | TicketStatus::Cancelled)
                            );
                        rows.push(
                            div()
                                .pt(px(SPACE_2))
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(if open_blocker {
                                    STATUS_RED
                                } else {
                                    TEXT_TERTIARY
                                }))
                                .child(relation.name())
                                .into_any_element(),
                        );
                    }
                    rows.push(self.related_row(id, relation, other, ui).into_any_element());
                }
                rows
            })
    }

    /// One related Ticket: open it, or remove the relationship.
    fn related_row(&self, id: i64, relation: Relation, other: i64, ui: &mut Ui<Self>) -> Div {
        let title = self
            .ticket(other)
            .map_or_else(String::new, |t| t.title.clone());
        let other_status = self.ticket(other).map(|t| t.status);
        let (from, to, link) = relation.link(id, other);
        let mut related = Button::new(
            ElementId::NamedInteger("tickets.related".into(), other as u64),
            format!("{} {title}", ticket_key(other)),
        )
        .ghost()
        .small()
        .full_width()
        .align_start();
        if let Some(status) = other_status {
            related = related.leading(
                icon(status_icon(status), ICON_SIZE_XS).text_color(rgb(status_color(status))),
            );
        }
        row()
            .min_h(px(PROPERTY_ROW_HEIGHT))
            .gap(px(SPACE_1))
            .child(
                div().flex_1().min_w_0().child(
                    related
                        .build(ui, move |this: &mut Self, _, cx| this.select(other, cx))
                        .ml(px(-CONTROL_INSET_X_SM)),
                ),
            )
            .child(
                Button::new(
                    ElementId::NamedInteger("tickets.unlink".into(), other as u64),
                    format!("Remove {} {}", relation.name(), ticket_key(other)),
                )
                .ghost()
                .small()
                .icon(Icon::Close)
                .icon_only()
                .tint(TEXT_TERTIARY)
                .enabled(!self.pending.busy())
                .build(ui, move |this: &mut Self, _, cx| {
                    this.command(
                        TicketCommand::Unlink {
                            from_id: from,
                            to_id: to,
                            link,
                        },
                        cx,
                    )
                })
                .mr(px(-TRAILING_ICON_BLEED)),
            )
    }

    /// The Work behind this Ticket and the Conversation it came from.
    fn work(&self, ticket: &Ticket, ui: &mut Ui<Self>) -> Div {
        let mut runs: Vec<_> = self
            .state
            .runs
            .iter()
            .filter(|r| r.ticket_id == ticket.id)
            .collect();
        runs.sort_by_key(|r| std::cmp::Reverse(r.generation));
        let conversation = ticket.conversation_id.map(|id| {
            let title = self
                .conversation_titles
                .iter()
                .find(|(other, _)| *other == id)
                .map_or_else(|| "Conversation".to_owned(), |(_, title)| title.clone());
            (id, title)
        });
        column()
            .gap(px(SPACE_1))
            .child(
                row()
                    .h(px(CONTROL_HEIGHT_SM))
                    .justify_between()
                    .child(eyebrow("Work"))
                    .when(!runs.is_empty(), |s| {
                        s.child(
                            Button::new("tickets.runs", "All Work")
                                .ghost()
                                .small()
                                .tint(TEXT_SECONDARY)
                                .trailing(icon(Icon::ArrowUpRight, ICON_SIZE_SM))
                                .build(ui, |_: &mut Self, _, cx| {
                                    cx.emit(Destination::Page(Route::Temporal))
                                })
                                .mr(px(-CONTROL_INSET_X_SM)),
                        )
                    }),
            )
            .when(runs.is_empty(), |s| {
                s.child(hint(if self.is_agent(&ticket.assignee_id) {
                    "Starts in To do or In progress."
                } else {
                    "Assign an agent to start work."
                }))
            })
            .when(!runs.is_empty(), |s| {
                s.child(hint("A Ticket with Work cannot be deleted."))
            })
            .children(runs.into_iter().take(RECENT_WORK).map(|run| {
                let tone = state_tone(&run.state);
                let label = state_label(&run.state);
                row()
                    .debug_selector({
                        let id = run.run_id.clone();
                        move || format!("tickets.run.{id}")
                    })
                    .min_h(px(PROPERTY_ROW_HEIGHT))
                    .gap(px(SPACE_3))
                    .child(
                        div()
                            .w(px(PROPERTY_LABEL_WIDTH))
                            .flex_shrink_0()
                            .child(caption(format!("Work {}", run.generation))),
                    )
                    .child(status_pill(label.to_owned(), tone))
                    .when_some(run.error.clone(), |s, error| {
                        s.child(div().flex_1().min_w_0().truncate().child(hint(error)))
                    })
            }))
            .when_some(conversation, |s, (conversation, title)| {
                s.child(property_row(
                    "From",
                    Button::new("tickets.conversation", title)
                        .ghost()
                        .small()
                        .full_width()
                        .align_start()
                        .icon(Icon::Spark)
                        .build(ui, move |_: &mut Self, _, cx| {
                            cx.emit(Destination::Conversation(conversation))
                        })
                        .ml(px(-CONTROL_INSET_X_SM)),
                ))
            })
    }
}
