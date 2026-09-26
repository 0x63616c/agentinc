//! One Ticket: its title as the page heading, the description, a timeline of
//! history and Comments with the composer, and a properties column for status,
//! priority, assignee, labels, relationships, work and where it came from.
use super::*;
use crate::storage::{ActivityKind, Comment};

/// Attempts listed under Work, newest first.
const RECENT_RUNS: usize = 3;

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
    pub(super) fn detail(
        &mut self,
        ticket: &Ticket,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let beside = window.viewport_size().width >= px(PROPERTIES_BESIDE_MIN_WIDTH);
        let header = self.detail_header(ticket, cx);
        // Lower the main column so its first heading shares a line with the first
        // property label inside the card beside it.
        let main = column()
            .flex_1()
            .min_w_0()
            .pt(px(SPACE_2))
            .gap(px(SECTION_GAP))
            .child(self.description_section(ticket, window, cx))
            .child(self.timeline(ticket, window, cx));
        let properties = self.properties(ticket, window, cx);
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
        Page::document(header)
            .child(
                column()
                    .id("tickets-page")
                    .track_focus(&self.page_focus)
                    .gap(px(SPACE_4))
                    .when_some(self.error.clone(), |s, error| {
                        s.child(banner(Tone::Danger, error))
                    })
                    .when_some(
                        self.form_error
                            .clone()
                            .filter(|_| self.overlays.borrow().active().is_none()),
                        |s, error| s.child(banner(Tone::Danger, error)),
                    )
                    .child(body),
            )
            .build()
    }

    /// A way back, the title as the page's one heading, a meta line and the
    /// actions that apply to this Ticket.
    fn detail_header(&self, ticket: &Ticket, cx: &mut Context<Self>) -> PageHeader {
        let id = ticket.id;
        let revision = ticket.revision;
        let running = self.running(ticket);
        let has_runs = self.state.runs.iter().any(|r| r.ticket_id == id);
        let enabled = !self.pending;
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
                    .icon("chevronLeft")
                    .tint(TEXT_SECONDARY)
                    .build(&self.hover, |this, _, cx| this.close_detail(cx), cx)
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
                            Button::new("tickets.stop", "Stop work")
                                .secondary()
                                .icon("stop")
                                .enabled(enabled)
                                .build(
                                    &self.hover,
                                    move |this, _, cx| {
                                        this.command(TicketCommand::Cancel { id, revision }, cx)
                                    },
                                    cx,
                                ),
                        )
                    })
                    .when(!has_runs, |s| {
                        s.child(
                            Button::new("tickets.delete", "Delete")
                                .ghost()
                                .icon("trash")
                                .tint(TEXT_SECONDARY)
                                .enabled(enabled)
                                .build(
                                    &self.hover,
                                    move |this, window, cx| {
                                        this.overlays.borrow_mut().open(
                                            Overlay::DeleteTicket(id),
                                            window,
                                            cx,
                                            Some(this.cancel_focus.clone()),
                                        );
                                        cx.notify();
                                    },
                                    cx,
                                ),
                        )
                    })
                    .child(
                        Button::new("tickets.rename", "Rename")
                            .ghost()
                            .icon("edit")
                            .tint(TEXT_SECONDARY)
                            .enabled(enabled)
                            .build(
                                &self.hover,
                                move |this: &mut Self, window, cx| {
                                    let title = this.ticket(id).map(|t| t.title.clone());
                                    this.rename.update(cx, |input, cx| {
                                        input.set_text(&title.unwrap_or_default(), cx)
                                    });
                                    let focus = this.rename.focus_handle(cx);
                                    this.form_error = None;
                                    this.overlays.borrow_mut().open(
                                        Overlay::RenameTicket(id),
                                        window,
                                        cx,
                                        Some(focus),
                                    );
                                    cx.notify();
                                },
                                cx,
                            ),
                    ),
            )
    }

    fn description_section(&self, ticket: &Ticket, window: &Window, cx: &mut Context<Self>) -> Div {
        let id = ticket.id;
        let revision = ticket.revision;
        let body: AnyElement = if self.editing_description {
            column()
                .gap(px(SPACE_3))
                .child(
                    Field::new(self.description.clone())
                        .multiline()
                        .selector("tickets.description")
                        .build(window, cx),
                )
                .child(
                    row()
                        .gap(px(CONTROL_GAP))
                        .justify_end()
                        .child(
                            Button::new("tickets.description.cancel", "Cancel")
                                .secondary()
                                .small()
                                .build(
                                    &self.hover,
                                    |this: &mut Self, _, cx| {
                                        this.editing_description = false;
                                        cx.notify();
                                    },
                                    cx,
                                ),
                        )
                        .child(
                            Button::new("tickets.description.save", "Save")
                                .primary()
                                .small()
                                .enabled(!self.pending)
                                .build(
                                    &self.hover,
                                    move |this: &mut Self, _, cx| {
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
                                    },
                                    cx,
                                ),
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
                                .enabled(!self.pending)
                                .build(
                                    &self.hover,
                                    move |this: &mut Self, window, cx| {
                                        this.description
                                            .update(cx, |input, cx| input.set_text(&text, cx));
                                        this.editing_description = true;
                                        window.focus(&this.description.focus_handle(cx), cx);
                                        cx.notify();
                                    },
                                    cx,
                                )
                                .mr(px(-CONTROL_INSET_X_SM)),
                        )
                    }),
            )
            .child(body)
    }

    fn timeline(&self, ticket: &Ticket, window: &Window, cx: &mut Context<Self>) -> Div {
        let now = list::now();
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
                    .child(self.assignee_avatar("owner", AVATAR_SIZE))
                    .child(
                        div().flex_1().min_w_0().child(
                            Field::new(self.comment.clone())
                                .selector("Add a Comment")
                                .build(window, cx),
                        ),
                    )
                    .child(
                        Button::new("tickets.post", "Post")
                            .primary()
                            .enabled(
                                !self.pending && !self.comment.read(cx).content.trim().is_empty(),
                            )
                            .build(&self.hover, |this, _, cx| this.add_comment(cx), cx),
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
        let (glyph, action) = self.describe(entry);
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
                    .child(hint(relative_time(entry.created_at, now))),
            )
    }

    /// A glyph and the rest of the sentence after the actor's name.
    fn describe(&self, entry: &TicketActivity) -> (&'static str, String) {
        let from = entry.from_value.as_deref().unwrap_or_default();
        let to = entry.to_value.as_deref().unwrap_or_default();
        let other = |value: &str| {
            value
                .parse::<i64>()
                .map(ticket_key)
                .unwrap_or_else(|_| value.to_owned())
        };
        match entry.kind {
            ActivityKind::Created => ("plus", "created the Ticket".into()),
            ActivityKind::Renamed => ("edit", format!("renamed it from “{from}”")),
            ActivityKind::Described => ("edit", "updated the description".into()),
            ActivityKind::Status => match (status_from_key(from), status_from_key(to)) {
                (Some(from), Some(to)) => (
                    status_icon(to),
                    format!("moved it from {} to {}", status_name(from), status_name(to)),
                ),
                _ => ("history", "changed the status".into()),
            },
            ActivityKind::Priority => match priority_from_key(to) {
                Some(TicketPriority::None) => ("priority-none", "removed the priority".into()),
                Some(priority) => (
                    priority_icon(priority),
                    format!("set priority to {}", priority_name(priority)),
                ),
                None => ("history", "changed the priority".into()),
            },
            ActivityKind::Assigned => {
                ("user", format!("assigned it to {}", self.assignee_name(to)))
            }
            ActivityKind::Labels => {
                let split = |value: &str| -> Vec<String> {
                    value
                        .split(',')
                        .filter(|label| !label.is_empty())
                        .map(str::to_owned)
                        .collect()
                };
                let (before, after) = (split(from), split(to));
                let added: Vec<_> = after
                    .iter()
                    .filter(|l| !before.contains(l))
                    .cloned()
                    .collect();
                let removed: Vec<_> = before
                    .iter()
                    .filter(|l| !after.contains(l))
                    .cloned()
                    .collect();
                let sentence = match (added.is_empty(), removed.is_empty()) {
                    (false, true) => format!("added {}", added.join(", ")),
                    (true, false) => format!("removed {}", removed.join(", ")),
                    _ => "changed the labels".to_owned(),
                };
                ("tag", sentence)
            }
            ActivityKind::Linked | ActivityKind::Unlinked => {
                let key = other(to);
                let sentence = match (entry.kind, Relation::from_history(from)) {
                    (ActivityKind::Linked, Some(Relation::Blocks)) => {
                        format!("marked it as blocking {key}")
                    }
                    (ActivityKind::Linked, Some(Relation::BlockedBy)) => {
                        format!("marked it as blocked by {key}")
                    }
                    (ActivityKind::Linked, Some(Relation::Duplicates)) => {
                        format!("marked it as a duplicate of {key}")
                    }
                    (ActivityKind::Linked, Some(Relation::DuplicatedBy)) => {
                        format!("marked {key} as a duplicate of it")
                    }
                    (ActivityKind::Linked, Some(Relation::Parent)) => {
                        format!("made it a Sub-Ticket of {key}")
                    }
                    (ActivityKind::Linked, Some(Relation::SubIssue)) => {
                        format!("added {key} as a Sub-Ticket")
                    }
                    (ActivityKind::Linked, _) => format!("related it to {key}"),
                    (_, relation) => format!(
                        "removed its link to {key}{}",
                        relation
                            .map_or(String::new(), |r| format!(" ({})", r.name().to_lowercase()))
                    ),
                };
                ("link", sentence)
            }
            ActivityKind::Work => (
                "play",
                match to {
                    "queued" => "started work".into(),
                    "completed" => "finished the work".into(),
                    "failed" => "stopped: the work failed".into(),
                    "cancelled" => "cancelled the work".into(),
                    state => format!("work is {state}"),
                },
            ),
        }
    }

    fn comment_row(&self, comment: &Comment, now: i64) -> Stateful<Div> {
        let author = self.assignee_name(&comment.author_id);
        list_item(("comment", comment.id as u64), comment.body.clone())
            .items_start()
            .accessibility_id(format!("comment.{}", comment.id))
            .gap(px(SPACE_3))
            .child(self.assignee_avatar(&comment.author_id, AVATAR_SIZE))
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
                            .child(hint(relative_time(comment.created_at, now))),
                    )
                    .child(
                        div()
                            .px(px(SPACE_3))
                            .py(px(SPACE_2))
                            .rounded(px(RADIUS_MD))
                            .bg(rgb(SURFACE_RAISED))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .text_size(type_size(BODY_SIZE))
                            .line_height(relative(BODY_LINE_HEIGHT))
                            .child(comment.body.clone()),
                    ),
            )
    }

    fn properties(&self, ticket: &Ticket, window: &Window, cx: &mut Context<Self>) -> Div {
        let id = ticket.id;
        let revision = ticket.revision;
        let enabled = !self.pending;
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
        .open(self.menu == Some(Menu::Status))
        .enabled(enabled)
        .quiet()
        .width(width)
        .build(
            &self.hover,
            |this: &mut Self, _, cx| this.toggle_menu(Menu::Status, cx),
            move |this: &mut Self, index, _, cx| {
                this.menu = None;
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
            cx,
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
        .open(self.menu == Some(Menu::Priority))
        .enabled(enabled)
        .quiet()
        .width(width)
        .build(
            &self.hover,
            |this: &mut Self, _, cx| this.toggle_menu(Menu::Priority, cx),
            move |this: &mut Self, index, _, cx| {
                this.menu = None;
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
            cx,
        );
        let assignees = self.state.assignees.clone();
        let assignee = Select::new(
            "tickets.detail.assignee",
            assignees
                .iter()
                .map(|a| {
                    let name = self.assignee_name(&a.id);
                    SelectOption::new(name.clone()).avatar(name, a.kind == AssigneeKind::Agent)
                })
                .collect(),
        )
        .value(assignees.iter().position(|a| a.id == ticket.assignee_id))
        .open(self.menu == Some(Menu::Assignee))
        .enabled(enabled)
        .quiet()
        .width(width)
        .build(
            &self.hover,
            |this: &mut Self, _, cx| this.toggle_menu(Menu::Assignee, cx),
            move |this: &mut Self, index, _, cx| {
                this.menu = None;
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
            cx,
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
                self.label_picker(&ticket.labels, Menu::Labels, window, cx),
            ))
            .child(divider().my(px(SPACE_3)))
            .child(self.relationships(ticket, cx))
            .child(divider().my(px(SPACE_3)))
            .child(self.work(ticket, cx))
            .child(divider().my(px(SPACE_3)))
            .child(property_row(
                "Created",
                caption(timestamp(ticket.created_at)),
            ))
            .child(property_row(
                "Updated",
                caption(timestamp(ticket.updated_at)),
            ))
    }

    fn relationships(&self, ticket: &Ticket, cx: &mut Context<Self>) -> Div {
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
                        Button::new("tickets.link", "Add relationship")
                            .ghost()
                            .small()
                            .icon("plus")
                            .icon_only()
                            .tint(TEXT_SECONDARY)
                            .enabled(!self.pending && self.state.tickets.len() > 1)
                            .build(
                                &self.hover,
                                move |this: &mut Self, window, cx| {
                                    this.link_relation = Relation::BlockedBy;
                                    this.link_target = None;
                                    this.form_error = None;
                                    this.link_search.update(cx, |input, _| input.reset());
                                    let focus = this.link_search.focus_handle(cx);
                                    this.overlays.borrow_mut().open(
                                        Overlay::LinkTicket(id),
                                        window,
                                        cx,
                                        Some(focus),
                                    );
                                    cx.notify();
                                },
                                cx,
                            )
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
                    rows.push(self.related_row(id, relation, other, cx).into_any_element());
                }
                rows
            })
    }

    /// One related Ticket: open it, or remove the relationship.
    fn related_row(&self, id: i64, relation: Relation, other: i64, cx: &mut Context<Self>) -> Div {
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
                        .build(
                            &self.hover,
                            move |this: &mut Self, _, cx| this.select(other, cx),
                            cx,
                        )
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
                .icon("close")
                .icon_only()
                .tint(TEXT_TERTIARY)
                .enabled(!self.pending)
                .build(
                    &self.hover,
                    move |this: &mut Self, _, cx| {
                        this.command(
                            TicketCommand::Unlink {
                                from_id: from,
                                to_id: to,
                                link,
                            },
                            cx,
                        )
                    },
                    cx,
                )
                .mr(px(-TRAILING_ICON_BLEED)),
            )
    }

    /// The agent runs behind this Ticket and the Conversation it came from.
    fn work(&self, ticket: &Ticket, cx: &mut Context<Self>) -> Div {
        let mut runs: Vec<_> = self
            .state
            .runs
            .iter()
            .filter(|r| r.ticket_id == ticket.id)
            .collect();
        runs.sort_by_key(|r| std::cmp::Reverse(r.generation));
        let conversation = ticket.conversation_id.map(|id| {
            let title = self
                .store
                .as_ref()
                .and_then(|store| {
                    store
                        .snapshot()
                        .conversations
                        .into_iter()
                        .find(|c| c.id == id)
                        .map(|c| c.title)
                })
                .unwrap_or_else(|| "Conversation".into());
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
                            Button::new("tickets.runs", "Runs")
                                .ghost()
                                .small()
                                .tint(TEXT_SECONDARY)
                                .trailing(icon("arrowUpRight", ICON_SIZE_SM))
                                .build(
                                    &self.hover,
                                    |_: &mut Self, _, cx| cx.emit(TicketsEvent::OpenRuns),
                                    cx,
                                )
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
            .children(runs.into_iter().take(RECENT_RUNS).map(|run| {
                let tone = match run.state.as_str() {
                    "running" => Tone::Info,
                    "completed" => Tone::Success,
                    "failed" => Tone::Danger,
                    "cancelled" => Tone::Neutral,
                    _ => Tone::Warning,
                };
                let label = match run.state.as_str() {
                    "queued" => "Queued",
                    "running" => "Running",
                    "completed" => "Completed",
                    "failed" => "Failed",
                    "cancelled" => "Cancelled",
                    other => other,
                };
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
                            .child(caption(format!("Attempt {}", run.generation))),
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
                        .icon("spark")
                        .build(
                            &self.hover,
                            move |_: &mut Self, _, cx| {
                                cx.emit(TicketsEvent::OpenConversation(conversation))
                            },
                            cx,
                        )
                        .ml(px(-CONTROL_INSET_X_SM)),
                ))
            })
    }
}
