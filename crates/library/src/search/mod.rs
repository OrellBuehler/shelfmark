mod date;
mod parser;

use std::collections::HashSet;

use regex::{Regex, RegexBuilder};

use self::date::{DateQuery, Op};
pub use self::parser::{LOCATIONS, Node, parse};
use crate::error::{Error, Result};
use crate::langs;
use crate::record::Record;
use crate::text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Title,
    TitleSort,
    Authors,
    AuthorSort,
    Tags,
    Series,
    SeriesSort,
    Publisher,
    Comments,
    Formats,
    Languages,
    Identifiers,
    Uuid,
    Rating,
    Cover,
    Id,
    Pages,
    SeriesIndex,
    Size,
    Timestamp,
    Pubdate,
    LastModified,
    Narrators,
    Audio,
    Ebook,
    Duration,
}

const ALL_FIELDS: &[Field] = &[
    Field::Title,
    Field::Authors,
    Field::AuthorSort,
    Field::Tags,
    Field::Series,
    Field::Publisher,
    Field::Comments,
    Field::Formats,
    Field::Languages,
    Field::Identifiers,
    Field::TitleSort,
    Field::Narrators,
    Field::Rating,
];

fn field_for(location: &str) -> Option<Field> {
    Some(match location {
        "title" => Field::Title,
        "title_sort" => Field::TitleSort,
        "authors" | "author" => Field::Authors,
        "author_sort" => Field::AuthorSort,
        "tags" | "tag" => Field::Tags,
        "series" => Field::Series,
        "series_sort" => Field::SeriesSort,
        "publisher" => Field::Publisher,
        "comments" | "comment" => Field::Comments,
        "formats" | "format" => Field::Formats,
        "languages" | "language" => Field::Languages,
        "identifiers" | "identifier" | "isbn" => Field::Identifiers,
        "uuid" => Field::Uuid,
        "rating" => Field::Rating,
        "cover" => Field::Cover,
        "id" => Field::Id,
        "pages" => Field::Pages,
        "series_index" => Field::SeriesIndex,
        "size" => Field::Size,
        "date" => Field::Timestamp,
        "pubdate" => Field::Pubdate,
        "last_modified" => Field::LastModified,
        "narrators" | "narrator" => Field::Narrators,
        "audio" | "audiobook" => Field::Audio,
        "ebook" => Field::Ebook,
        "duration" => Field::Duration,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Contains,
    Equals,
    Regex,
    Accent,
}

#[derive(Clone, Debug)]
struct TextMatch {
    kind: Kind,
    q: String,
    norm: String,
    re: Option<Regex>,
}

impl TextMatch {
    fn new(raw: &str) -> Result<Self> {
        let (kind, q) = matchkind(raw);
        let re = if kind == Kind::Regex {
            Some(
                RegexBuilder::new(&q)
                    .case_insensitive(true)
                    .build()
                    .map_err(|e| Error::Search(format!("invalid regular expression {q:?}: {e}")))?,
            )
        } else {
            None
        };
        let norm = match kind {
            Kind::Contains => text::primary_no_punc(&q),
            Kind::Accent => text::primary(&q),
            _ => String::new(),
        };
        Ok(TextMatch { kind, q, norm, re })
    }

    fn one(&self, value: &str) -> bool {
        match self.kind {
            Kind::Contains => {
                if self.norm.is_empty() {
                    value.to_lowercase().contains(&self.q)
                } else {
                    text::primary_no_punc(value).contains(&self.norm)
                }
            }
            Kind::Accent => text::primary(value).contains(&self.norm),
            Kind::Regex => self.re.as_ref().is_some_and(|r| r.is_match(value)),
            Kind::Equals => {
                let t = value.to_lowercase();
                let q = self.q.as_str();
                if let Some(internal) = q.strip_prefix("..") {
                    let q = format!(".{internal}");
                    return q == t
                        || t.split('.')
                            .map(str::trim)
                            .filter(|c| !c.is_empty())
                            .any(|c| c == internal);
                }
                if let Some(prefix) = q.strip_prefix('.') {
                    return t
                        .strip_prefix(prefix)
                        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'));
                }
                q == t
            }
        }
    }
}

fn matchkind(query: &str) -> (Kind, String) {
    let mut kind = Kind::Contains;
    let mut q = query;
    if query.chars().count() > 1 {
        if let Some(r) = query.strip_prefix('\\') {
            q = r;
        } else if let Some(r) = query.strip_prefix('=') {
            kind = Kind::Equals;
            q = r;
        } else if let Some(r) = query.strip_prefix('~') {
            kind = Kind::Regex;
            q = r;
        } else if let Some(r) = query.strip_prefix('^') {
            kind = Kind::Accent;
            q = r;
        }
    }
    if kind == Kind::Regex {
        (kind, q.to_string())
    } else {
        (kind, q.to_lowercase())
    }
}

#[derive(Clone, Debug)]
enum NumQuery {
    Present(bool),
    Cmp { op: Op, value: f64 },
}

impl NumQuery {
    fn parse(query: &str, int_only: bool) -> Result<Self> {
        if query == "true" || query == "false" {
            return Ok(NumQuery::Present(query == "true"));
        }
        let (op, mut q) = date::split_op(query);
        let mut mult = 1.0;
        if q.chars().count() > 1 {
            let last = q.chars().last().unwrap().to_ascii_lowercase();
            let m = match last {
                'k' => 1024.0,
                'm' => 1024.0 * 1024.0,
                'g' => 1024.0 * 1024.0 * 1024.0,
                _ => 1.0,
            };
            if m != 1.0 {
                mult = m;
                q = &q[..q.len() - 1];
            }
        }
        let bad = || Error::Search(format!("non-numeric value in query: {query}"));
        let base = if int_only {
            q.trim().parse::<i64>().map_err(|_| bad())? as f64
        } else {
            q.trim().parse::<f64>().map_err(|_| bad())?
        };
        Ok(NumQuery::Cmp {
            op,
            value: base * mult,
        })
    }

    fn parse_plain(query: &str) -> Result<Self> {
        if query == "true" || query == "false" {
            return Ok(NumQuery::Present(query == "true"));
        }
        let (op, q) = date::split_op(query);
        let value = q
            .trim()
            .trim_end_matches('h')
            .parse::<f64>()
            .map_err(|_| Error::Search(format!("non-numeric value in query: {query}")))?;
        Ok(NumQuery::Cmp { op, value })
    }

    fn matches(&self, v: Option<f64>) -> bool {
        match self {
            NumQuery::Present(want) => v.is_some() == *want,
            NumQuery::Cmp { op, value } => {
                let Some(v) = v else { return false };
                match op {
                    Op::Eq => v == *value,
                    Op::Ne => v != *value,
                    Op::Gt => v > *value,
                    Op::Ge => v >= *value,
                    Op::Lt => v < *value,
                    Op::Le => v <= *value,
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct PairQuery {
    key: Option<TextMatch>,
    val: Option<TextMatch>,
    truth: Option<(Option<String>, bool)>,
}

impl PairQuery {
    fn new(query: &str) -> Result<Self> {
        let (key_raw, val_raw) = match query.split_once(':') {
            Some((k, v)) => (Some(k.trim()), v.trim()),
            None => (None, query),
        };
        let key_q = key_raw.map(|k| matchkind(k).1).filter(|k| !k.is_empty());
        let val_q = matchkind(val_raw).1;
        if val_q == "true" || val_q == "false" {
            return Ok(PairQuery {
                key: None,
                val: None,
                truth: Some((key_q, val_q == "true")),
            });
        }
        let key = match (key_raw, key_q) {
            (Some(k), Some(_)) => Some(TextMatch::new(k)?),
            _ => None,
        };
        let val = if val_q.is_empty() {
            None
        } else {
            Some(TextMatch::new(val_raw)?)
        };
        Ok(PairQuery {
            key,
            val,
            truth: None,
        })
    }

    fn matches(&self, r: &Record) -> bool {
        if let Some((key, want)) = &self.truth {
            let found = match key {
                Some(k) => r.identifiers.iter().any(|(t, v)| t == k && !v.is_empty()),
                None => !r.identifiers.is_empty(),
            };
            return found == *want;
        }
        r.identifiers.iter().any(|(t, v)| {
            self.key.as_ref().is_none_or(|k| k.one(t)) && self.val.as_ref().is_none_or(|m| m.one(v))
        })
    }
}

#[derive(Clone, Debug)]
enum Leaf {
    Text { field: Field, m: TextMatch },
    HasValue { field: Field, want: bool },
    Num { field: Field, q: NumQuery },
    RatingExact(NumQuery),
    Count { field: Field, q: NumQuery },
    Date { field: Field, q: DateQuery },
    Pair(PairQuery),
    Any(Vec<Leaf>),
}

#[derive(Clone, Debug)]
enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Never,
    Leaf(Leaf),
}

#[derive(Clone, Debug)]
pub struct Compiled {
    expr: Expr,
    needs_comments: bool,
}

pub fn compile(query: &str, saved: &dyn Fn(&str) -> Option<String>) -> Result<Compiled> {
    let mut needs_comments = false;
    let mut seen = HashSet::new();
    let node = parse(query)?;
    let expr = build(&node, saved, &mut seen, &mut needs_comments)?;
    Ok(Compiled {
        expr,
        needs_comments,
    })
}

impl Compiled {
    pub fn matches(&self, r: &Record, comments: Option<&str>) -> bool {
        eval(&self.expr, &Ctx { r, comments })
    }

    pub fn needs_comments(&self) -> bool {
        self.needs_comments
    }
}

fn build(
    node: &Node,
    saved: &dyn Fn(&str) -> Option<String>,
    seen: &mut HashSet<String>,
    needs: &mut bool,
) -> Result<Expr> {
    Ok(match node {
        Node::And(a, b) => Expr::And(
            Box::new(build(a, saved, seen, needs)?),
            Box::new(build(b, saved, seen, needs)?),
        ),
        Node::Or(a, b) => Expr::Or(
            Box::new(build(a, saved, seen, needs)?),
            Box::new(build(b, saved, seen, needs)?),
        ),
        Node::Not(a) => Expr::Not(Box::new(build(a, saved, seen, needs)?)),
        Node::Token { loc, query } => {
            if loc == "search" {
                let name = query.strip_prefix('=').unwrap_or(query).to_lowercase();
                if !seen.insert(name.clone()) {
                    return Err(Error::Search(format!("recursive saved search: {query}")));
                }
                let text = saved(&name)
                    .ok_or_else(|| Error::Search(format!("unknown saved search: {query}")))?;
                let inner = parse(&text)?;
                let expr = build(&inner, saved, seen, needs)?;
                seen.remove(&name);
                return Ok(expr);
            }
            if query.trim().is_empty() {
                return Ok(Expr::Never);
            }
            leaf(loc, query, needs)?
        }
    })
}

fn is_int_valued(q: &str) -> Option<i64> {
    if let Ok(i) = q.parse::<i64>() {
        return Some(i);
    }
    q.parse::<f64>()
        .ok()
        .filter(|f| f.is_finite() && f.fract() == 0.0)
        .map(|f| f as i64)
}

fn text_leaf(field: Field, query: &str) -> Result<Leaf> {
    let q = if field == Field::Languages {
        let (kind, inner) = matchkind(query);
        match langs::canonicalize(&inner) {
            Some(code) => match kind {
                Kind::Equals => format!("={code}"),
                Kind::Contains => code,
                _ => query.to_string(),
            },
            None => query.to_string(),
        }
    } else {
        query.to_string()
    };
    let (kind, inner) = matchkind(&q);
    if kind == Kind::Contains && (inner == "true" || inner == "false") {
        return Ok(Leaf::HasValue {
            field,
            want: inner == "true",
        });
    }
    Ok(Leaf::Text {
        field,
        m: TextMatch::new(&q)?,
    })
}

fn leaf(loc: &str, query: &str, needs: &mut bool) -> Result<Expr> {
    let lowered = query.to_lowercase();
    if loc == "all" {
        *needs = true;
        let mut leaves = Vec::new();
        let int_query = is_int_valued(query.trim());
        for &f in ALL_FIELDS {
            if f == Field::Rating {
                if let Some(i) = int_query {
                    leaves.push(Leaf::Num {
                        field: Field::Rating,
                        q: NumQuery::Cmp {
                            op: Op::Eq,
                            value: i as f64,
                        },
                    });
                }
                continue;
            }
            if f == Field::Identifiers {
                let (kind, inner) = matchkind(query);
                if kind == Kind::Contains && (inner == "true" || inner == "false") {
                    leaves.push(Leaf::HasValue {
                        field: f,
                        want: inner == "true",
                    });
                } else {
                    let m = TextMatch::new(query)?;
                    leaves.push(Leaf::Pair(PairQuery {
                        key: Some(m.clone()),
                        val: None,
                        truth: None,
                    }));
                    leaves.push(Leaf::Pair(PairQuery {
                        key: None,
                        val: Some(m),
                        truth: None,
                    }));
                }
                continue;
            }
            leaves.push(text_leaf(f, query)?);
        }
        return Ok(Expr::Leaf(Leaf::Any(leaves)));
    }
    let field =
        field_for(loc).ok_or_else(|| Error::Search(format!("unknown search field: {loc}")))?;
    let leaf = match field {
        Field::Timestamp | Field::Pubdate | Field::LastModified => Leaf::Date {
            field,
            q: date::parse(&lowered, crate::dates::now())?,
        },
        Field::Rating => {
            let q = NumQuery::parse(&lowered, false)?;
            match q {
                NumQuery::Cmp { value, .. } if value.fract() != 0.0 => Leaf::RatingExact(q),
                _ => Leaf::Num { field, q },
            }
        }
        Field::Id | Field::Pages | Field::Cover => Leaf::Num {
            field,
            q: NumQuery::parse(&lowered, true)?,
        },
        Field::Size | Field::SeriesIndex => Leaf::Num {
            field,
            q: NumQuery::parse(&lowered, false)?,
        },
        Field::Duration => Leaf::Num {
            field,
            q: NumQuery::parse_plain(&lowered)?,
        },
        Field::Audio | Field::Ebook => Leaf::HasValue {
            field,
            want: !matches!(lowered.trim(), "false" | "no" | "0"),
        },
        Field::Identifiers => {
            let q = if loc == "isbn" {
                format!("=isbn:{query}")
            } else {
                query.to_string()
            };
            Leaf::Pair(PairQuery::new(&q)?)
        }
        Field::Authors | Field::Tags | Field::Languages | Field::Formats
            if lowered.len() > 1
                && lowered.starts_with('#')
                && lowered[1..].starts_with(['=', '<', '>', '!']) =>
        {
            Leaf::Count {
                field,
                q: NumQuery::parse(&lowered[1..], true)?,
            }
        }
        _ => {
            if field == Field::Comments {
                *needs = true;
            }
            text_leaf(field, query)?
        }
    };
    Ok(Expr::Leaf(leaf))
}

struct Ctx<'a> {
    r: &'a Record,
    comments: Option<&'a str>,
}

fn eval(e: &Expr, c: &Ctx) -> bool {
    match e {
        Expr::And(a, b) => eval(a, c) && eval(b, c),
        Expr::Or(a, b) => eval(a, c) || eval(b, c),
        Expr::Not(a) => !eval(a, c),
        Expr::Never => false,
        Expr::Leaf(l) => eval_leaf(l, c),
    }
}

fn any_value(c: &Ctx, f: Field, mut pred: impl FnMut(&str) -> bool) -> bool {
    let r = c.r;
    match f {
        Field::Title => pred(&r.title),
        Field::TitleSort => pred(&r.sort),
        Field::Authors => r.authors.iter().any(|a| pred(a)),
        Field::AuthorSort => pred(&r.author_sort),
        Field::Tags => r.tags.iter().any(|a| pred(a)),
        Field::Series => r.series.as_deref().is_some_and(pred),
        Field::SeriesSort => r.series.as_deref().is_some_and(|s| {
            pred(&text::title_sort(
                s,
                r.languages.first().map(String::as_str),
            ))
        }),
        Field::Publisher => r.publisher.as_deref().is_some_and(pred),
        Field::Comments => c.comments.is_some_and(pred),
        Field::Formats => {
            r.formats.iter().any(|x| pred(&x.format))
                || r.audio
                    .as_ref()
                    .is_some_and(|a| a.formats.iter().any(|f| pred(f)))
        }
        Field::Languages => r.languages.iter().any(|a| pred(a)),
        Field::Uuid => pred(&r.uuid),
        Field::Narrators => r
            .audio
            .as_ref()
            .is_some_and(|a| a.narrators.iter().any(|n| pred(n))),
        _ => false,
    }
}

fn has_value(c: &Ctx, f: Field) -> bool {
    let r = c.r;
    match f {
        Field::Title => !r.title.trim().is_empty(),
        Field::TitleSort => !r.sort.trim().is_empty(),
        Field::Authors => !r.authors.is_empty(),
        Field::AuthorSort => !r.author_sort.trim().is_empty(),
        Field::Tags => !r.tags.is_empty(),
        Field::Series | Field::SeriesSort => r.series.is_some(),
        Field::Publisher => r.publisher.is_some(),
        Field::Comments => c.comments.is_some_and(|c| !c.trim().is_empty()),
        Field::Formats => !r.formats.is_empty() || r.audio.is_some(),
        Field::Narrators => r.audio.as_ref().is_some_and(|a| !a.narrators.is_empty()),
        Field::Audio => r.audio.is_some(),
        Field::Ebook => !r.is_audio_entry() || r.ebook_paired.is_some(),
        Field::Languages => !r.languages.is_empty(),
        Field::Identifiers => !r.identifiers.is_empty(),
        Field::Uuid => !r.uuid.is_empty(),
        Field::Rating => r.rating > 0,
        _ => true,
    }
}

fn num_value(r: &Record, f: Field) -> Option<f64> {
    match f {
        Field::Rating => (r.rating > 0).then_some((r.rating / 2) as f64),
        Field::Cover => Some(if r.has_cover { 1.0 } else { 0.0 }),
        Field::Id => Some(r.id as f64),
        Field::Pages => r.pages.map(|p| p as f64),
        Field::SeriesIndex => Some(r.series_index),
        Field::Size => (!r.formats.is_empty() || r.is_audio_entry()).then_some(r.size as f64),
        Field::Duration => r
            .audio
            .as_ref()
            .map(|a| a.duration_ms as f64 / 3_600_000.0),
        _ => None,
    }
}

fn eval_leaf(l: &Leaf, c: &Ctx) -> bool {
    let r = c.r;
    match l {
        Leaf::Text { field, m } => any_value(c, *field, |v| m.one(v)),
        Leaf::HasValue { field, want } => has_value(c, *field) == *want,
        Leaf::Num { field, q } => {
            if *field == Field::Cover
                && let NumQuery::Present(want) = q
            {
                return r.has_cover == *want;
            }
            q.matches(num_value(r, *field))
        }
        Leaf::RatingExact(q) => q.matches((r.rating > 0).then(|| r.rating as f64 / 2.0)),
        Leaf::Count { field, q } => {
            let n = match field {
                Field::Authors => r.authors.len(),
                Field::Tags => r.tags.len(),
                Field::Languages => r.languages.len(),
                _ => r.formats.len(),
            };
            q.matches(Some(n as f64))
        }
        Leaf::Date { field, q } => {
            let v = match field {
                Field::Timestamp => r.timestamp.as_ref(),
                Field::Pubdate => r.pubdate.as_ref(),
                _ => r.last_modified.as_ref(),
            };
            q.matches(v)
        }
        Leaf::Pair(p) => p.matches(r),
        Leaf::Any(list) => list.iter().any(|x| eval_leaf(x, c)),
    }
}

#[cfg(test)]
mod tests;
