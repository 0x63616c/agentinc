//! The one place the app turns clock values into words. Inputs are epoch
//! seconds (the daemon's unit); `duration` takes milliseconds because that
//! is what workflow executions carry. Output is local time.
use chrono::{DateTime, Datelike, Local};

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
