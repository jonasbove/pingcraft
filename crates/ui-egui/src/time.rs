//! Times as the inbox and the chat show them, through `chrono` (house rule 5), in the computer's time zone.

use chrono::{DateTime, Datelike, Local, TimeZone};
use pingcraft_core::Timestamp;

use crate::tl;

fn local(at: Timestamp) -> Option<DateTime<Local>> {
    let millis = i64::try_from(at.0).ok()?;
    Local.timestamp_millis_opt(millis).single()
}

/// The time of day, `14:32`.
pub fn clock(at: Timestamp) -> String {
    local(at).map(|t| t.format("%H:%M").to_string()).unwrap_or_default()
}

/// The full date and time, for a message's details: "Tuesday 24 November 2026 · 14:28:03".
pub fn full(at: Timestamp) -> String {
    local(at).map(|t| t.format("%A %-d %B %Y · %H:%M:%S").to_string()).unwrap_or_default()
}

/// What the inbox shows for a chat's last message: the time today, the weekday this week, else the date.
pub fn short_time(at: Timestamp) -> String {
    short_time_at(at, Local::now())
}

fn short_time_at(at: Timestamp, now: DateTime<Local>) -> String {
    let Some(t) = local(at) else { return String::new() };
    let days = now.date_naive().signed_duration_since(t.date_naive()).num_days();
    match days {
        0 => t.format("%H:%M").to_string(),
        1..=6 => t.format("%a").to_string(),
        _ => t.format("%-d %b").to_string(),
    }
}

/// The day divider's label: "Today · 24 Nov", "Yesterday · 23 Nov", or the date.
pub fn day_label(at: Timestamp) -> String {
    day_label_at(at, Local::now())
}

fn day_label_at(at: Timestamp, now: DateTime<Local>) -> String {
    let Some(t) = local(at) else { return String::new() };
    let date = if t.year() == now.year() { t.format("%-d %b").to_string() } else { t.format("%-d %b %Y").to_string() };
    match now.date_naive().signed_duration_since(t.date_naive()).num_days() {
        0 => format!("{} · {date}", tl!("Today")),
        1 => format!("{} · {date}", tl!("Yesterday")),
        _ => date,
    }
}

#[cfg(test)]
mod tests {
    use super::{day_label_at, short_time_at};
    use chrono::{Local, TimeZone};
    use pingcraft_core::Timestamp;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> (Timestamp, chrono::DateTime<Local>) {
        let t = Local.with_ymd_and_hms(y, m, d, h, min, 0).single().unwrap();
        (Timestamp(u64::try_from(t.timestamp_millis()).unwrap()), t)
    }

    #[test]
    fn today_shows_the_time_this_week_the_day_and_earlier_the_date() {
        let (_, now) = at(2026, 11, 24, 15, 0);
        assert_eq!(short_time_at(at(2026, 11, 24, 14, 32).0, now), "14:32");
        assert_eq!(short_time_at(at(2026, 11, 23, 9, 0).0, now), "Mon");
        assert_eq!(short_time_at(at(2026, 11, 10, 9, 0).0, now), "10 Nov");
        assert_eq!(day_label_at(at(2026, 11, 24, 9, 0).0, now), "Today · 24 Nov");
        assert_eq!(day_label_at(at(2026, 11, 23, 9, 0).0, now), "Yesterday · 23 Nov");
        assert_eq!(day_label_at(at(2025, 1, 2, 9, 0).0, now), "2 Jan 2025");
    }
}
