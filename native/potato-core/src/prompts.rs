//! Native prompt resources and factual, per-turn clock context.
use chrono::{DateTime, FixedOffset, Utc};
use chrono_tz::Tz;

pub(crate) const CORE: &str = include_str!("../prompts/core.md");
pub(crate) const SCHEDULING: &str = include_str!("../prompts/scheduling.md");

fn host_timezone() -> Option<Tz> {
    if let Ok(zone) = std::env::var("TZ") {
        return zone.trim_start_matches(':').parse().ok();
    }
    // macOS and common Unix installations expose the zone through this link.
    if let Ok(path) = std::fs::canonicalize("/etc/localtime") {
        if let Some((_, zone)) = path.to_string_lossy().split_once("/zoneinfo/") {
            if let Ok(zone) = zone.parse() {
                return Some(zone);
            }
        }
    }
    None
}

fn format_time(now: DateTime<Utc>, local: FixedOffset, zone: Option<Tz>) -> String {
    match zone {
        Some(zone) => format!(
            "Current time (UTC): {}\nHost local time: {}\nHost IANA timezone: {zone}",
            now.to_rfc3339(), now.with_timezone(&zone).to_rfc3339()
        ),
        None => format!(
            "Current time (UTC): {}\nHost local time: {}\nHost IANA timezone: unknown; the current offset does not identify a timezone or future daylight-saving rules.",
            now.to_rfc3339(), now.with_timezone(&local).to_rfc3339()
        ),
    }
}

pub(crate) fn time_context() -> String {
    let now = chrono::Local::now();
    format_time(now.with_timezone(&Utc), *now.offset(), host_timezone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_uses_zone_rules_and_does_not_infer_a_zone_from_an_offset() {
        let offset = FixedOffset::east_opt(8 * 3600).unwrap();
        let winter = "2026-01-15T16:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let summer = "2026-07-15T16:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let zone = "America/New_York".parse().unwrap();
        assert!(format_time(winter, offset, Some(zone)).contains("11:00:00-05:00"));
        assert!(format_time(summer, offset, Some(zone)).contains("12:00:00-04:00"));
        let unknown = format_time(winter, offset, None);
        assert!(unknown.contains("2026-01-16T00:00:00+08:00"));
        assert!(unknown.contains("IANA timezone: unknown"));
        assert!(!unknown.contains("Asia/Shanghai"));
    }
}
