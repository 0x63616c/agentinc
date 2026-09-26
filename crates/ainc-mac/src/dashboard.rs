//! The Dashboard: the home screen. One glance band, the lights, what's next
//! on the calendar and what the agents are doing.
use crate::{
    automations::OpenTicket,
    calendar::{CalendarModel, day_label, local, now, span, today, until},
    calendar_page::event_color,
    home::{HomeModel, glyph},
    model::{OpenRoute, Route},
    smart_home::{climate_status, degrees},
    storage::{Store, Ticket, TicketStatus},
    ui::*,
};
use ainc_client::types::{CalendarEvent, SwitchKey};
use chrono::Timelike;
use gpui::{prelude::*, *};
use std::sync::Arc;

/// "Good morning" until noon, "Good afternoon" until six, then "Good evening".
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}
fn status_label(status: TicketStatus) -> (&'static str, Tone) {
    match status {
        TicketStatus::InProgress => ("In progress", Tone::Info),
        TicketStatus::ToDo => ("To do", Tone::Neutral),
        TicketStatus::Backlog => ("Backlog", Tone::Neutral),
        TicketStatus::Done => ("Done", Tone::Success),
    }
}

pub struct DashboardPage {
    home: Entity<HomeModel>,
    calendar: Entity<CalendarModel>,
    store: Option<Arc<Store>>,
    name: SharedString,
    hover: HoverFade,
    _observe: Vec<Subscription>,
}
impl EventEmitter<OpenRoute> for DashboardPage {}
impl EventEmitter<OpenTicket> for DashboardPage {}
impl HoverHost for DashboardPage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}

impl DashboardPage {
    pub fn new(
        home: Entity<HomeModel>,
        calendar: Entity<CalendarModel>,
        store: Option<Arc<Store>>,
        name: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            _observe: vec![
                cx.observe(&home, |_, _, cx| cx.notify()),
                cx.observe(&calendar, |_, _, cx| cx.notify()),
            ],
            home,
            calendar,
            store,
            name: name.into(),
            hover: HoverFade::default(),
        }
    }
    pub fn set_name(&mut self, name: impl Into<SharedString>) {
        self.name = name.into();
    }

    /// One third of the glance band: an eyebrow, a large value and a line,
    /// opening the page it summarizes.
    #[allow(clippy::too_many_arguments)]
    fn glance(
        &self,
        id: &'static str,
        label: &'static str,
        glyph: &'static str,
        value: AnyElement,
        detail: AnyElement,
        route: Route,
        first: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let element: ElementId = id.into();
        let (progress, on_hover) = self.hover.track(&element, true, cx);
        action_button(
            ButtonSpec {
                id: element,
                label: format!("Open {}", route.label()).into(),
                enabled: true,
            },
            |button| {
                button
                    .flex_1()
                    .min_w_0()
                    .flex_col()
                    .items_start()
                    .justify_between()
                    .gap(px(SPACE_4))
                    .p(px(SPACE_6))
                    .rounded(px(0.))
                    .when(!first, |s| s.border_l_1().border_color(rgb(BORDER)))
                    .on_hover(on_hover)
                    .bg(rgba((HOVER << 8) | (progress * 255.) as u32))
                    .child(
                        row()
                            .w_full()
                            .justify_between()
                            .child(
                                row()
                                    .gap(px(SPACE_2))
                                    .child(icon(glyph, ICON_SIZE_SM))
                                    .child(eyebrow(label)),
                            )
                            .child(
                                icon("arrowUpRight", ICON_SIZE_SM).opacity(0.4 + 0.6 * progress),
                            ),
                    )
                    .child(
                        column()
                            .w_full()
                            .gap(px(SPACE_2))
                            .child(value)
                            .child(detail),
                    )
            },
            move |_: &mut Self, _, cx| cx.emit(OpenRoute(route)),
            cx,
        )
    }

    fn band(
        &self,
        next: Option<&CalendarEvent>,
        open: &[Ticket],
        running: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let home = self.home.read(cx).snapshot.clone();
        let climate = home.as_ref().and_then(|h| h.climate.clone());
        let (inside, inside_detail) = match (&home, &climate) {
            (_, Some(climate)) => (
                hero(climate.ambient.map_or_else(|| "—".into(), degrees)).into_any_element(),
                {
                    let (status, tone) = climate_status(climate);
                    row()
                        .gap(px(SPACE_2))
                        .child(status_dot(tone))
                        .child(caption(status))
                        .into_any_element()
                },
            ),
            (Some(h), None) if h.connection.is_none() => (
                hero("—").text_color(rgb(TEXT_TERTIARY)).into_any_element(),
                caption("Connect the control center in Settings").into_any_element(),
            ),
            _ => (
                hero("—").text_color(rgb(TEXT_TERTIARY)).into_any_element(),
                caption("Waiting for the thermostat").into_any_element(),
            ),
        };
        let now = now();
        let (next_value, next_detail) = match next {
            Some(event) => (
                column()
                    .gap(px(SPACE_1))
                    .child(
                        div()
                            .text_size(type_size(TITLE_SIZE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(relative(TITLE_LINE_HEIGHT))
                            .truncate()
                            .child(event.title.clone()),
                    )
                    .child(caption(format!(
                        "{} · {}",
                        day_label(local(event.starts_at).date_naive(), today()),
                        span(event)
                    )))
                    .into_any_element(),
                row()
                    .gap(px(SPACE_2))
                    .child(
                        div()
                            .size(px(SPACE_2))
                            .rounded_full()
                            .bg(event_color(event)),
                    )
                    .child(caption(if event.starts_at <= now {
                        "Happening now".to_owned()
                    } else {
                        until(event.starts_at, now)
                    }))
                    .into_any_element(),
            ),
            None => (
                div()
                    .text_size(type_size(TITLE_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(TEXT_SECONDARY))
                    .child("Nothing scheduled")
                    .into_any_element(),
                caption("Your calendar is clear").into_any_element(),
            ),
        };
        let in_progress = open
            .iter()
            .filter(|t| t.status == TicketStatus::InProgress)
            .count();
        let work_detail = match (in_progress, running) {
            (0, 0) => "Nothing in progress".to_owned(),
            (p, 0) => format!("{p} in progress"),
            (p, r) => format!(
                "{p} in progress · {r} agent {}",
                if r == 1 { "run" } else { "runs" }
            ),
        };
        row()
            .debug_selector(|| "dashboard.band".into())
            .w_full()
            .items_stretch()
            .overflow_hidden()
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .child(self.glance(
                "dashboard.inside",
                "Inside",
                "thermometer",
                inside,
                inside_detail,
                Route::SmartHome,
                true,
                cx,
            ))
            .child(self.glance(
                "dashboard.next",
                "Next up",
                "calendar",
                next_value,
                next_detail,
                Route::Calendar,
                false,
                cx,
            ))
            .child(self.glance(
                "dashboard.work",
                "Open work",
                "tasks",
                hero(open.len().to_string()).into_any_element(),
                caption(work_detail).into_any_element(),
                Route::Tickets,
                false,
                cx,
            ))
    }

    fn lights(&self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.home.read(cx).snapshot.clone()?;
        home.connection.as_ref()?;
        let mut tiles = row().w_full().flex_wrap().gap(px(SPACE_3));
        for key in [
            SwitchKey::All,
            SwitchKey::Lamps,
            SwitchKey::LivingRoomLamps,
            SwitchKey::BedroomLamps,
            SwitchKey::KitchenCeiling,
            SwitchKey::UnderCabinet,
        ] {
            let Some(state) = home.switches.iter().find(|s| s.key == key) else {
                continue;
            };
            let on = state.on;
            tiles = tiles.child(switch_tile(
                SharedString::from(format!("dashboard.switch.{key}")),
                state.label.clone(),
                glyph(key),
                on,
                state.pending,
                home.reachable,
                &self.hover,
                move |this: &mut Self, _, cx| this.home.update(cx, |h, cx| h.switch(key, !on, cx)),
                cx,
            ));
        }
        Some(
            column()
                .debug_selector(|| "dashboard.lights".into())
                .gap(px(SPACE_3))
                .child(self.section(
                    "Lights",
                    "dashboard.lights.open",
                    "Smart Home",
                    Route::SmartHome,
                    cx,
                ))
                .child(tiles),
        )
    }

    fn section(
        &self,
        label: &'static str,
        id: &'static str,
        link: &'static str,
        route: Route,
        cx: &mut Context<Self>,
    ) -> Div {
        row()
            .w_full()
            .h(px(CONTROL_HEIGHT_SM))
            .justify_between()
            .child(eyebrow(label))
            .child(
                Button::new(id, link)
                    .ghost()
                    .small()
                    .trailing(icon("arrowRight", ICON_SIZE_SM))
                    .build(
                        &self.hover,
                        move |_: &mut Self, _, cx| cx.emit(OpenRoute(route)),
                        cx,
                    ),
            )
    }

    fn upcoming(&self, cx: &mut Context<Self>) -> Div {
        let events: Vec<CalendarEvent> = self
            .calendar
            .read(cx)
            .upcoming(6)
            .into_iter()
            .cloned()
            .collect();
        let today = today();
        let mut list = column();
        let mut last_day = None;
        for event in &events {
            let date = local(event.starts_at).date_naive().max(today);
            if last_day != Some(date) {
                list = list.child(
                    div()
                        .px(px(SPACE_4))
                        .pt(px(SPACE_4))
                        .pb(px(SPACE_1))
                        .child(eyebrow(day_label(date, today))),
                );
                last_day = Some(date);
            }
            list = list.child(
                row()
                    .min_h(px(LIST_ROW_HEIGHT))
                    .px(px(SPACE_4))
                    .gap(px(SPACE_3))
                    .child(
                        div()
                            .w(px(EVENT_BAR_WIDTH))
                            .h(px(SPACE_5))
                            .rounded_full()
                            .bg(event_color(event)),
                    )
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .gap(px(SPACE_HALF))
                            .child(div().truncate().child(event.title.clone()))
                            .child(caption(match &event.location {
                                Some(location) => format!("{} · {location}", span(event)),
                                None => span(event),
                            })),
                    ),
            );
        }
        let body = if events.is_empty() {
            column()
                .p(px(SPACE_6))
                .gap(px(SPACE_1))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Nothing coming up"),
                )
                .child(caption(
                    "Add an event, or show your Mac's calendars on the Calendar page.",
                ))
        } else {
            list.pb(px(SPACE_2))
        };
        column()
            .flex_1()
            .min_w_0()
            .gap(px(SPACE_3))
            .child(self.section(
                "Upcoming",
                "dashboard.calendar.open",
                "Calendar",
                Route::Calendar,
                cx,
            ))
            .child(
                column()
                    .debug_selector(|| "dashboard.upcoming".into())
                    .rounded(px(RADIUS_LG))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE_RAISED))
                    .child(body),
            )
    }

    fn work(&self, open: &[Ticket], cx: &mut Context<Self>) -> Div {
        let (runs, assignees) = self
            .store
            .as_ref()
            .map(|s| {
                let snapshot = s.tickets();
                (snapshot.runs, snapshot.assignees)
            })
            .unwrap_or_default();
        let mut sorted: Vec<&Ticket> = open.iter().collect();
        sorted.sort_by_key(|t| {
            (
                t.status != TicketStatus::InProgress,
                t.status != TicketStatus::ToDo,
                -t.id,
            )
        });
        let mut list = column();
        for (index, ticket) in sorted.iter().take(6).enumerate() {
            let (label, tone) = status_label(ticket.status);
            let assignee = assignees
                .iter()
                .find(|a| a.id == ticket.assignee_id)
                .map_or("Unassigned".to_owned(), |a| a.name.clone());
            let running = runs.iter().any(|r| {
                r.ticket_id == ticket.id && matches!(r.state.as_str(), "queued" | "running")
            });
            let id = ticket.id;
            list = list.when(index > 0, |s| s.child(divider())).child(
                ListRow::new(
                    SharedString::from(format!("dashboard.ticket.{id}")),
                    ticket.title.clone(),
                )
                .subtitle(if running {
                    format!("{assignee} · working now")
                } else {
                    assignee
                })
                .trailing(status_pill(label, tone))
                .build(
                    &self.hover,
                    move |_: &mut Self, _, cx| cx.emit(OpenTicket(id)),
                    cx,
                )
                .rounded(px(0.))
                .px(px(SPACE_4)),
            );
        }
        let body = if sorted.is_empty() {
            column()
                .p(px(SPACE_6))
                .gap(px(SPACE_1))
                .child(div().font_weight(FontWeight::MEDIUM).child("All clear"))
                .child(caption("Open Tickets and agent runs appear here."))
        } else {
            list
        };
        column()
            .flex_1()
            .min_w_0()
            .gap(px(SPACE_3))
            .child(self.section(
                "Agent work",
                "dashboard.tickets.open",
                "Tickets",
                Route::Tickets,
                cx,
            ))
            .child(
                column()
                    .debug_selector(|| "dashboard.work-list".into())
                    .overflow_hidden()
                    .rounded(px(RADIUS_LG))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE_RAISED))
                    .child(body),
            )
    }
}

impl Render for DashboardPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let now = chrono::Local::now();
        let first = self.name.split_whitespace().next().unwrap_or("").to_owned();
        let title = if first.is_empty() {
            greeting(now.hour()).to_owned()
        } else {
            format!("{}, {first}", greeting(now.hour()))
        };
        let tickets = self
            .store
            .as_ref()
            .map(|s| s.tickets())
            .map(|s| (s.tickets, s.runs));
        let (open, running): (Vec<Ticket>, usize) =
            tickets.map_or((vec![], 0), |(tickets, runs)| {
                (
                    tickets
                        .into_iter()
                        .filter(|t| t.status != TicketStatus::Done)
                        .collect(),
                    runs.iter()
                        .filter(|r| matches!(r.state.as_str(), "queued" | "running"))
                        .count(),
                )
            });
        let next = self
            .calendar
            .read(cx)
            .upcoming(1)
            .first()
            .map(|e| (*e).clone());
        let mut page = Page::document(
            PageHeader::new(title).description(now.format("%A, %B %-d").to_string()),
        )
        .child(self.band(next.as_ref(), &open, running, cx));
        if let Some(lights) = self.lights(cx) {
            page = page.child(lights);
        }
        page = page.child(
            row()
                .w_full()
                .items_start()
                .gap(px(SPACE_6))
                .child(self.upcoming(cx))
                .child(self.work(&open, cx)),
        );
        div()
            .id("dashboard.page")
            .debug_selector(|| "dashboard.page".into())
            .size_full()
            .child(page.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;

    #[test]
    fn greetings_follow_the_clock() {
        assert_eq!(greeting(7), "Good morning");
        assert_eq!(greeting(13), "Good afternoon");
        assert_eq!(greeting(21), "Good evening");
        assert_eq!(greeting(2), "Good evening");
    }
}
