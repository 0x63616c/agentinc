//! The list view: one table, grouped by status in board order, for scanning
//! many Tickets at once.
use super::*;

/// Labels shown in a row before the title needs the room.
const LIST_LABELS: usize = 2;

fn columns() -> [TableColumn; 3] {
    [
        TableColumn::new("Ticket"),
        TableColumn::new("Assignee").width(LIST_ASSIGNEE_WIDTH),
        TableColumn::new("Updated")
            .width(LIST_UPDATED_WIDTH)
            .right(),
    ]
}

pub(super) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl TicketsPage {
    pub(super) fn list(&self, visible: &[Ticket], cx: &mut Context<Self>) -> Div {
        if visible.is_empty() {
            return EmptyState::new("search", "No matching Tickets")
                .description("Try another search, or clear the filters to see every Ticket.")
                .selector("tickets.list.empty")
                .action(
                    Button::new("tickets.list.clear", "Clear filters")
                        .secondary()
                        .build(
                            &self.hover,
                            |this: &mut Self, _, cx| {
                                this.filters = Filters::default();
                                this.search.update(cx, |input, cx| {
                                    input.reset();
                                    cx.notify();
                                });
                                cx.notify();
                            },
                            cx,
                        ),
                )
                .build();
        }
        let columns = columns();
        let now = now();
        let mut table = table_container()
            .debug_selector(|| "tickets.list".into())
            .child(table_header(&columns));
        for status in STATUSES {
            let tickets = in_column(visible, status);
            if tickets.is_empty() {
                continue;
            }
            let key = status_key(status);
            table = table.child(
                row()
                    .debug_selector(move || format!("tickets.group.{key}"))
                    .w_full()
                    .h(px(TABLE_HEADER_HEIGHT))
                    .pl(px(SPACE_4))
                    .pr(px(SPACE_2))
                    .gap(px(SPACE_2))
                    .bg(rgb(SURFACE))
                    .border_b_1()
                    .border_color(rgb(BORDER_SUBTLE))
                    .child(
                        icon(status_icon(status), ICON_SIZE_SM)
                            .text_color(rgb(status_color(status))),
                    )
                    .child(
                        div()
                            .text_size(type_size(LABEL_SIZE))
                            .font_weight(FontWeight::MEDIUM)
                            .child(status_name(status)),
                    )
                    .child(hint(tickets.len().to_string()))
                    .child(div().flex_1())
                    .child(
                        Button::new(
                            SharedString::from(format!("tickets.group.{key}.create")),
                            format!("New Ticket in {}", status_name(status)),
                        )
                        .ghost()
                        .small()
                        .icon("plus")
                        .icon_only()
                        .tint(TEXT_TERTIARY)
                        .enabled(self.store.is_some() && !self.pending)
                        .build(
                            &self.hover,
                            move |this: &mut Self, window, cx| {
                                this.open_create(Some(status), window, cx)
                            },
                            cx,
                        ),
                    ),
            );
            for ticket in tickets {
                table = table.child(self.list_row(ticket, &columns, now, cx));
            }
        }
        column().w_full().child(table)
    }

    fn list_row(
        &self,
        ticket: &Ticket,
        columns: &[TableColumn],
        now: i64,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = ticket.id;
        let blockers = open_blockers(&self.state.tickets, &self.state.links, id);
        let summary = row()
            .w_full()
            .gap(px(SPACE_3))
            .child(
                icon(priority_icon(ticket.priority), ICON_SIZE_SM)
                    .text_color(rgb(priority_color(ticket.priority))),
            )
            .child(
                row()
                    .w(px(LIST_KEY_WIDTH))
                    .flex_shrink_0()
                    .gap(px(SPACE_1))
                    .child(hint(ticket_key(id)))
                    .when(self.running(ticket), |s| s.child(status_dot(Tone::Info))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(type_size(BODY_SIZE))
                    .child(ticket.title.clone()),
            )
            .when(!blockers.is_empty(), |s| {
                s.child(status_pill(
                    format!("Blocked by {}", ticket_key(blockers[0])),
                    Tone::Danger,
                ))
            })
            .children(
                ticket
                    .labels
                    .iter()
                    .take(LIST_LABELS)
                    .map(|label| tag(label.clone(), label_color(label))),
            );
        let assignee = row()
            .gap(px(SPACE_2))
            .child(self.assignee_avatar(&ticket.assignee_id, AVATAR_SIZE_SM))
            .child(
                div()
                    .truncate()
                    .text_size(type_size(LABEL_SIZE))
                    .text_color(rgb(TEXT_SECONDARY))
                    .child(self.assignee_name(&ticket.assignee_id)),
            );
        table_row(
            ElementId::NamedInteger("ticket".into(), id as u64),
            format!("{} {}", ticket_key(id), ticket.title),
            columns,
            vec![
                summary.into_any_element(),
                assignee.into_any_element(),
                hint(relative_time(ticket.updated_at, now)).into_any_element(),
            ],
            true,
            &self.hover,
            move |this: &mut Self, _, cx| this.select(id, cx),
            cx,
        )
    }
}
