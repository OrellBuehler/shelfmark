use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, Row, params_from_iter};
use shelfmark_model::{Book, BookSummary, FormatFile, Metadata};

use crate::dates;
use crate::db;
use crate::error::Result;

#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct FormatRec {
    pub format: String,
    pub size: u64,
    pub name: String,
}

#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub id: i64,
    pub title: String,
    pub sort: String,
    pub authors: Vec<String>,
    pub author_sort: String,
    pub series: Option<String>,
    pub series_index: f64,
    pub tags: Vec<String>,
    pub publisher: Option<String>,
    pub languages: Vec<String>,
    pub rating: u8,
    pub formats: Vec<FormatRec>,
    pub identifiers: Vec<(String, String)>,
    pub has_cover: bool,
    pub size: u64,
    pub timestamp: Option<DateTime<Utc>>,
    pub pubdate: Option<DateTime<Utc>>,
    pub last_modified: Option<DateTime<Utc>>,
    pub timestamp_raw: String,
    pub last_modified_raw: String,
    pub path: String,
    pub uuid: String,
    pub comments: Option<String>,
    pub pages: Option<i64>,
}

fn each(
    conn: &Connection,
    sql: &str,
    one: Option<i64>,
    mut f: impl FnMut(&Row) -> rusqlite::Result<()>,
) -> Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params_from_iter(one.iter()))?;
    while let Some(row) = rows.next()? {
        f(row)?;
    }
    Ok(())
}

pub fn load(conn: &Connection, one: Option<i64>) -> Result<Vec<Record>> {
    let w = |col: &str| {
        if one.is_some() {
            format!(" WHERE {col}=?1")
        } else {
            String::new()
        }
    };
    let mut recs: Vec<Record> = Vec::new();
    let mut index: HashMap<i64, usize> = HashMap::new();

    each(
        conn,
        &format!(
            "SELECT id, title, sort, timestamp, pubdate, series_index, author_sort, path, uuid, has_cover, last_modified FROM books{} ORDER BY id",
            w("id")
        ),
        one,
        |r| {
            let timestamp_raw: Option<String> = r.get(3)?;
            let pubdate_raw: Option<String> = r.get(4)?;
            let modified_raw: Option<String> = r.get(10)?;
            let timestamp_raw = timestamp_raw.unwrap_or_default();
            let last_modified_raw = modified_raw.unwrap_or_default();
            let title: String = r.get(1)?;
            let sort: Option<String> = r.get(2)?;
            let rec = Record {
                id: r.get(0)?,
                sort: sort.unwrap_or_else(|| title.clone()),
                title,
                series_index: r.get::<_, Option<f64>>(5)?.unwrap_or(1.0),
                author_sort: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                path: r.get(7)?,
                uuid: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
                has_cover: r.get::<_, Option<i64>>(9)?.unwrap_or(0) != 0,
                timestamp: dates::parse(&timestamp_raw),
                pubdate: pubdate_raw
                    .as_deref()
                    .and_then(dates::parse)
                    .filter(|d| !dates::is_undefined(d)),
                last_modified: dates::parse(&last_modified_raw),
                timestamp_raw,
                last_modified_raw,
                ..Default::default()
            };
            index.insert(rec.id, recs.len());
            recs.push(rec);
            Ok(())
        },
    )?;

    each(
        conn,
        &format!(
            "SELECT l.book, a.name FROM books_authors_link l JOIN authors a ON a.id=l.author{} ORDER BY l.id",
            w("l.book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                recs[i]
                    .authors
                    .push(r.get::<_, String>(1)?.replace('|', ","));
            }
            Ok(())
        },
    )?;
    each(
        conn,
        &format!(
            "SELECT l.book, t.name FROM books_tags_link l JOIN tags t ON t.id=l.tag{} ORDER BY l.id",
            w("l.book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                recs[i].tags.push(r.get(1)?);
            }
            Ok(())
        },
    )?;
    each(
        conn,
        &format!(
            "SELECT l.book, s.name FROM books_series_link l JOIN series s ON s.id=l.series{} ORDER BY l.id",
            w("l.book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                recs[i].series = Some(r.get(1)?);
            }
            Ok(())
        },
    )?;
    each(
        conn,
        &format!(
            "SELECT l.book, p.name FROM books_publishers_link l JOIN publishers p ON p.id=l.publisher{} ORDER BY l.id",
            w("l.book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                recs[i].publisher = Some(r.get(1)?);
            }
            Ok(())
        },
    )?;
    each(
        conn,
        &format!(
            "SELECT l.book, r.rating FROM books_ratings_link l JOIN ratings r ON r.id=l.rating{} ORDER BY l.id",
            w("l.book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                let v: Option<i64> = r.get(1)?;
                recs[i].rating = v.unwrap_or(0).clamp(0, 10) as u8;
            }
            Ok(())
        },
    )?;
    each(
        conn,
        &format!(
            "SELECT l.book, g.lang_code FROM books_languages_link l JOIN languages g ON g.id=l.lang_code{} ORDER BY l.id",
            w("l.book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                recs[i].languages.push(r.get(1)?);
            }
            Ok(())
        },
    )?;
    each(
        conn,
        &format!(
            "SELECT book, format, uncompressed_size, name FROM data{} ORDER BY id",
            w("book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                let size = r.get::<_, i64>(2)?.max(0) as u64;
                let rec = &mut recs[i];
                rec.size = rec.size.max(size);
                rec.formats.push(FormatRec {
                    format: r.get::<_, String>(1)?.to_uppercase(),
                    size,
                    name: r.get(3)?,
                });
            }
            Ok(())
        },
    )?;
    each(
        conn,
        &format!(
            "SELECT book, type, val FROM identifiers{} ORDER BY id",
            w("book")
        ),
        one,
        |r| {
            if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                recs[i].identifiers.push((r.get(1)?, r.get(2)?));
            }
            Ok(())
        },
    )?;
    if one.is_some() {
        each(
            conn,
            &format!("SELECT book, text FROM comments{}", w("book")),
            one,
            |r| {
                if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                    recs[i].comments = Some(r.get(1)?);
                }
                Ok(())
            },
        )?;
    }
    if one.is_none() && db::has_table(conn, "books_pages_link") {
        each(
            conn,
            &format!("SELECT book, pages FROM books_pages_link{}", w("book")),
            one,
            |r| {
                if let Some(&i) = index.get(&r.get::<_, i64>(0)?) {
                    let p: i64 = r.get(1)?;
                    if p > 0 {
                        recs[i].pages = Some(p);
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(recs)
}

pub fn load_comments(conn: &Connection) -> Result<HashMap<i64, String>> {
    let mut map = HashMap::new();
    each(conn, "SELECT book, text FROM comments", None, |r| {
        map.insert(r.get(0)?, r.get(1)?);
        Ok(())
    })?;
    Ok(map)
}

fn rating_opt(r: u8) -> Option<u8> {
    if r == 0 { None } else { Some(r) }
}

impl Record {
    pub fn summary(&self) -> BookSummary {
        BookSummary {
            id: self.id,
            title: self.title.clone(),
            sort: self.sort.clone(),
            authors: self.authors.clone(),
            author_sort: self.author_sort.clone(),
            series: self.series.clone(),
            series_index: self.series_index,
            tags: self.tags.clone(),
            publisher: self.publisher.clone(),
            languages: self.languages.clone(),
            rating: rating_opt(self.rating),
            formats: self.formats.iter().map(|f| f.format.clone()).collect(),
            identifiers: self.identifiers.iter().cloned().collect::<BTreeMap<_, _>>(),
            has_cover: self.has_cover,
            size: self.size,
            timestamp: self
                .timestamp
                .as_ref()
                .map_or_else(|| self.timestamp_raw.replacen(' ', "T", 1), dates::iso),
            pubdate: self.pubdate.as_ref().map(dates::iso),
            last_modified: self
                .last_modified
                .as_ref()
                .map_or_else(|| self.last_modified_raw.replacen(' ', "T", 1), dates::iso),
            ..Default::default()
        }
    }

    pub fn book(&self) -> Book {
        Book {
            summary: self.summary(),
            comments: self.comments.clone(),
            uuid: self.uuid.clone(),
            path: self.path.clone(),
            files: self
                .formats
                .iter()
                .map(|f| FormatFile {
                    format: f.format.clone(),
                    file_name: format!("{}.{}", f.name, f.format.to_lowercase()),
                    size: f.size,
                })
                .collect(),
        }
    }

    pub fn metadata(&self) -> Metadata {
        Metadata {
            title: Some(self.title.clone()),
            title_sort: Some(self.sort.clone()),
            authors: self.authors.clone(),
            author_sort: Some(self.author_sort.clone()),
            series: self.series.clone(),
            series_index: Some(self.series_index),
            tags: self.tags.clone(),
            publisher: self.publisher.clone(),
            pubdate: self.pubdate.as_ref().map(dates::iso),
            languages: self.languages.clone(),
            identifiers: self.identifiers.iter().cloned().collect(),
            rating: rating_opt(self.rating),
            comments: self.comments.clone(),
            uuid: Some(self.uuid.clone()),
        }
    }
}
