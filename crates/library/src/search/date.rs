use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

#[derive(Clone, Debug)]
pub enum DateQuery {
    Defined(bool),
    Cmp {
        op: Op,
        date: DateTime<Utc>,
        fields: u8,
    },
}

pub fn split_op(q: &str) -> (Op, &str) {
    for (p, op) in [
        ("!=", Op::Ne),
        (">=", Op::Ge),
        ("<=", Op::Le),
        ("=", Op::Eq),
        (">", Op::Gt),
        ("<", Op::Lt),
    ] {
        if let Some(rest) = q.strip_prefix(p) {
            return (op, rest);
        }
    }
    (Op::Eq, q)
}

fn months_ago(dt: DateTime<Utc>, n: i64) -> Option<DateTime<Utc>> {
    let total = dt.year() as i64 * 12 + dt.month() as i64 - 1 - n;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    let d = NaiveDate::from_ymd_opt(i32::try_from(y).ok()?, m as u32, 1)?;
    Some(Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0)?))
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn month_from_name(w: &str) -> Option<u32> {
    if w.len() < 3 {
        return None;
    }
    let w = w.to_lowercase();
    MONTHS
        .iter()
        .position(|m| w.starts_with(m))
        .map(|i| i as u32 + 1)
}

fn ymd(y: i32, m: u32, d: u32) -> Option<DateTime<Utc>> {
    let n = NaiveDate::from_ymd_opt(y, m, d)?.and_hms_opt(0, 0, 0)?;
    Some(Utc.from_utc_datetime(&n))
}

fn parse_absolute(q: &str) -> Option<(DateTime<Utc>, u8)> {
    let words: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return None;
    }
    let mut nums: Vec<u32> = Vec::new();
    let mut year: Option<i32> = None;
    let mut month: Option<u32> = None;
    let mut digit_order: Vec<&str> = Vec::new();
    for w in &words {
        if let Ok(n) = w.parse::<u32>() {
            if w.len() == 4 {
                if year.is_some() {
                    return None;
                }
                year = Some(n as i32);
            } else if w.len() <= 2 {
                nums.push(n);
            } else {
                return None;
            }
            digit_order.push(w);
        } else if let Some(m) = month_from_name(w) {
            if month.is_some() {
                return None;
            }
            month = Some(m);
        } else if !matches!(w.to_lowercase().as_str(), "st" | "nd" | "rd" | "th" | "of") {
            return None;
        }
    }
    let year = year?;
    let (m, d, fields) = match (month, nums.as_slice()) {
        (Some(m), []) => (m, 1, 2),
        (Some(m), [d]) => (m, *d, 3),
        (None, []) => (1, 1, 1),
        (None, [a]) => (*a, 1, 2),
        (None, [a, b]) => {
            let first_is_year = digit_order.first().map(|w| w.len() == 4).unwrap_or(false);
            if first_is_year {
                (*a, *b, 3)
            } else if *a > 12 {
                (*b, *a, 3)
            } else {
                (*a, *b, 3)
            }
        }
        _ => return None,
    };
    Some((ymd(year, m, d)?, fields))
}

pub fn parse(query: &str, now: DateTime<Utc>) -> Result<DateQuery> {
    if query.len() < 2 {
        return Err(Error::Search(format!("date conversion error: {query}")));
    }
    if query == "true" || query == "false" {
        return Ok(DateQuery::Defined(query == "true"));
    }
    let (op, q) = split_op(query);
    let q = q.trim();
    let bad = || Error::Search(format!("date conversion error: {q}"));
    let (date, fields) = match q.trim_start_matches('_') {
        "today" => (now, 3),
        "yesterday" => (now - Duration::days(1), 3),
        "thismonth" => (now, 2),
        "lastmonth" => (months_ago(now, 1).ok_or_else(bad)?, 2),
        "thisyear" => (now, 1),
        "lastyear" => (months_ago(now, 12).ok_or_else(bad)?, 1),
        other => {
            let ago = [("daysago", 3u8), ("monthsago", 2), ("yearsago", 1)]
                .iter()
                .find_map(|(suffix, f)| {
                    other
                        .strip_suffix(suffix)
                        .map(|n| (n.trim_end_matches('_'), *f, *suffix))
                });
            if let Some((num, fields, unit)) = ago {
                let n: i64 = num
                    .parse()
                    .map_err(|_| Error::Search(format!("number conversion error: {num}")))?;
                let d = match unit {
                    "daysago" => now.checked_sub_signed(Duration::days(n)),
                    "monthsago" => months_ago(now, n),
                    _ => months_ago(now, n.checked_mul(12).ok_or_else(bad)?),
                };
                (
                    d.ok_or_else(|| Error::Search(format!("date out of range: {q}")))?,
                    fields,
                )
            } else {
                parse_absolute(other).ok_or_else(bad)?
            }
        }
    };
    Ok(DateQuery::Cmp { op, date, fields })
}

fn eq(db: &DateTime<Utc>, q: &DateTime<Utc>, fields: u8) -> bool {
    db.year() == q.year()
        && (fields == 1 || (db.month() == q.month() && (fields == 2 || db.day() == q.day())))
}

fn gt(db: &DateTime<Utc>, q: &DateTime<Utc>, fields: u8) -> bool {
    if db.year() > q.year() {
        return true;
    }
    if fields > 1 && db.year() == q.year() {
        if db.month() > q.month() {
            return true;
        }
        return fields == 3 && db.month() == q.month() && db.day() > q.day();
    }
    false
}

fn lt(db: &DateTime<Utc>, q: &DateTime<Utc>, fields: u8) -> bool {
    if db.year() < q.year() {
        return true;
    }
    if fields > 1 && db.year() == q.year() {
        if db.month() < q.month() {
            return true;
        }
        return fields == 3 && db.month() == q.month() && db.day() < q.day();
    }
    false
}

impl DateQuery {
    pub fn matches(&self, value: Option<&DateTime<Utc>>) -> bool {
        match self {
            DateQuery::Defined(want) => value.is_some() == *want,
            DateQuery::Cmp { op, date, fields } => {
                let Some(v) = value else { return false };
                match op {
                    Op::Eq => eq(v, date, *fields),
                    Op::Ne => !eq(v, date, *fields),
                    Op::Gt => gt(v, date, *fields),
                    Op::Le => !gt(v, date, *fields),
                    Op::Lt => lt(v, date, *fields),
                    Op::Ge => !lt(v, date, *fields),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> DateTime<Utc> {
        crate::dates::parse(s).unwrap()
    }

    fn m(q: &str, v: &str) -> bool {
        parse(q, d("2024-06-15 12:00:00+00:00"))
            .unwrap()
            .matches(Some(&d(v)))
    }

    #[test]
    fn absolute() {
        assert!(m("2023", "2023-05-01"));
        assert!(!m("2023", "2022-05-01"));
        assert!(m(">2020", "2023-05-01"));
        assert!(!m(">2023", "2023-12-31"));
        assert!(m(">=2023-01", "2023-01-01"));
        assert!(!m(">=2023-02", "2023-01-31"));
        assert!(m("=2023-05-01", "2023-05-01 10:00:00+00:00"));
        assert!(m("<2023-05-02", "2023-05-01"));
        assert!(m("!=2023", "2024-01-01"));
        assert!(m("jan 2023", "2023-01-20"));
        assert!(m("2023-1", "2023-01-20"));
        assert!(m("1.2023", "2023-01-20"));
        assert!(m(">=15 march 2023", "2023-03-15"));
    }

    #[test]
    fn relative() {
        assert!(m(">30daysago", "2024-06-01"));
        assert!(!m(">30daysago", "2024-01-01"));
        assert!(m("today", "2024-06-15 01:00:00+00:00"));
        assert!(m("yesterday", "2024-06-14"));
        assert!(m("thismonth", "2024-06-01"));
        assert!(m("lastmonth", "2024-05-31"));
        assert!(m("thisyear", "2024-01-01"));
        assert!(m("lastyear", "2023-01-01"));
        assert!(m("<2monthsago", "2024-03-01"));
        assert!(m(">=1yearsago", "2023-07-01"));
    }

    #[test]
    fn defined() {
        let q = parse("true", d("2024-01-01")).unwrap();
        assert!(q.matches(Some(&d("2020-01-01"))));
        assert!(!q.matches(None));
        let q = parse("false", d("2024-01-01")).unwrap();
        assert!(q.matches(None));
    }

    #[test]
    fn errors() {
        assert!(parse("garbage", d("2024-01-01")).is_err());
        assert!(parse("xdaysago", d("2024-01-01")).is_err());
        assert!(parse("2023-13-45", d("2024-01-01")).is_err());
    }
}
