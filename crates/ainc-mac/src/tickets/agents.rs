//! The Agents page: registered agents with their open Tickets and live work.
use super::*;

impl TicketsPage {
    pub fn agents(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.hover.animate(window);
        let agents: Vec<_> = self
            .state
            .assignees
            .iter()
            .filter(|a| a.kind == AssigneeKind::Agent)
            .collect();
        let add = |this: &Self, id: &'static str, cx: &mut Context<Self>| {
            Button::new(id, "New Agent")
                .primary()
                .icon("plus")
                .enabled(this.daemon.is_some() && !this.pending.busy())
                .build(&this.hover, Self::open_agent, cx)
        };
        let sync = self.sync.read(cx);
        let (loaded, error, reconnecting, loading_started) = (
            sync.loaded,
            sync.message(),
            sync.reconnecting(),
            sync.loading_started,
        );
        Page::document(
            PageHeader::new("Agents")
                .description("Agents pick up the Tickets you assign to them.")
                .actions(add(self, "agents.create", cx)),
        )
        .child(
            column()
                .gap(px(SPACE_4))
                .when_some(error.clone(), |s, error| s.child(banner(Tone::Danger, error)))
                .when(reconnecting, |s| {
                    s.child(LoadingFrame::new(loading_started, window).inline("Reconnecting…"))
                })
                .when(!loaded && error.is_none(), |s| {
                    s.child(skeleton_rows("agents.loading", 3))
                })
                .when(loaded && agents.is_empty() && error.is_none(), |s| {
                    s.child(
                        EmptyState::new("agents", "No Agents yet")
                            .description("Register an agent with instructions and a model, then assign Tickets to it.")
                            .selector("agents.empty")
                            .action(
                                Button::new("agents.create.empty", "New Agent")
                                    .secondary()
                                    .icon("plus")
                                    .enabled(self.daemon.is_some() && !self.pending.busy())
                                    .build(&self.hover, Self::open_agent, cx),
                            )
                            .build(),
                    )
                })
                .when(!agents.is_empty(), |s| {
                    let count = agents.len();
                    s.child(card().p(px(SPACE_1)).gap_0().children(agents.iter().enumerate().map(
                        |(index, agent)| {
                            let name = agent.name.clone();
                            let assigned = self
                                .state
                                .tickets
                                .iter()
                                .filter(|t| {
                                    t.assignee_id == agent.id
                                        && !matches!(t.status, TicketStatus::Done | TicketStatus::Cancelled)
                                })
                                .count();
                            let running = self.state.runs.iter().any(|r| {
                                is_active(&r.state)
                                    && self.state.tickets.iter().any(|t| {
                                        t.id == r.ticket_id && t.assignee_id == agent.id
                                    })
                            });
                            let subtitle = copy::pluralize(assigned, "open Ticket", "open Tickets");
                            list_item(SharedString::from(format!("agent.{}", agent.id)), name.clone())
                                .accessibility_id(format!("agent.{}", agent.id))
                                .min_h(px(LIST_ROW_HEIGHT))
                                .px(px(SPACE_3))
                                .py(px(SPACE_2))
                                .gap(px(SPACE_3))
                                .when(index + 1 < count, |s| {
                                    s.border_b_1().border_color(rgb(BORDER_SUBTLE))
                                })
                                .child(avatar(&name, None, AVATAR_SIZE))
                                .child(
                                    column()
                                        .flex_1()
                                        .min_w_0()
                                        .gap(px(SPACE_HALF))
                                        .child(div().text_size(type_size(BODY_SIZE)).child(name.clone()))
                                        .child(caption(subtitle)),
                                )
                                .child(if running {
                                    status_pill("Running", Tone::Info)
                                } else {
                                    status_pill("Idle", Tone::Neutral)
                                })
                        },
                    )))
                }),
        )
        .build()
        .into_any_element()
    }
}
