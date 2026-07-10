use chrono::{DateTime, Utc};

/// Human-friendly age of a timestamp, e.g. `"2 days ago"`.
pub fn time_ago(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - then).num_seconds().max(0);
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;
    let weeks = days / 7;
    if secs < 60 {
        "Just now".to_owned()
    } else if mins < 60 {
        format!("{mins} minute{} ago", plural(mins))
    } else if hours < 24 {
        format!("{hours} hour{} ago", plural(hours))
    } else if days < 7 {
        format!("{days} day{} ago", plural(days))
    } else {
        format!("{weeks} week{} ago", plural(weeks))
    }
}

fn plural(n: i64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(secs_ago: i64) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc.with_ymd_and_hms(2026, 7, 9, 12, 0, 0).unwrap();
        (now - chrono::Duration::seconds(secs_ago), now)
    }

    #[test]
    fn time_ago_buckets() {
        let cases = [
            (30, "Just now"),
            (90, "1 minute ago"),
            (600, "10 minutes ago"),
            (3 * 3600, "3 hours ago"),
            (2 * 86_400, "2 days ago"),
            (10 * 86_400, "1 week ago"),
            (30 * 86_400, "4 weeks ago"),
        ];
        for (secs, expected) in cases {
            let (then, now) = at(secs);
            assert_eq!(time_ago(then, now), expected, "at {secs}s");
        }
    }

    #[test]
    fn time_ago_clamps_future_timestamps() {
        let (then, now) = at(-500);
        assert_eq!(time_ago(then, now), "Just now");
    }
}
