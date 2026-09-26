//! The Kanban board: one lane per status and cards that drag between and within
//! lanes. The drop indicator is placed from the cards' painted positions, the
//! move applies locally at once and the daemon's answer replaces it.
use super::*;
use std::{cell::Cell, collections::HashMap};

/// Labels shown on a card before the rest collapse into a count.
const CARD_LABELS: usize = 2;

/// A card being dragged. It is also the view that follows the pointer.
#[derive(Clone)]
pub(crate) struct DraggedTicket {
    id: i64,
    key: SharedString,
    title: SharedString,
    priority: TicketPriority,
    labels: Vec<String>,
    /// The assignee's name, photo and whether it is an agent.
    assignee: (SharedString, Option<Arc<Image>>, bool),
    width: Pixels,
    /// Where the pointer picked the card up, inside it.
    grab: Point<Pixels>,
}
impl Render for DraggedTicket {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Lifted off the board to just below and right of the pointer, so the
        // drop line under the pointer stays in view. A drag view's root ignores
        // margins, so the offset is padding on a wrapper.
        div()
            .pt(self.grab.y + px(DRAG_LIFT))
            .pl(self.grab.x + px(DRAG_LIFT))
            .child(self.card())
    }
}
impl DraggedTicket {
    fn card(&self) -> Div {
        let (name, photo, agent) = &self.assignee;
        column()
            .w(self.width)
            .p(px(SPACE_3))
            .gap(px(SPACE_1))
            .rounded(px(RADIUS_MD))
            .bg(rgb(SURFACE_OVERLAY))
            .border_1()
            .border_color(rgb(BORDER_STRONG))
            .shadow(shadow_dialog())
            .text_color(rgb(TEXT))
            .gap(px(SPACE_2))
            .child(
                row()
                    .w_full()
                    .child(hint(self.key.clone()))
                    .child(div().flex_1())
                    .child(if *agent {
                        agent_avatar(name, AVATAR_SIZE_SM)
                    } else {
                        avatar(name, photo.clone(), AVATAR_SIZE_SM)
                    }),
            )
            .child(
                div()
                    .w_full()
                    .text_size(type_size(BODY_SIZE))
                    .line_height(relative(TITLE_LINE_HEIGHT))
                    .line_clamp(3)
                    .text_ellipsis()
                    .child(self.title.clone()),
            )
            .when(
                self.priority != TicketPriority::None || !self.labels.is_empty(),
                |s| {
                    s.child(
                        row()
                            .flex_wrap()
                            .gap(px(CHIP_GAP))
                            .when(self.priority != TicketPriority::None, |s| {
                                s.child(priority_chip(self.priority))
                            })
                            .children(
                                self.labels
                                    .iter()
                                    .take(CARD_LABELS)
                                    .map(|label| tag(label.clone(), label_color(label))),
                            ),
                    )
                },
            )
    }
}

/// Where a dropped card lands: directly below `after`, or first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DropTarget {
    pub status: TicketStatus,
    pub after: Option<i64>,
}

#[derive(Default)]
pub(crate) struct DragState {
    /// Each lane's card bounds from the last paint, top first.
    geometry: Rc<Geometry>,
    /// The card under the pointer, set when its drag starts.
    dragging: Rc<Cell<Option<i64>>>,
    target: Option<DropTarget>,
    /// The card that just landed, for its settle highlight.
    landed: Option<(i64, Instant)>,
}
impl DragState {
    /// Clear drag state once the gesture ends and keep the landing highlight moving.
    pub(super) fn settle(&mut self, window: &mut Window, cx: &App) {
        if !cx.has_active_drag() {
            self.target = None;
            self.dragging.set(None);
        }
        if let Some((_, at)) = self.landed {
            if at.elapsed().as_millis() < u128::from(SETTLE_MS) && !reduced_motion() {
                window.request_animation_frame();
            } else {
                self.landed = None;
            }
        }
    }
    /// The card a pointer at `y` would drop below, ignoring the dragged card.
    fn after(&self, status: TicketStatus, dragged: i64, y: Pixels) -> Option<i64> {
        let geometry = self.geometry.borrow();
        geometry
            .get(status_key(status))?
            .iter()
            .filter(|(id, _)| *id != dragged)
            .take_while(|(_, bounds)| bounds.center().y < y)
            .last()
            .map(|(id, _)| *id)
    }
}

impl TicketsPage {
    pub(super) fn board(
        &self,
        visible: &[Ticket],
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let dragging = self.drag.dragging.get().filter(|_| cx.has_active_drag());
        div()
            .id("tickets.board")
            .debug_selector(|| "tickets.board".into())
            .flex()
            .flex_row()
            .size_full()
            .min_h_0()
            .gap(px(SPACE_3))
            .overflow_x_scroll()
            .children(
                STATUSES.map(|status| self.lane(status, in_column(visible, status), dragging, cx)),
            )
    }

    fn lane(
        &self,
        status: TicketStatus,
        cards: Vec<&Ticket>,
        dragging: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let key = status_key(status);
        let ids: Vec<i64> = cards.iter().map(|t| t.id).collect();
        let over = self.drag.target.filter(|t| t.status == status);
        // No indicator where a drop would leave the card exactly where it is.
        let indicator = over.filter(|target| {
            let Some(index) = dragging.and_then(|id| ids.iter().position(|x| *x == id)) else {
                return true;
            };
            target.after != index.checked_sub(1).map(|above| ids[above])
        });
        let geometry = self.drag.geometry.clone();
        let painted = ids.clone();
        let body = div()
            .flex()
            .flex_col()
            .on_children_prepainted(move |bounds, _, _| {
                // Child 0 is the slot above the first card.
                geometry.borrow_mut().insert(
                    key,
                    painted
                        .iter()
                        .copied()
                        .zip(bounds.into_iter().skip(1))
                        .collect(),
                );
            })
            .id(SharedString::from(format!("tickets.lane.{key}.cards")))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(BOARD_LANE_INSET))
            .child(
                drop_line(indicator.is_some_and(|t| t.after.is_none())).mb(px(
                    if indicator.is_some_and(|t| t.after.is_none()) {
                        SPACE_2
                    } else {
                        0.
                    },
                )),
            )
            .children(cards.iter().map(|ticket| {
                column()
                    .pb(px(SPACE_2))
                    .child(self.card(ticket, dragging == Some(ticket.id), cx))
                    .when(indicator.is_some_and(|t| t.after == Some(ticket.id)), |s| {
                        s.child(drop_line(true).mt(px(SPACE_2)))
                    })
            }))
            .when(cards.is_empty(), |s| {
                s.child(row().h(px(EMPTY_LANE_HEIGHT)).justify_center().child(hint(
                    if over.is_some() {
                        "Drop here"
                    } else {
                        "No Tickets"
                    },
                )))
            });
        let name = status_name(status);
        // Closed work is moved there, not created there.
        let creates = !matches!(status, TicketStatus::Done | TicketStatus::Cancelled);
        column()
            .id(SharedString::from(format!("tickets.lane.{key}")))
            .debug_selector(move || format!("tickets.lane.{key}"))
            .flex_1()
            .min_w(px(BOARD_LANE_MIN_WIDTH))
            .h_full()
            .min_h_0()
            .rounded(px(RADIUS_LG))
            .bg(rgb(if over.is_some() {
                SURFACE
            } else {
                SURFACE_SUNKEN
            }))
            // A lane is a well, not another box: its edge shows only while a card
            // is over it.
            .border_1()
            .border_color(rgb(if over.is_some() {
                BORDER_STRONG
            } else {
                SURFACE_SUNKEN
            }))
            .child(
                row()
                    .debug_selector(move || format!("tickets.lane.{key}.header"))
                    .h(px(LANE_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .pl(px(SPACE_3))
                    .pr(px(SPACE_1))
                    .gap(px(SPACE_2))
                    .child(
                        icon(status_icon(status), ICON_SIZE_SM)
                            .text_color(rgb(status_color(status))),
                    )
                    .child(
                        div()
                            .text_size(type_size(LABEL_SIZE))
                            .font_weight(FontWeight::MEDIUM)
                            .child(name),
                    )
                    .child(hint(cards.len().to_string()))
                    .child(div().flex_1())
                    .when(creates, |s| {
                        s.child(
                            Button::new(
                                SharedString::from(format!("tickets.lane.{key}.create")),
                                format!("New Ticket in {name}"),
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
                        )
                    }),
            )
            // The scroll area stops one inset above the lane's edge, so cards
            // never run into the bottom even while the lane scrolls.
            .child(
                column()
                    .flex_1()
                    .min_h_0()
                    .pb(px(BOARD_LANE_INSET))
                    .child(body),
            )
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<DraggedTicket>, _, cx| {
                    // Every lane hears every move; only the one under the pointer answers.
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    let dragged = event.drag(cx).id;
                    let target = Some(DropTarget {
                        status,
                        after: this.drag.after(status, dragged, event.event.position.y),
                    });
                    if this.drag.target != target {
                        this.drag.target = target;
                        cx.notify();
                    }
                }),
            )
            .on_drop(cx.listener(move |this, dragged: &DraggedTicket, _, cx| {
                this.drop_ticket(dragged.id, status, cx)
            }))
    }

    fn card(&self, ticket: &Ticket, ghost: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = ticket.id;
        let element = ElementId::NamedInteger("ticket".into(), id as u64);
        let (progress, on_hover) = self.hover.track(&element, true, cx);
        // Freshly landed cards settle from a bright edge to their resting border.
        let settle = self
            .drag
            .landed
            .filter(|(landed, _)| *landed == id)
            .map(|(_, at)| (at.elapsed().as_millis() as f32 / SETTLE_MS as f32).min(1.));
        let border = match settle {
            Some(t) => blend(FOCUS_FIELD, BORDER, t * t),
            None => blend(BORDER, BORDER_STRONG, progress),
        };
        let key: SharedString = ticket_key(id).into();
        let title: SharedString = ticket.title.clone().into();
        let footer = self.card_footer(ticket);
        let preview = DraggedTicket {
            id,
            key: key.clone(),
            title: title.clone(),
            priority: ticket.priority,
            labels: ticket.labels.clone(),
            assignee: (
                self.assignee_name(&ticket.assignee_id),
                (ticket.assignee_id == "owner")
                    .then(|| self.owner.1.clone())
                    .flatten(),
                self.is_agent(&ticket.assignee_id),
            ),
            width: px(BOARD_LANE_MIN_WIDTH),
            grab: Point::default(),
        };
        let dragging = self.drag.dragging.clone();
        let geometry = self.drag.geometry.clone();
        action_button(
            ButtonSpec {
                id: element,
                label: format!("{key} {title}").into(),
                enabled: true,
            },
            |card| {
                card.flex_col()
                    .items_start()
                    .w_full()
                    .p(px(SPACE_3))
                    .gap(px(SPACE_2))
                    .rounded(px(RADIUS_MD))
                    .bg(blend(SURFACE_RAISED, HOVER, progress))
                    .border_1()
                    .border_color(border)
                    .when(ghost, |s| s.opacity(GHOST_OPACITY))
                    .on_hover(on_hover)
                    .child(
                        row()
                            .w_full()
                            .gap(px(SPACE_2))
                            .child(hint(key.clone()))
                            .when(self.running(ticket), |s| {
                                s.child(
                                    row()
                                        .gap(px(SPACE_1))
                                        .child(status_dot(Tone::Info))
                                        .child(hint("Working")),
                                )
                            })
                            .child(div().flex_1())
                            .child(self.assignee_avatar(&ticket.assignee_id, AVATAR_SIZE_SM)),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_size(type_size(BODY_SIZE))
                            .line_height(relative(TITLE_LINE_HEIGHT))
                            .text_color(rgb(TEXT))
                            .line_clamp(3)
                            .text_ellipsis()
                            .child(title.clone()),
                    )
                    .when_some(footer, |s, footer| s.child(footer))
            },
            move |this: &mut Self, _, cx| this.select(id, cx),
            cx,
        )
        .on_drag(preview, move |drag, grab, _, cx| {
            dragging.set(Some(drag.id));
            let width = card_width(&geometry, drag.id).unwrap_or(drag.width);
            let drag = DraggedTicket {
                width,
                grab,
                ..drag.clone()
            };
            cx.new(|_| drag)
        })
    }

    /// Priority, labels, open blockers, sub-issues and Comments; nothing when
    /// the Ticket has none of them.
    fn card_footer(&self, ticket: &Ticket) -> Option<Div> {
        let blockers = open_blockers(&self.state.tickets, &self.state.links, ticket.id);
        let children = relations(&self.state.links, ticket.id)
            .into_iter()
            .filter(|(relation, _)| *relation == Relation::SubIssue)
            .count();
        let comments = self.comment_count(ticket.id);
        let shown_labels = ticket.labels.iter().take(CARD_LABELS);
        let hidden_labels = ticket.labels.len().saturating_sub(CARD_LABELS);
        if ticket.priority == TicketPriority::None
            && ticket.labels.is_empty()
            && blockers.is_empty()
            && children == 0
            && comments == 0
        {
            return None;
        }
        let meta = |name: &'static str, text: String, glyph: u32, color: u32| {
            row()
                .gap(px(SPACE_1))
                .text_size(type_size(CAPTION_SIZE))
                .text_color(rgb(color))
                .child(icon(name, ICON_SIZE_XS).text_color(rgb(glyph)))
                .child(text)
        };
        Some(
            row()
                .w_full()
                .flex_wrap()
                .gap(px(CHIP_GAP))
                .when(ticket.priority != TicketPriority::None, |s| {
                    s.child(priority_chip(ticket.priority))
                })
                .children(shown_labels.map(|label| tag(label.clone(), label_color(label))))
                .when(hidden_labels > 0, |s| {
                    s.child(hint(format!("+{hidden_labels}")))
                })
                .when(!blockers.is_empty(), |s| {
                    let text = blockers
                        .iter()
                        .map(|id| ticket_key(*id))
                        .collect::<Vec<_>>()
                        .join(", ");
                    // Only the glyph is red; the keys stay quiet so text leads.
                    s.child(meta("status-blocked", text, STATUS_RED, TEXT_SECONDARY))
                })
                .when(children > 0, |s| {
                    s.child(meta(
                        "list",
                        children.to_string(),
                        TEXT_TERTIARY,
                        TEXT_TERTIARY,
                    ))
                })
                .when(comments > 0, |s| {
                    s.child(meta(
                        "feedback",
                        comments.to_string(),
                        TEXT_TERTIARY,
                        TEXT_TERTIARY,
                    ))
                }),
        )
    }

    fn drop_ticket(&mut self, id: i64, status: TicketStatus, cx: &mut Context<Self>) {
        let after = match self.drag.target.take() {
            Some(target) if target.status == status => target.after,
            // No move event reached this lane: drop at its end.
            _ => in_column(&self.state.tickets, status)
                .iter()
                .map(|t| t.id)
                .filter(|other| *other != id)
                .last(),
        };
        self.drag.dragging.set(None);
        self.move_ticket(id, status, after, cx);
    }

    /// Move a Ticket on the board: locally at once, then through the daemon.
    pub(super) fn move_ticket(
        &mut self,
        id: i64,
        status: TicketStatus,
        after: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        let Some(ticket) = self.ticket(id) else {
            return;
        };
        if self.pending {
            return;
        }
        let (revision, from) = (ticket.revision, ticket.status);
        let before: Vec<i64> = in_column(&self.state.tickets, status)
            .iter()
            .map(|t| t.id)
            .collect();
        if !apply_move(&mut self.state.tickets, id, status, after) {
            cx.notify();
            return;
        }
        let now: Vec<i64> = in_column(&self.state.tickets, status)
            .iter()
            .map(|t| t.id)
            .collect();
        if from == status && before == now {
            cx.notify();
            return;
        }
        self.drag.landed = Some((id, Instant::now()));
        self.command(
            TicketCommand::Move {
                id,
                revision,
                status,
                after,
            },
            cx,
        );
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_drop_target(&self) -> Option<(TicketStatus, Option<i64>)> {
        self.drag.target.map(|t| (t.status, t.after))
    }
    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_card_width(&self, id: i64) -> Option<Pixels> {
        card_width(&self.drag.geometry, id)
    }
}

type Geometry = RefCell<HashMap<&'static str, Vec<(i64, Bounds<Pixels>)>>>;

/// A card's painted width, so its drag preview matches it.
fn card_width(geometry: &Geometry, id: i64) -> Option<Pixels> {
    geometry
        .borrow()
        .values()
        .flatten()
        .find(|(card, _)| *card == id)
        .map(|(_, bounds)| bounds.size.width)
}

/// A card's priority: the bare glyph at the height of the labels beside it.
fn priority_chip(priority: TicketPriority) -> Div {
    row().h(px(PILL_HEIGHT)).flex_shrink_0().child(
        icon(priority_icon(priority), ICON_SIZE_SM).text_color(rgb(priority_color(priority))),
    )
}

/// The white line where a dragged card will land; zero height when idle.
fn drop_line(active: bool) -> Div {
    div()
        .flex_shrink_0()
        .h(px(if active { DROP_INDICATOR_HEIGHT } else { 0. }))
        .mx(px(SPACE_1))
        .rounded_full()
        .bg(rgb(PRIMARY))
}
