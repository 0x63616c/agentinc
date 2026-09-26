//! The Dashboard: the home screen. One glance band, the lights, what's next
//! on the calendar and what the agents are doing.
use crate::{
    automations::OpenTicket,
    calendar::{CalendarModel, day_label, local, now, span, today, until},
    calendar_page::event_color,
    home::{HomeModel, glyph, tile_state},
    model::{OpenRoute, Route},
    smart_home::{climate_status, degrees},
    storage::{Store, Ticket, TicketStatus},
    ui::*,
};
use ainc_client::types::{CalendarEvent, SwitchKey};
use chrono::Timelike;
use gpui::{prelude::*, *};
use std::sync::Arc;

/// How many upcoming events the Dashboard lists before pointing at the Calendar.
const UPCOMING: usize = 4;
/// How many open Tickets it lists.
const WORK: usize = 5;

/// "Good morning" until noon, "Good afternoon" until six, "Good evening"
/// until midnight, and a plain hello in the small hours.
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        18..=23 => "Good evening",
        _ => "Hello",
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
/// A bordered card body that fills its column.
fn card_body() -> Div {
    column()
        .flex_1()
        .overflow_hidden()
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE_RAISED))
}
/// What an empty card says, inset like every card.
fn empty(title: &'static str, line: &'static str) -> Div {
    column()
        .p(px(CARD_INSET))
        .gap(px(SPACE_1))
        .child(div().font_weight(FontWeight::MEDIUM).child(title))
        .child(caption(line))
}

pub struct DashboardPage {
    home: Entity<HomeModel>,
    calendar: Entity<CalendarModel>,
    store: Option<Arc<Store>>,
    name: SharedString,
    /// A narrow window: glance numerals step down a size so they never clip.
    compact: bool,
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
            compact: false,
            hover: HoverFade::default(),
        }
    }
    pub fn set_name(&mut self, name: impl Into<SharedString>) {
        self.name = name.into();
    }
    fn numeral(&self, text: impl Into<SharedString>) -> Div {
        hero(text).when(self.compact, |s| s.text_size(type_size(COMPACT_HERO_SIZE)))
    }

    /// One third of the glance band: an eyebrow, a value on the shared
    /// baseline and a line, opening the page it summarizes.
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
                    .gap(px(SPACE_4))
                    .p(px(CARD_INSET))
                    .pt(px(CARD_INSET - EYEBROW_OPTICAL_LIFT))
                    .rounded(px(0.))
                    .when(!first, |s| s.border_l_1().border_color(rgb(BORDER)))
                    .on_hover(on_hover)
                    .bg(rgba((HOVER << 8) | (progress * 255.) as u32))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .child(icon(glyph, ICON_SIZE_SM))
                            .child(eyebrow(label)),
                    )
                    .child(
                        column()
                            .w_full()
                            .gap(px(SPACE_2))
                            // Every value sits on the same baseline, whatever its size.
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .h(type_size(if self.compact {
                                        COMPACT_HERO_SIZE
                                    } else {
                                        HERO_SIZE
                                    }))
                                    .overflow_hidden()
                                    .flex()
                                    .items_end()
                                    .child(value),
                            )
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
            (_, Some(climate)) => {
                let (status, tone) = climate_status(climate);
                (
                    self.numeral(climate.ambient.map_or_else(|| "—".into(), degrees))
                        .into_any_element(),
                    row()
                        .gap(px(SPACE_2))
                        .child(status_dot(tone))
                        .child(caption(status))
                        .into_any_element(),
                )
            }
            (Some(h), None) if h.connection.is_none() => (
                self.numeral("—")
                    .text_color(rgb(TEXT_TERTIARY))
                    .into_any_element(),
                caption("Connect your control center").into_any_element(),
            ),
            _ => (
                self.numeral("—")
                    .text_color(rgb(TEXT_TERTIARY))
                    .into_any_element(),
                caption("Waiting for the thermostat").into_any_element(),
            ),
        };
        let now = now();
        // Next up leads with its time, so all three values share one baseline.
        let (next_value, next_detail) = match next {
            Some(event) => (
                if event.all_day {
                    self.numeral("All day").into_any_element()
                } else {
                    // The meridiem is a unit beside the numeral, so the value stays narrow.
                    let time = local(event.starts_at);
                    row()
                        .items_baseline()
                        .gap(px(SPACE_1))
                        .child(self.numeral(time.format("%-I:%M").to_string()))
                        .child(
                            div()
                                .text_size(type_size(TITLE_SIZE))
                                .text_color(rgb(TEXT_SECONDARY))
                                .child(time.format("%p").to_string()),
                        )
                        .into_any_element()
                },
                caption(format!(
                    "{} · {}",
                    event.title,
                    if event.starts_at <= now {
                        "now".to_owned()
                    } else {
                        let day = local(event.starts_at).date_naive();
                        if day == today() {
                            until(event.starts_at, now)
                        } else {
                            day_label(day, today())
                        }
                    }
                ))
                .into_any_element(),
            ),
            None => (
                self.numeral("—")
                    .text_color(rgb(TEXT_TERTIARY))
                    .into_any_element(),
                caption("Nothing scheduled").into_any_element(),
            ),
        };
        let in_progress = open
            .iter()
            .filter(|t| t.status == TicketStatus::InProgress)
            .count();
        let work_detail = if open.is_empty() {
            "All clear".to_owned()
        } else {
            format!("{in_progress} in progress · {running} running")
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
                self.numeral(open.len().to_string()).into_any_element(),
                caption(work_detail).into_any_element(),
                Route::Tickets,
                false,
                cx,
            ))
    }

    /// An eyebrow with a quiet link to the page behind the section.
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
                    .tint(TEXT_SECONDARY)
                    .trailing(icon("arrowRight", ICON_SIZE_SM))
                    .build(
                        &self.hover,
                        move |_: &mut Self, _, cx| cx.emit(OpenRoute(route)),
                        cx,
                    ),
            )
    }

    fn lights(&self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.home.read(cx).snapshot.clone()?;
        let body = if home.connection.is_none() {
            row()
                .w_full()
                .justify_between()
                .gap(px(SPACE_4))
                .p(px(CARD_INSET))
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE_RAISED))
                .child(caption(
                    "Connect your control center to switch lights from here.",
                ))
                .child(
                    Button::new("dashboard.lights.connect", "Open Settings")
                        .secondary()
                        .small()
                        .build(
                            &self.hover,
                            |_: &mut Self, _, cx| cx.emit(OpenRoute(Route::Settings)),
                            cx,
                        ),
                )
        } else {
            let mut tiles = row().w_full().flex_wrap().gap(px(SPACE_3));
            for key in [
                SwitchKey::All,
                SwitchKey::Lamps,
                SwitchKey::LivingRoomLamps,
                SwitchKey::BedroomLamps,
                SwitchKey::KitchenCeiling,
                SwitchKey::UnderCabinet,
            ] {
                if !home.switches.iter().any(|s| s.key == key) {
                    continue;
                }
                // Short names: the icon already says which room.
                let label = match key {
                    SwitchKey::All => "All lights",
                    SwitchKey::Lamps => "All lamps",
                    SwitchKey::LivingRoomLamps => "Living room",
                    SwitchKey::BedroomLamps => "Bedroom",
                    SwitchKey::KitchenCeiling => "Ceiling",
                    SwitchKey::UnderCabinet => "Under cabinet",
                };
                let state = tile_state(&home, key);
                let on = state.on;
                let mut tile = SwitchTile::new(
                    SharedString::from(format!("dashboard.switch.{key}")),
                    label,
                    glyph(key),
                )
                .on(on)
                .mixed(state.mixed)
                .pending(state.pending)
                .enabled(home.reachable);
                if let Some(detail) = state.detail {
                    tile = tile.detail(detail);
                }
                tiles = tiles.child(tile.build(
                    &self.hover,
                    move |this: &mut Self, _, cx| {
                        this.home.update(cx, |h, cx| h.switch(key, !on, cx))
                    },
                    cx,
                ));
            }
            tiles
        };
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
                .child(body),
        )
    }

    fn upcoming(&self, cx: &mut Context<Self>) -> Div {
        let all: Vec<CalendarEvent> = self
            .calendar
            .read(cx)
            .upcoming(UPCOMING + 50)
            .into_iter()
            .cloned()
            .collect();
        let more = all.len().saturating_sub(UPCOMING);
        let events = &all[..all.len().min(UPCOMING)];
        let today = today();
        let body = if events.is_empty() {
            empty(
                "Nothing coming up",
                "Add an event, or show your Mac's calendars on the Calendar page.",
            )
        } else {
            let mut list = column().pb(px(CARD_INSET - SPACE_2));
            let mut last_day = None;
            for event in events {
                let date = local(event.starts_at).date_naive().max(today);
                if last_day != Some(date) {
                    list = list.child(
                        div()
                            .px(px(CARD_INSET))
                            .pt(px(if last_day.is_some() {
                                SPACE_3
                            } else {
                                CARD_INSET - EYEBROW_OPTICAL_LIFT
                            }))
                            .pb(px(SPACE_1))
                            .child(eyebrow(day_label(date, today))),
                    );
                    last_day = Some(date);
                }
                list = list.child(
                    row()
                        .px(px(CARD_INSET))
                        .py(px(SPACE_2))
                        .gap(px(SPACE_3))
                        .items_stretch()
                        .child(
                            div()
                                .w(px(EVENT_BAR_WIDTH))
                                .flex_shrink_0()
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
            if more > 0 {
                list = list.child(
                    div()
                        .px(px(CARD_INSET))
                        .pt(px(SPACE_2))
                        .child(hint(format!("{more} more on the Calendar"))),
                );
            }
            list
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
                card_body()
                    .debug_selector(|| "dashboard.upcoming".into())
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
        let body = if sorted.is_empty() {
            empty("All clear", "Open Tickets and agent runs appear here.")
        } else {
            let mut list = column();
            for (index, ticket) in sorted.iter().take(WORK).enumerate() {
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
                    .px(px(CARD_INSET)),
                );
            }
            if sorted.len() > WORK {
                list = list.child(
                    div()
                        .px(px(CARD_INSET))
                        .py(px(SPACE_3))
                        .child(hint(format!("{} more on Tickets", sorted.len() - WORK))),
                );
            }
            list
        };
        column()
            .flex_1()
            .min_w_0()
            .gap(px(SPACE_3))
            .child(self.section(
                "Open work",
                "dashboard.tickets.open",
                "Tickets",
                Route::Tickets,
                cx,
            ))
            .child(
                card_body()
                    .debug_selector(|| "dashboard.work-list".into())
                    .child(body),
            )
    }
}

impl Render for DashboardPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        self.compact = window.viewport_size().width < px(CLIMATE_STACK_BELOW);
        let now = chrono::Local::now();
        let first = self.name.split_whitespace().next().unwrap_or("").to_owned();
        let title = if first.is_empty() {
            greeting(now.hour()).to_owned()
        } else {
            format!("{}, {first}", greeting(now.hour()))
        };
        let tickets = self.store.as_ref().map(|s| s.tickets());
        let (open, running): (Vec<Ticket>, usize) = tickets.map_or((vec![], 0), |snapshot| {
            (
                snapshot
                    .tickets
                    .into_iter()
                    .filter(|t| t.status != TicketStatus::Done)
                    .collect(),
                snapshot
                    .runs
                    .iter()
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
        // Equal heights only when both sides have something to show; with no
        // open work, the glance band already says so and Upcoming takes the row.
        let both = !open.is_empty() && self.calendar.read(cx).upcoming(1).len() == 1;
        page = page.child(
            row()
                .w_full()
                .when(both, |s| s.items_stretch())
                .when(!both, |s| s.items_start())
                .gap(px(SPACE_6))
                .child(self.upcoming(cx))
                .when(!open.is_empty(), |s| s.child(self.work(&open, cx))),
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
        assert_eq!(greeting(2), "Hello");
    }
}
