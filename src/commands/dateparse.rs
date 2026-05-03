use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use crate::error::{DevTodoError, Result};

/// Parse a flexible date input (`2025-01-01`, `2025-01-01T10:00:00`, `7d ago`, `yesterday`, ...)
/// into the ISO 8601 format used by the SQLite columns: `YYYY-MM-DDTHH:MM:SS`.
pub fn parse_to_db_format(input: &str) -> Result<String> {
    let trimmed = input.trim();

    // Plain date: pad with start-of-day for inclusive lower bounds.
    if let Ok(d) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok(d
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.format("%Y-%m-%dT%H:%M:%S").to_string());
    }

    // Relative shortcuts that dateparser can't handle: "today", "yesterday", "7d ago",
    // "2 weeks ago", "3m ago", etc.
    if let Some(dt) = parse_relative(trimmed) {
        return Ok(dt.format("%Y-%m-%dT%H:%M:%S").to_string());
    }

    // Fallback to dateparser for absolute formats it knows about.
    let dt: DateTime<Utc> =
        dateparser::parse(trimmed).map_err(|e| DevTodoError::InvalidDate {
            input: trimmed.to_string(),
            reason: e.to_string(),
        })?;
    Ok(dt.naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn parse_relative(input: &str) -> Option<NaiveDateTime> {
    let now = Utc::now().naive_utc();
    let lower = input.to_lowercase();
    match lower.as_str() {
        "now" => return Some(now),
        "today" => return Some(now.date().and_hms_opt(0, 0, 0).unwrap()),
        "yesterday" => {
            return Some(
                (now - Duration::days(1))
                    .date()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            );
        }
        _ => {}
    }

    // Forms: "7d ago", "2w ago", "1m ago", "1y ago", "3 days ago", "2 weeks ago"
    let stripped = lower.strip_suffix(" ago").unwrap_or(&lower);
    let parts: Vec<&str> = stripped.split_whitespace().collect();
    let (n_str, unit) = match parts.len() {
        2 => (parts[0], parts[1]),
        // Compact form like "7d" or "2w".
        1 => {
            let s = parts[0];
            let split_at = s.find(|c: char| c.is_alphabetic())?;
            (&s[..split_at], &s[split_at..])
        }
        _ => return None,
    };
    let n: i64 = n_str.parse().ok()?;
    let days = match unit.trim_end_matches('s') {
        "d" | "day" => n,
        "w" | "week" => n * 7,
        "m" | "month" => n * 30,
        "y" | "year" => n * 365,
        "h" | "hour" => return Some(now - Duration::hours(n)),
        _ => return None,
    };
    Some(now - Duration::days(days))
}

/// For end-of-day inclusive upper bounds: a bare date becomes `YYYY-MM-DDT23:59:59`.
pub fn parse_to_db_format_end(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok(d
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string());
    }
    parse_to_db_format(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_iso_date() {
        let s = parse_to_db_format("2025-01-15").unwrap();
        assert_eq!(s, "2025-01-15T00:00:00");
    }

    #[test]
    fn end_of_day_for_upper_bound() {
        let s = parse_to_db_format_end("2025-01-15").unwrap();
        assert_eq!(s, "2025-01-15T23:59:59");
    }

    #[test]
    fn parses_full_datetime() {
        let s = parse_to_db_format("2025-01-15T10:30:45").unwrap();
        assert_eq!(s, "2025-01-15T10:30:45");
    }

    #[test]
    fn rejects_garbage_with_invaliddate_variant() {
        let err = parse_to_db_format("not a date").unwrap_err();
        assert!(
            matches!(&err, crate::error::DevTodoError::InvalidDate { input, .. } if input == "not a date"),
            "expected InvalidDate, got {err:?}"
        );
    }

    #[test]
    fn parses_yesterday_today() {
        assert!(parse_to_db_format("yesterday").is_ok());
        assert!(parse_to_db_format("today").is_ok());
        assert!(parse_to_db_format("now").is_ok());
    }

    #[test]
    fn parses_compact_relative() {
        assert!(parse_to_db_format("7d").is_ok());
        assert!(parse_to_db_format("2w").is_ok());
    }

    #[test]
    fn parses_natural_relative() {
        assert!(parse_to_db_format("3 days ago").is_ok());
        assert!(parse_to_db_format("1 week ago").is_ok());
        assert!(parse_to_db_format("1 year ago").is_ok());
    }
}
