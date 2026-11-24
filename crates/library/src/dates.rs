use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};

pub const UNDEFINED: &str = "0101-01-01 00:00:00+00:00";

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

pub fn format_db(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S%.6f+00:00").to_string()
}

pub fn now_db() -> String {
    format_db(&now())
}

pub fn parse(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    for fmt in [
        "%Y-%m-%d %H:%M:%S%.f%:z",
        "%Y-%m-%dT%H:%M:%S%.f%:z",
        "%Y-%m-%d %H:%M:%S%.f%z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
    ] {
        if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
            return Some(dt.with_timezone(&Utc));
        }
    }
    if let Some(stripped) = s.strip_suffix('Z')
        && let Some(dt) = parse_naive(stripped)
    {
        return Some(dt);
    }
    if let Some(dt) = parse_naive(s) {
        return Some(dt);
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|n| Utc.from_utc_datetime(&n))
}

fn parse_naive(s: &str) -> Option<DateTime<Utc>> {
    for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(Utc.from_utc_datetime(&n));
        }
    }
    None
}

pub fn is_undefined(dt: &DateTime<Utc>) -> bool {
    dt.year() < 101 || (dt.year() == 101 && dt.month() == 1 && dt.day() <= 2)
}

pub fn iso(dt: &DateTime<Utc>) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.timestamp_subsec_micros() % 1_000_000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dt = parse("2024-05-01 18:49:25.003532+00:00").unwrap();
        assert_eq!(format_db(&dt), "2024-05-01 18:49:25.003532+00:00");
        assert_eq!(iso(&dt), "2024-05-01T18:49:25.003532+00:00");
        assert_eq!(
            parse("2024-05-01T18:49:25+02:00").unwrap().to_rfc3339(),
            "2024-05-01T16:49:25+00:00"
        );
        assert_eq!(parse("2024-05-01").unwrap().year(), 2024);
        assert!(parse("garbage").is_none());
    }

    #[test]
    fn undefined() {
        let dt = parse(UNDEFINED).unwrap();
        assert!(is_undefined(&dt));
        assert!(!is_undefined(
            &parse("2013-08-13 21:03:06.859085+00:00").unwrap()
        ));
    }
}
