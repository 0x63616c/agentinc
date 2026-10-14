//! The dates behind the week and month views: which days a view shows, how
//! to step between them, and which Events touch a day. Pure, so it is tested
//! without a window.
use ainc_client::types::CalendarEvent;
use chrono::{Datelike, Days, Local, Months, NaiveDate, TimeZone, Weekday};

/// Today's local date.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The Monday-to-Sunday week holding `day`.
pub fn week_of(day: NaiveDate) -> [NaiveDate; 7] {
    let monday = day - Days::new(u64::from(day.weekday().num_days_from_monday()));
    std::array::from_fn(|i| monday + Days::new(i as u64))
}

/// Whole weeks covering the month holding `day`, Monday first.
pub fn month_of(day: NaiveDate) -> Vec<[NaiveDate; 7]> {
    let first = day.with_day(1).expect("the first exists in every month");
    let last = (first + Months::new(1)) - Days::new(1);
    let mut weeks = vec![week_of(first)];
    while weeks.last().expect("one week")[6] < last {
        weeks.push(week_of(weeks.last().expect("one week")[6] + Days::new(1)));
    }
    weeks
}

/// `day` moved `delta` weeks.
pub fn step_week(day: NaiveDate, delta: i32) -> NaiveDate {
    if delta < 0 {
        day - Days::new(u64::from(delta.unsigned_abs()) * 7)
    } else {
        day + Days::new(u64::from(delta.unsigned_abs()) * 7)
    }
}

/// The first of the month `delta` months from `day`'s.
pub fn step_month(day: NaiveDate, delta: i32) -> NaiveDate {
    let first = day.with_day(1).expect("the first exists in every month");
    if delta < 0 {
        first - Months::new(delta.unsigned_abs())
    } else {
        first + Months::new(delta.unsigned_abs())
    }
}

/// The local epoch seconds `day` starts and ends at.
fn bounds(day: NaiveDate) -> (i64, i64) {
    let start = |d: NaiveDate| {
        Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).expect("midnight"))
            .earliest()
            .map_or(0, |t| t.timestamp())
    };
    (start(day), start(day + Days::new(1)))
}

/// The Events that touch `day`, in start order.
pub fn on_day(events: &[CalendarEvent], day: NaiveDate) -> Vec<&CalendarEvent> {
    let (start, end) = bounds(day);
    events
        .iter()
        .filter(|e| e.starts_at < end && e.ends_at > start)
        .collect()
}

pub fn is_weekend(day: NaiveDate) -> bool {
    matches!(day.weekday(), Weekday::Sat | Weekday::Sun)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }
    fn event(id: i64, starts_at: i64, ends_at: i64) -> CalendarEvent {
        CalendarEvent {
            id,
            title: format!("Event {id}"),
            starts_at,
            ends_at,
        }
    }

    #[test]
    fn weeks_run_monday_to_sunday() {
        let week = week_of(date(2026, 10, 14));
        assert_eq!(week[0], date(2026, 10, 12));
        assert_eq!(week[6], date(2026, 10, 18));
        assert_eq!(week_of(date(2026, 10, 18))[0], date(2026, 10, 12));
    }

    #[test]
    fn months_are_whole_weeks_around_the_month() {
        let weeks = month_of(date(2026, 10, 14));
        assert_eq!(weeks.len(), 5);
        assert_eq!(weeks[0][0], date(2026, 9, 28));
        assert_eq!(weeks[4][6], date(2026, 11, 1));
        assert_eq!(month_of(date(2027, 2, 10)).len(), 4);
    }

    #[test]
    fn stepping_moves_by_week_and_month() {
        assert_eq!(step_week(date(2026, 10, 14), 1), date(2026, 10, 21));
        assert_eq!(step_week(date(2026, 10, 14), -2), date(2026, 9, 30));
        assert_eq!(step_month(date(2026, 10, 31), 1), date(2026, 11, 1));
        assert_eq!(step_month(date(2026, 1, 15), -1), date(2025, 12, 1));
    }

    #[test]
    fn events_show_on_every_day_they_touch() {
        let (start, _) = bounds(date(2026, 10, 14));
        let hour = 3600;
        let events = [
            event(1, start + 9 * hour, start + 10 * hour),
            event(2, start + 22 * hour, start + 26 * hour),
            event(3, start - 5 * hour, start - hour),
        ];
        let ids = |day| {
            on_day(&events, day)
                .iter()
                .map(|e| e.title.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(date(2026, 10, 14)), ["Event 1", "Event 2"]);
        assert_eq!(ids(date(2026, 10, 15)), ["Event 2"]);
        assert_eq!(ids(date(2026, 10, 13)), ["Event 3"]);
        assert!(is_weekend(date(2026, 10, 17)) && !is_weekend(date(2026, 10, 14)));
    }
}
