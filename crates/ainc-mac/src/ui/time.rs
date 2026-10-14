//! The one place the app turns clock values into words. Inputs are epoch
//! seconds (the daemon's unit); `duration` takes milliseconds because that
//! is what workflow executions carry. Output is local time.
use chrono::{DateTime, Datelike, Local, NaiveDate};

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const WEEK: i64 = 7 * DAY;

/// Epoch seconds now.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn local(seconds: i64) -> Option<DateTime<Local>> {
    DateTime::from_timestamp(seconds, 0).map(|t| t.with_timezone(&Local))
}

/// A short age: `now`, `5m`, `3h`, `2d`, then the date (`Sep 23`).
pub fn relative(then: i64, now: i64) -> String {
    let age = (now - then).max(0);
    match age {
        0..MINUTE => "now".into(),
        MINUTE..HOUR => format!("{}m", age / MINUTE),
        HOUR..DAY => format!("{}h", age / HOUR),
        DAY..WEEK => format!("{}d", age / DAY),
        _ => local(then)
            .map(|t| t.format("%b %-d").to_string())
            .unwrap_or_default(),
    }
}

/// A local date and time: `Sep 23, 16:06`, with the year when it is not this year.
pub fn absolute(seconds: i64) -> String {
    absolute_at(seconds, now())
}

fn absolute_at(seconds: i64, now: i64) -> String {
    let Some(time) = local(seconds) else {
        return "Time unavailable".into();
    };
    let this_year = local(now).is_some_and(|n| n.year() == time.year());
    if this_year {
        time.format("%b %-d, %H:%M").to_string()
    } else {
        time.format("%b %-d, %Y, %H:%M").to_string()
    }
}

/// A local wall-clock time: `14:30`.
pub fn clock(seconds: i64) -> String {
    local(seconds).map_or_else(
        || "Time unavailable".into(),
        |t| t.format("%H:%M").to_string(),
    )
}

/// A span of local times: `14:00 – 15:30`.
pub fn span(start: i64, end: i64) -> String {
    match (local(start), local(end)) {
        (Some(start), Some(end)) => format!("{} – {}", start.format("%H:%M"), end.format("%H:%M")),
        _ => "Time unavailable".into(),
    }
}

/// A day relative to `now`: `Today`, `Tomorrow`, `Yesterday`, then the
/// weekday and date (`Thursday, Oct 15`, with the year when it is not this year).
pub fn day(seconds: i64, now: i64) -> String {
    let (Some(day), Some(today)) = (local(seconds), local(now)) else {
        return "Undated".into();
    };
    let days = day
        .date_naive()
        .signed_duration_since(today.date_naive())
        .num_days();
    match days {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        _ if day.year() == today.year() => day.format("%A, %b %-d").to_string(),
        _ => day.format("%A, %b %-d, %Y").to_string(),
    }
}

/// A full local date: `Monday, October 12`.
pub fn long_date(seconds: i64) -> String {
    local(seconds).map_or_else(
        || "Time unavailable".into(),
        |t| t.format("%A, %B %-d").to_string(),
    )
}

/// A weekday's short name: `Mon`.
pub fn weekday(day: NaiveDate) -> String {
    day.format("%a").to_string()
}

/// A week by its ends: `Oct 12 – 18, 2026`, or `Sep 28 – Oct 4, 2026` across a month.
pub fn week_label(start: NaiveDate, end: NaiveDate) -> String {
    if start.month() == end.month() {
        format!("{} – {}, {}", start.format("%b %-d"), end.day(), end.year())
    } else {
        format!(
            "{} – {}, {}",
            start.format("%b %-d"),
            end.format("%b %-d"),
            end.year()
        )
    }
}

/// A month by name: `October 2026`.
pub fn month_label(day: NaiveDate) -> String {
    day.format("%B %Y").to_string()
}

/// An elapsed span: `45s`, `3m 05s`, `2h 14m`.
pub fn duration(millis: i64) -> String {
    let seconds = millis.max(0) / 1000;
    if seconds < MINUTE {
        format!("{seconds}s")
    } else if seconds < HOUR {
        format!("{}m {:02}s", seconds / MINUTE, seconds % MINUTE)
    } else {
        format!("{}h {:02}m", seconds / HOUR, (seconds % HOUR) / MINUTE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
        Local
            .with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn days_are_named_relative_to_today() {
        let today = at(2026, 10, 12, 9, 0);
        assert_eq!(day(today, today), "Today");
        assert_eq!(day(today + DAY, today), "Tomorrow");
        assert_eq!(day(today - DAY, today), "Yesterday");
        assert_eq!(day(today + 3 * DAY, today), "Thursday, Oct 15");
        assert_eq!(
            span(at(2026, 10, 12, 14, 0), at(2026, 10, 12, 15, 30)),
            "14:00 – 15:30"
        );
        assert_eq!(clock(at(2026, 10, 12, 14, 5)), "14:05");
        assert_eq!(long_date(today), "Monday, October 12");
        let date = |m, d| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
        assert_eq!(weekday(date(10, 12)), "Mon");
        assert_eq!(week_label(date(10, 12), date(10, 18)), "Oct 12 – 18, 2026");
        assert_eq!(week_label(date(9, 28), date(10, 4)), "Sep 28 – Oct 4, 2026");
        assert_eq!(month_label(date(10, 12)), "October 2026");
    }

    #[test]
    fn relative_steps_through_units_then_dates() {
        let then = at(2026, 9, 23, 16, 6);
        assert_eq!(relative(then, then + 30), "now");
        assert_eq!(relative(then, then - 30), "now");
        assert_eq!(relative(then, then + 5 * MINUTE), "5m");
        assert_eq!(relative(then, then + 3 * HOUR), "3h");
        assert_eq!(relative(then, then + 2 * DAY), "2d");
        assert_eq!(relative(then, then + 2 * WEEK), "Sep 23");
    }

    #[test]
    fn absolute_adds_the_year_only_when_it_differs() {
        let then = at(2026, 9, 23, 16, 6);
        assert_eq!(absolute_at(then, at(2026, 12, 1, 9, 0)), "Sep 23, 16:06");
        assert_eq!(
            absolute_at(then, at(2027, 1, 1, 9, 0)),
            "Sep 23, 2026, 16:06"
        );
    }

    #[test]
    fn duration_is_compact() {
        assert_eq!(duration(45_000), "45s");
        assert_eq!(duration(185_000), "3m 05s");
        assert_eq!(duration(2 * 3_600_000 + 14 * 60_000), "2h 14m");
        assert_eq!(duration(-5), "0s");
    }
}
