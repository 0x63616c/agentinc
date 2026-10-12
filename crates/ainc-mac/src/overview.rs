//! Overview: the first page. A greeting, the weather, what is on the list,
//! what is coming up and how the house is doing, each a step away.
use crate::{
    daemon::Daemon,
    page::Page,
    routes::{Destination, Route},
    smart_home::{SmartHomePage, weather_at},
    sync::{SliceChanged, Sync},
    ui::*,
};
use ainc_client::types::{CalendarEvent, CalendarSnapshot, Todo, TodoSnapshot};
use chrono::{DateTime, Local, Timelike};
use gpui::{prelude::*, *};
use std::sync::Arc;

/// How many Todos and Events the glance shows.
const GLANCE: usize = 4;

pub struct OverviewPage {
    daemon: Arc<Daemon>,
    sync: Entity<Sync>,
    home: Entity<SmartHomePage>,
    todos: Arc<TodoSnapshot>,
    calendar: Arc<CalendarSnapshot>,
    /// The local person's first name, once the profile has loaded.
    name: Option<String>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Destination> for OverviewPage {}

fn greeting(hour: u32) -> &'static str {
    match hour {
        5..12 => "Good morning",
        12..18 => "Good afternoon",
        _ => "Good evening",
    }
}

impl OverviewPage {
    pub fn new(
        daemon: Arc<Daemon>,
        sync: Entity<Sync>,
        home: Entity<SmartHomePage>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.observe(&home, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, _: &SliceChanged, cx| {
                this.reload();
                cx.notify();
            }),
        ];
        Self {
            todos: daemon.todos(),
            calendar: daemon.calendar(),
            daemon,
            sync,
            home,
            name: None,
            _subscriptions: subscriptions,
        }
    }
    pub(crate) fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }
    pub(crate) fn reload(&mut self) {
        self.todos = self.daemon.todos();
        self.calendar = self.daemon.calendar();
    }
    fn glance(title: &'static str, name: Icon, route: Route, ui: &mut Ui<Self>) -> Div {
        card().child(
            row()
                .items_center()
                .gap(px(SPACE_2))
                .child(icon(name, ICON_SIZE))
                .child(heading(title))
                .child(div().flex_1())
                .child(
                    Button::new(
                        SharedString::from(format!("overview.open.{}", route.key())),
                        "Open",
                    )
                    .icon(Icon::ArrowRight)
                    .ghost()
                    .small()
                    .build(ui, move |_, _, cx| cx.emit(Destination::Page(route))),
                ),
        )
    }
    fn todos_card(&self, ui: &mut Ui<Self>) -> Div {
        let open: Vec<&Todo> = self.todos.todos.iter().filter(|t| !t.completed).collect();
        let more = open.len().saturating_sub(GLANCE);
        Self::glance("Todos", Icon::Todos, Route::Todos, ui)
            .child(if open.is_empty() {
                caption("Nothing open. Add one on the Todos page.")
            } else {
                column()
                    .gap(px(SPACE_2))
                    .children(open.iter().take(GLANCE).map(|todo| {
                        row()
                            .items_center()
                            .gap(px(SPACE_2))
                            .child(
                                div()
                                    .size(px(ICON_SIZE_SM))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(theme().border_strong)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(type_size(BODY_SIZE))
                                    .child(todo.title.clone()),
                            )
                    }))
            })
            .when(more > 0, |s| s.child(hint(format!("and {more} more"))))
    }
    fn calendar_card(&self, now: i64, ui: &mut Ui<Self>) -> Div {
        let upcoming: Vec<&CalendarEvent> = self
            .calendar
            .events
            .iter()
            .filter(|e| e.ends_at > now)
            .collect();
        let more = upcoming.len().saturating_sub(GLANCE);
        Self::glance("Calendar", Icon::Calendar, Route::Calendar, ui)
            .child(if upcoming.is_empty() {
                caption("Nothing scheduled.")
            } else {
                column()
                    .gap(px(SPACE_2))
                    .children(upcoming.iter().take(GLANCE).map(|event| {
                        let day = time::day(event.starts_at, now);
                        row()
                            .items_center()
                            .gap(px(SPACE_3))
                            .child(
                                column()
                                    .min_w(px(LIST_UPDATED_WIDTH + SPACE_6))
                                    .flex_shrink_0()
                                    .whitespace_nowrap()
                                    .child(div().text_size(type_size(LABEL_SIZE)).child(day))
                                    .child(hint(time::span(event.starts_at, event.ends_at))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(type_size(BODY_SIZE))
                                    .child(event.title.clone()),
                            )
                    }))
            })
            .when(more > 0, |s| s.child(hint(format!("and {more} more"))))
    }
    fn home_card(&self, ui: &mut Ui<Self>) -> Div {
        let home = self.home.read(ui.cx);
        let lights = copy::pluralize(home.lights_on(), "light on", "lights on");
        let climate = home
            .climate()
            .map_or("Climate off".to_owned(), |t| format!("Climate {t}°"));
        let playing = home.playing().map_or("Nothing playing".to_owned(), |s| {
            format!("{} in the {}", s.track, s.room.to_lowercase())
        });
        let alarm = home.next_alarm().map_or("No alarm set".to_owned(), |a| {
            format!("{} alarm at {}", a.label, a.time)
        });
        let stat = |name: Icon, text: String| {
            row()
                .items_center()
                .gap(px(SPACE_2))
                .child(icon(name, ICON_SIZE_SM))
                .child(div().text_size(type_size(BODY_SIZE)).child(text))
        };
        Self::glance("Smart Home", Icon::SmartHome, Route::SmartHome, ui).child(
            column()
                .gap(px(SPACE_2))
                .child(stat(Icon::Bulb, lights))
                .child(stat(Icon::Thermometer, climate))
                .child(stat(Icon::Music, playing))
                .child(stat(Icon::Bell, alarm)),
        )
    }
}

impl Page for OverviewPage {
    const ROUTE: Route = Route::Overview;
}

impl Render for OverviewPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.sync.read(cx).load_state();
        let today = Local::now();
        let now = time::now();
        let weather = weather_at(today.hour());
        let open = self.todos.todos.iter().filter(|t| !t.completed).count();
        let events_today = self
            .calendar
            .events
            .iter()
            .filter(|e| {
                e.ends_at > now
                    && DateTime::from_timestamp(e.starts_at, 0)
                        .is_some_and(|t| t.with_timezone(&Local).date_naive() == today.date_naive())
            })
            .count();
        let count = |n: usize, one: &str, many: &str| match n {
            0 => format!("no {many}"),
            _ => copy::pluralize(n, one, many),
        };
        let summary = format!(
            "{}. {} and {} today.",
            time::long_date(now),
            count(open, "open Todo", "open Todos"),
            count(events_today, "Event", "Events")
        );
        let ui = &mut Ui::new(window, cx);
        PageFrame::document(
            PageHeader::new(match &self.name {
                Some(name) => format!("{}, {name}", greeting(today.hour())),
                None => greeting(today.hour()).to_owned(),
            })
            .description(summary)
            .actions(
                row()
                    .items_center()
                    .gap(px(SPACE_2))
                    .child(icon(weather.icon, ICON_SIZE_LG).text_color(rgb(theme().text)))
                    .child(
                        div()
                            .text_size(type_size(HEADING_SIZE))
                            .child(format!("{}°", weather.temperature)),
                    )
                    .child(caption(weather.summary)),
            ),
        )
        .child(column().gap(px(SPACE_4)).children(page_frame(
            "overview",
            &state,
            SKELETON_ROWS,
            ui,
            |ui| {
                column()
                    .gap(px(SPACE_4))
                    .child(
                        row()
                            .items_start()
                            .gap(px(SPACE_4))
                            .child(div().flex_1().min_w_0().child(self.todos_card(ui)))
                            .child(div().flex_1().min_w_0().child(self.calendar_card(now, ui))),
                    )
                    .child(self.home_card(ui))
                    .into_any_element()
            },
        )))
        .build()
    }
}

#[cfg(test)]
mod tests {
    use super::greeting;

    #[test]
    fn the_greeting_follows_the_hour() {
        assert_eq!(greeting(7), "Good morning");
        assert_eq!(greeting(13), "Good afternoon");
        assert_eq!(greeting(22), "Good evening");
    }
}
