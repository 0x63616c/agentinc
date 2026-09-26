//! Calendar state shared by the Dashboard and the Calendar page, the Mac
//! calendar import, and the date arithmetic both pages draw with.
use crate::{
    calendar_store::{Access, CalendarSource},
    storage::{self, api_error},
};
use ainc_client::types::{ActionView, CalendarCommand, CalendarEvent, CalendarImportRequest};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Weekday};
use gpui::{prelude::*, *};
use std::{sync::Arc, time::Instant};

const DAY: i64 = 86_400;
/// How far back and ahead the Mac's calendars are mirrored.
const IMPORT_BEHIND: i64 = 31 * DAY;
const IMPORT_AHEAD: i64 = 365 * DAY;
/// How often an authorized Mac calendar is mirrored again while AgentInc runs.
const SYNC_EVERY: std::time::Duration = std::time::Duration::from_secs(15 * 60);

pub struct CalendarModel {
    pub events: Vec<CalendarEvent>,
    pub last_import: Option<ActionView>,
    pub access: Access,
    pub error: Option<String>,
    pub loaded: bool,
    /// The Unix range `events` covers.
    window: (i64, i64),
    source: Arc<dyn CalendarSource>,
    refreshing: bool,
    syncing: bool,
    last_sync: Option<Instant>,
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
/// Local midnight at the start of `date`, in Unix seconds.
pub fn day_start(date: NaiveDate) -> i64 {
    Local
        .from_local_datetime(&date.and_time(NaiveTime::MIN))
        .earliest()
        .map_or(0, |t| t.timestamp())
}
pub fn local(seconds: i64) -> DateTime<Local> {
    DateTime::from_timestamp(seconds, 0)
        .unwrap_or_default()
        .with_timezone(&Local)
}
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

impl CalendarModel {
    pub fn new(source: Arc<dyn CalendarSource>, cx: &mut Context<Self>) -> Self {
        let access = source.access();
        let mut this = Self {
            events: vec![],
            last_import: None,
            access,
            error: None,
            loaded: false,
            window: Self::default_window(),
            source,
            refreshing: false,
            syncing: false,
            last_sync: None,
        };
        #[cfg(not(test))]
        {
            this.refresh(cx);
            cx.spawn(async move |this, cx| {
                loop {
                    let alive = this.update(cx, |this, cx| {
                        let importing = this
                            .last_import
                            .as_ref()
                            .is_some_and(|i| matches!(i.state.as_str(), "queued" | "running"));
                        if importing {
                            this.refresh(cx);
                        }
                        if this.access == Access::Granted
                            && this.last_sync.is_none_or(|t| t.elapsed() >= SYNC_EVERY)
                        {
                            this.sync(cx);
                        }
                    });
                    if alive.is_err() {
                        break;
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                }
            })
            .detach();
        }
        let _ = (&mut this, cx);
        this
    }
    fn default_window() -> (i64, i64) {
        let today = day_start(today());
        (today - 62 * DAY, today + 200 * DAY)
    }
    /// Make sure `events` covers `[from, to)`, loading a wider range if not.
    pub fn cover(&mut self, from: i64, to: i64, cx: &mut Context<Self>) {
        if from >= self.window.0 && to <= self.window.1 {
            return;
        }
        self.window = (
            from - 31 * DAY,
            (to + 120 * DAY).min(from - 31 * DAY + 399 * DAY),
        );
        self.refresh(cx);
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let (from, to) = self.window;
        let work = cx.background_executor().spawn(async move {
            storage::background(async move {
                let client = storage::client().await?;
                let snapshot = client
                    .calendar_state()
                    .from(from)
                    .to(to)
                    .send()
                    .await
                    .map_err(api_error)?;
                anyhow::Ok(snapshot.into_inner())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.refreshing = false;
                this.loaded = true;
                match result {
                    Ok(snapshot) => {
                        this.events = snapshot.events;
                        this.last_import = snapshot.last_import;
                        this.error = None;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    /// Ask macOS for calendar access, then mirror the calendars once granted.
    pub fn request_access(&mut self, cx: &mut Context<Self>) {
        let source = self.source.clone();
        let work = cx
            .background_executor()
            .spawn(async move { source.request() });
        cx.spawn(async move |this, cx| {
            let access = work.await;
            let _ = this.update(cx, |this, cx| {
                this.access = access;
                if access == Access::Granted {
                    this.sync(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    /// Read the Mac's calendars and hand them to the daemon as one import.
    pub fn sync(&mut self, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        self.access = self.source.access();
        if self.access != Access::Granted {
            cx.notify();
            return;
        }
        self.syncing = true;
        self.last_sync = Some(Instant::now());
        cx.notify();
        let source = self.source.clone();
        let window_start = day_start(today()) - IMPORT_BEHIND;
        let window_end = day_start(today()) + IMPORT_AHEAD;
        let work = cx.background_executor().spawn(async move {
            let events = source.events(window_start, window_end)?;
            storage::background(async move {
                let client = storage::client().await?;
                client
                    .calendar_import()
                    .body(CalendarImportRequest {
                        window_start,
                        window_end,
                        events,
                    })
                    .send()
                    .await
                    .map_err(api_error)?;
                anyhow::Ok(())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.syncing = false;
                if let Err(error) = result {
                    this.error = Some(format!("Calendar sync failed: {error}"));
                }
                this.refresh(cx);
            });
        })
        .detach();
    }
    pub fn syncing(&self) -> bool {
        self.syncing
            || self
                .last_import
                .as_ref()
                .is_some_and(|i| matches!(i.state.as_str(), "queued" | "running"))
    }
    /// Create, edit or delete an AgentInc event.
    pub fn command(
        &mut self,
        command: CalendarCommand,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<()>> {
        let work = cx.background_executor().spawn(async move {
            storage::background(async move {
                let client = storage::client().await?;
                client
                    .calendar_command()
                    .body(ainc_client::types::CalendarRequest {
                        operation_id: uuid::Uuid::new_v4().to_string(),
                        command,
                    })
                    .send()
                    .await
                    .map_err(api_error)?;
                anyhow::Ok(())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| this.refresh(cx));
            result
        })
    }
    /// The next events that have not ended, soonest first.
    pub fn upcoming(&self, limit: usize) -> Vec<&CalendarEvent> {
        let now = now();
        self.events
            .iter()
            .filter(|e| e.ends_at > now)
            .take(limit)
            .collect()
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn fixture(
        &mut self,
        events: Vec<CalendarEvent>,
        access: Access,
        last_import: Option<ActionView>,
        cx: &mut Context<Self>,
    ) {
        self.events = events;
        self.access = access;
        self.last_import = last_import;
        self.loaded = true;
        self.error = None;
        cx.notify();
    }
}

/// Events overlapping one local day, all-day events first.
pub fn on_day(events: &[CalendarEvent], date: NaiveDate) -> Vec<&CalendarEvent> {
    let start = day_start(date);
    let end = day_start(date + Duration::days(1));
    let mut day: Vec<_> = events
        .iter()
        .filter(|e| {
            e.starts_at < end
                && (e.ends_at > start || (e.ends_at == e.starts_at && e.starts_at >= start))
        })
        .collect();
    day.sort_by_key(|e| (!e.all_day, e.starts_at));
    day
}

/// Six Monday-first weeks that cover `month`.
pub fn month_grid(month: NaiveDate) -> Vec<NaiveDate> {
    let first = month.with_day(1).unwrap_or(month);
    let lead = first.weekday().num_days_from_monday() as i64;
    (0..42).map(|i| first + Duration::days(i - lead)).collect()
}
/// The Monday-first week containing `date`.
pub fn week(date: NaiveDate) -> Vec<NaiveDate> {
    let monday = date - Duration::days(date.weekday().num_days_from_monday() as i64);
    (0..7).map(|i| monday + Duration::days(i)).collect()
}
pub const WEEKDAYS: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

/// Side-by-side lanes for one day's timed events: each event's lane and the
/// number of lanes in its overlapping cluster.
pub fn lanes(events: &[&CalendarEvent]) -> Vec<(usize, usize)> {
    let mut placed: Vec<(usize, usize)> = vec![(0, 1); events.len()];
    let mut order: Vec<usize> = (0..events.len()).collect();
    order.sort_by_key(|&i| (events[i].starts_at, -events[i].ends_at));
    let mut cluster: Vec<usize> = vec![];
    let mut lane_ends: Vec<i64> = vec![];
    let mut cluster_end = i64::MIN;
    let close = |cluster: &mut Vec<usize>, lanes: usize, placed: &mut Vec<(usize, usize)>| {
        for &i in cluster.iter() {
            placed[i].1 = lanes;
        }
        cluster.clear();
    };
    for i in order {
        let event = events[i];
        let end = event.ends_at.max(event.starts_at + 15 * 60);
        if event.starts_at >= cluster_end && !cluster.is_empty() {
            close(&mut cluster, lane_ends.len(), &mut placed);
            lane_ends.clear();
        }
        let lane = lane_ends
            .iter()
            .position(|&e| e <= event.starts_at)
            .unwrap_or_else(|| {
                lane_ends.push(i64::MIN);
                lane_ends.len() - 1
            });
        lane_ends[lane] = end;
        placed[i].0 = lane;
        cluster.push(i);
        cluster_end = cluster_end.max(end);
    }
    let count = lane_ends.len();
    close(&mut cluster, count, &mut placed);
    placed
}

/// "3 PM", "3:30 PM".
pub fn clock(seconds: i64) -> String {
    let time = local(seconds);
    if time.minute() == 0 {
        time.format("%-I %p").to_string()
    } else {
        time.format("%-I:%M %p").to_string()
    }
}
/// "All day", "3 – 4 PM", "11:30 AM – 12:15 PM".
pub fn span(event: &CalendarEvent) -> String {
    if event.all_day {
        let days = ((event.ends_at - event.starts_at) + DAY / 2) / DAY;
        return if days > 1 {
            format!("All day · {days} days")
        } else {
            "All day".into()
        };
    }
    let (from, to) = (local(event.starts_at), local(event.ends_at));
    if event.ends_at == event.starts_at {
        return clock(event.starts_at);
    }
    if from.format("%p").to_string() == to.format("%p").to_string()
        && from.date_naive() == to.date_naive()
    {
        let bare = |t: DateTime<Local>| {
            if t.minute() == 0 {
                t.format("%-I").to_string()
            } else {
                t.format("%-I:%M").to_string()
            }
        };
        format!("{} – {}", bare(from), clock(event.ends_at))
    } else {
        format!("{} – {}", clock(event.starts_at), clock(event.ends_at))
    }
}
/// "Today", "Tomorrow", "Yesterday", or "Mon, Sep 28".
pub fn day_label(date: NaiveDate, today: NaiveDate) -> String {
    match (date - today).num_days() {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        _ if date.year() == today.year() => date.format("%a, %b %-d").to_string(),
        _ => date.format("%a, %b %-d, %Y").to_string(),
    }
}
/// "now", "in 25 min", "in 3 h", "in 2 days".
pub fn until(seconds: i64, now: i64) -> String {
    let minutes = (seconds - now) / 60;
    match minutes {
        m if m <= 0 => "now".into(),
        m if m < 60 => format!("in {m} min"),
        m if m < 24 * 60 => format!("in {} h", (m + 30) / 60),
        m => {
            let days = (m + 12 * 60) / (24 * 60);
            format!("in {days} {}", if days == 1 { "day" } else { "days" })
        }
    }
}
/// "2 min ago" and friends for sync status.
pub fn ago(seconds: i64, now: i64) -> String {
    let minutes = (now - seconds).max(0) / 60;
    match minutes {
        0 => "just now".into(),
        m if m < 60 => format!("{m} min ago"),
        m if m < 24 * 60 => format!("{} h ago", m / 60),
        m => format!("{} days ago", m / (24 * 60)),
    }
}

/// Dates typed into the event form: `2026-09-26`, `9/26/2026`, `9/26`,
/// `Sep 26` or `Sep 26 2026`.
pub fn parse_date(text: &str, today: NaiveDate) -> Option<NaiveDate> {
    let text = text.trim().replace(',', "");
    for format in ["%Y-%m-%d", "%m/%d/%Y", "%b %d %Y", "%B %d %Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(&text, format) {
            return Some(date);
        }
    }
    for format in ["%m/%d", "%b %d", "%B %d"] {
        if let Ok(date) =
            NaiveDate::parse_from_str(&format!("{text} {}", today.year()), &format!("{format} %Y"))
        {
            return Some(date);
        }
    }
    None
}
/// Times typed into the event form: `15:00`, `3:00 PM`, `3pm`, `3 pm`.
pub fn parse_time(text: &str) -> Option<NaiveTime> {
    let mut text = text.trim().to_ascii_uppercase().replace(' ', "");
    // chrono needs minutes: "3PM" reads as "3:00PM".
    if !text.contains(':') && (text.ends_with("AM") || text.ends_with("PM")) {
        text.insert_str(text.len() - 2, ":00");
    }
    for format in ["%H:%M", "%I:%M%p"] {
        if let Ok(time) = NaiveTime::parse_from_str(&text, format) {
            return Some(time);
        }
    }
    None
}
/// The form's text for a date and a time.
pub fn date_text(date: NaiveDate) -> String {
    date.format("%b %-d, %Y").to_string()
}
pub fn time_text(seconds: i64) -> String {
    local(seconds).format("%-I:%M %p").to_string()
}
/// Unix seconds for a local date and time.
pub fn at(date: NaiveDate, time: NaiveTime) -> Option<i64> {
    Local
        .from_local_datetime(&date.and_time(time))
        .earliest()
        .map(|t| t.timestamp())
}

use chrono::Timelike;

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use ainc_client::types::EventSource;

    pub fn event(
        id: &str,
        title: &str,
        date: NaiveDate,
        start: (u32, u32),
        minutes: i64,
    ) -> CalendarEvent {
        let starts_at = at(date, NaiveTime::from_hms_opt(start.0, start.1, 0).unwrap()).unwrap();
        CalendarEvent {
            id: id.into(),
            source: EventSource::Macos,
            calendar: "Home".into(),
            color: None,
            title: title.into(),
            location: None,
            notes: None,
            starts_at,
            ends_at: starts_at + minutes * 60,
            all_day: false,
            revision: 0,
        }
    }
    /// A realistic fortnight around today.
    pub fn fortnight() -> Vec<CalendarEvent> {
        let today = today();
        let d = |n: i64| today + Duration::days(n);
        let tint = |mut e: CalendarEvent, color: u32| {
            e.color = Some(format!("#{color:06X}"));
            e
        };
        use crate::ui::{STATUS_AMBER, STATUS_BLUE, STATUS_GREEN, STATUS_PURPLE};
        let mut events = vec![
            tint(
                event("standup", "Team standup", d(0), (9, 30), 15),
                STATUS_BLUE,
            ),
            tint(event("dentist", "Dentist", d(0), (15, 0), 60), STATUS_AMBER),
            {
                let mut e = tint(
                    event("dinner", "Dinner with Sam", d(0), (19, 30), 120),
                    STATUS_PURPLE,
                );
                e.location = Some("Bestia".into());
                e
            },
            tint(event("gym", "Gym", d(1), (7, 0), 60), STATUS_GREEN),
            tint(
                event("review", "Design review", d(1), (11, 0), 90),
                STATUS_BLUE,
            ),
            tint(
                event("sync", "Roadmap sync", d(1), (11, 30), 30),
                STATUS_BLUE,
            ),
            tint(
                event("flight", "Flight to SFO", d(3), (8, 15), 95),
                STATUS_AMBER,
            ),
            tint(event("offsite", "Offsite", d(4), (10, 0), 360), STATUS_BLUE),
            tint(event("brunch", "Brunch", d(6), (11, 0), 90), STATUS_PURPLE),
            tint(
                event("call", "Call with Mum", d(8), (18, 0), 45),
                STATUS_GREEN,
            ),
        ];
        let mut holiday = event("holiday", "Long weekend", d(5), (0, 0), 0);
        holiday.all_day = true;
        holiday.starts_at = day_start(d(5));
        holiday.ends_at = day_start(d(7));
        events.push(holiday);
        let mut own = event("own", "Plan the week", d(2), (9, 0), 60);
        own.source = EventSource::Agentinc;
        own.calendar = "AgentInc".into();
        events.push(own);
        events.sort_by_key(|e| e.starts_at);
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The GPUI glob exports its own `test` attribute; these are plain tests.
    use ::core::prelude::v1::test;

    #[test]
    fn month_grids_start_on_monday_and_cover_the_month() {
        let september = NaiveDate::from_ymd_opt(2026, 9, 26).unwrap();
        let grid = month_grid(september);
        assert_eq!(grid.len(), 42);
        assert_eq!(grid[0], NaiveDate::from_ymd_opt(2026, 8, 31).unwrap());
        assert_eq!(grid[1], NaiveDate::from_ymd_opt(2026, 9, 1).unwrap());
        assert_eq!(
            week(september)[0],
            NaiveDate::from_ymd_opt(2026, 9, 21).unwrap()
        );
    }

    #[test]
    fn overlapping_events_share_lanes_and_later_ones_reuse_them() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 27).unwrap();
        let review = fixtures::event("review", "Design review", date, (11, 0), 90);
        let sync = fixtures::event("sync", "Roadmap sync", date, (11, 30), 30);
        let lunch = fixtures::event("lunch", "Lunch", date, (12, 0), 60);
        let evening = fixtures::event("evening", "Evening", date, (18, 0), 60);
        let day = [&review, &sync, &lunch, &evening];
        assert_eq!(lanes(&day), [(0, 2), (1, 2), (1, 2), (0, 1)]);
    }

    #[test]
    fn forms_accept_the_ways_people_type_dates_and_times() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 26).unwrap();
        let expected = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        for text in [
            "2026-10-03",
            "10/3/2026",
            "10/3",
            "Oct 3",
            "Oct 3, 2026",
            "October 3",
        ] {
            assert_eq!(parse_date(text, today), Some(expected), "{text}");
        }
        assert_eq!(parse_date("someday", today), None);
        let three = NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        for text in ["15:00", "3:00 PM", "3pm", "3 pm", "03:00pm"] {
            assert_eq!(parse_time(text), Some(three), "{text}");
        }
        assert_eq!(parse_time("25:00"), None);
        assert_eq!(parse_date(&date_text(expected), today), Some(expected));
    }

    #[test]
    fn spans_and_relative_labels_read_naturally() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 26).unwrap();
        let dentist = fixtures::event("d", "Dentist", date, (15, 0), 60);
        assert_eq!(span(&dentist), "3 – 4 PM");
        let late = fixtures::event("l", "Late", date, (11, 30), 45);
        assert_eq!(span(&late), "11:30 AM – 12:15 PM");
        assert_eq!(day_label(date, date), "Today");
        assert_eq!(day_label(date + Duration::days(1), date), "Tomorrow");
        assert_eq!(day_label(date + Duration::days(2), date), "Mon, Sep 28");
        assert_eq!(until(1_000 + 25 * 60, 1_000), "in 25 min");
        assert_eq!(until(1_000 + 3 * 3600, 1_000), "in 3 h");
        assert_eq!(ago(1_000, 1_000 + 120), "2 min ago");
    }

    #[test]
    fn a_day_lists_all_day_events_first_and_includes_spanning_ones() {
        let events = fixtures::fortnight();
        let holiday_second_day = today() + Duration::days(6);
        let day = on_day(&events, holiday_second_day);
        assert_eq!(day[0].title, "Long weekend");
        assert!(day.iter().any(|e| e.title == "Brunch"));
        assert!(on_day(&events, today() + Duration::days(7)).is_empty());
    }
}
