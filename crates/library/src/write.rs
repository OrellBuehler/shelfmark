use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, params};
use shelfmark_model::{BookId, BookPatch};

use crate::dates;
use crate::error::{Error, Result};
use crate::langs;
use crate::text;

pub struct Spec {
    pub table: &'static str,
    pub col: &'static str,
    pub link: &'static str,
    pub link_col: &'static str,
}

pub static AUTHORS: Spec = Spec {
    table: "authors",
    col: "name",
    link: "books_authors_link",
    link_col: "author",
};
pub static TAGS: Spec = Spec {
    table: "tags",
    col: "name",
    link: "books_tags_link",
    link_col: "tag",
};
pub static SERIES: Spec = Spec {
    table: "series",
    col: "name",
    link: "books_series_link",
    link_col: "series",
};
pub static PUBLISHERS: Spec = Spec {
    table: "publishers",
    col: "name",
    link: "books_publishers_link",
    link_col: "publisher",
};
pub static LANGUAGES: Spec = Spec {
    table: "languages",
    col: "lang_code",
    link: "books_languages_link",
    link_col: "lang_code",
};

#[derive(Default)]
struct Items {
    by_lower: HashMap<String, i64>,
    names: HashMap<i64, String>,
}

pub struct Writer<'a> {
    pub conn: &'a Connection,
    items: HashMap<&'static str, Items>,
    pub dirtied: HashSet<i64>,
}

fn db_name(spec: &Spec, name: &str) -> String {
    if spec.table == "authors" {
        name.replace(',', "|")
    } else {
        name.to_string()
    }
}

fn mem_name(spec: &Spec, name: String) -> String {
    if spec.table == "authors" {
        name.replace('|', ",")
    } else {
        name
    }
}

pub fn single_text(s: &str) -> String {
    text::normalize_whitespace(s.trim())
}

pub fn clean_list(list: &[String], sep_replace: Option<(char, char)>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in list {
        let mut y = item.trim().to_string();
        if let Some((from, to)) = sep_replace {
            y = y.replace(from, &to.to_string());
        }
        let y = text::normalize_whitespace(&y);
        if y.is_empty() {
            continue;
        }
        if seen.insert(y.to_lowercase()) {
            out.push(y);
        }
    }
    out
}

pub fn clean_authors(list: &[String]) -> Vec<String> {
    let cleaned = clean_list(list, Some(('&', ';')));
    if cleaned.is_empty() {
        vec!["Unknown".to_string()]
    } else {
        cleaned
    }
}

pub fn clean_tags(list: &[String]) -> Vec<String> {
    clean_list(list, Some((',', ';')))
}

pub fn clean_languages(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in list {
        if let Some(code) = langs::clean_code(l)
            && !out.contains(&code)
        {
            out.push(code);
        }
    }
    out
}

pub fn clean_identifiers(map: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, v) in map {
        let k = k.to_lowercase().trim().replace([':', ','], "");
        let v = v.trim().replace(',', "|");
        if !k.is_empty() && !v.is_empty() {
            out.insert(k, v);
        }
    }
    out
}

pub fn parse_pubdate(s: &str) -> Result<String> {
    if s.trim().is_empty() {
        return Ok(dates::UNDEFINED.to_string());
    }
    match dates::parse(s) {
        Some(dt) if dates::is_undefined(&dt) => Ok(dates::UNDEFINED.to_string()),
        Some(dt) => Ok(dates::format_db(&dt)),
        None => Err(Error::Invalid {
            field: "pubdate",
            value: s.to_string(),
        }),
    }
}

impl<'a> Writer<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Writer {
            conn,
            items: HashMap::new(),
            dirtied: HashSet::new(),
        }
    }

    fn items(&mut self, spec: &Spec) -> Result<&mut Items> {
        if !self.items.contains_key(spec.table) {
            let mut it = Items::default();
            let mut stmt = self
                .conn
                .prepare(&format!("SELECT id, {} FROM {}", spec.col, spec.table))?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (id, name) = row?;
                let name = mem_name(spec, name);
                it.by_lower.insert(name.to_lowercase(), id);
                it.names.insert(id, name);
            }
            self.items.insert(spec.table, it);
        }
        Ok(self.items.get_mut(spec.table).unwrap())
    }

    fn get_or_create(&mut self, spec: &Spec, name: &str, allow_case_change: bool) -> Result<i64> {
        let key = name.to_lowercase();
        let existing = {
            let items = self.items(spec)?;
            items
                .by_lower
                .get(&key)
                .map(|id| (*id, items.names[id].clone()))
        };
        let conn = self.conn;
        if let Some((id, current)) = existing {
            if allow_case_change && current != name {
                conn.execute(
                    &format!("UPDATE {} SET {}=?1 WHERE id=?2", spec.table, spec.col),
                    params![db_name(spec, name), id],
                )?;
                self.items(spec)?.names.insert(id, name.to_string());
                let books: Vec<i64> = {
                    let mut stmt = conn.prepare(&format!(
                        "SELECT book FROM {} WHERE {}=?1",
                        spec.link, spec.link_col
                    ))?;
                    stmt.query_map([id], |r| r.get(0))?
                        .collect::<rusqlite::Result<_>>()?
                };
                if spec.table == "authors" {
                    for b in &books {
                        self.refresh_author_sort_case(*b)?;
                    }
                }
                self.dirtied.extend(books);
            }
            return Ok(id);
        }
        if spec.table == "authors" {
            self.conn.execute(
                "INSERT INTO authors(name, sort) VALUES (?1, ?2)",
                params![db_name(spec, name), text::author_to_author_sort(name)],
            )?;
        } else {
            self.conn.execute(
                &format!("INSERT INTO {}({}) VALUES (?1)", spec.table, spec.col),
                [name],
            )?;
        }
        let id = self.conn.last_insert_rowid();
        let items = self.items(spec)?;
        items.by_lower.insert(key, id);
        items.names.insert(id, name.to_string());
        Ok(id)
    }

    fn refresh_author_sort_case(&mut self, book: i64) -> Result<()> {
        let current: Option<String> = self
            .conn
            .query_row("SELECT author_sort FROM books WHERE id=?1", [book], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        let computed = self.author_sort_for_book(book)?;
        if let Some(cur) = current
            && cur != computed
            && cur.to_lowercase() == computed.to_lowercase()
        {
            self.conn.execute(
                "UPDATE books SET author_sort=?1 WHERE id=?2",
                params![computed, book],
            )?;
        }
        Ok(())
    }

    pub fn author_sort_for_book(&self, book: i64) -> Result<String> {
        let mut stmt = self.conn.prepare(
            "SELECT a.name, a.sort FROM books_authors_link l JOIN authors a ON a.id=l.author WHERE l.book=?1 ORDER BY l.id",
        )?;
        let rows = stmt.query_map([book], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?;
        let mut parts = Vec::new();
        for row in rows {
            let (name, sort) = row?;
            parts.push(
                sort.filter(|s| !s.is_empty())
                    .unwrap_or_else(|| text::author_to_author_sort(&name.replace('|', ","))),
            );
        }
        Ok(parts.join(" & "))
    }

    pub fn author_sort_from_names(&mut self, names: &[String]) -> Result<String> {
        let mut parts = Vec::new();
        for n in names {
            let id = self
                .items(&AUTHORS)?
                .by_lower
                .get(&n.to_lowercase())
                .copied();
            let sort: Option<String> = match id {
                Some(id) => {
                    self.conn
                        .query_row("SELECT sort FROM authors WHERE id=?1", [id], |r| r.get(0))?
                }
                None => None,
            };
            let sort = sort
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| text::author_to_author_sort(n));
            if !sort.is_empty() {
                parts.push(sort);
            }
        }
        Ok(parts.join(" & "))
    }

    pub fn prune(&mut self, spec: &Spec, candidates: &[i64]) -> Result<()> {
        for id in candidates {
            let used: bool = self
                .conn
                .query_row(
                    &format!(
                        "SELECT 1 FROM {} WHERE {}=?1 LIMIT 1",
                        spec.link, spec.link_col
                    ),
                    [id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !used {
                self.conn
                    .execute(&format!("DELETE FROM {} WHERE id=?1", spec.table), [id])?;
                if let Some(items) = self.items.get_mut(spec.table)
                    && let Some(name) = items.names.remove(id)
                {
                    items.by_lower.remove(&name.to_lowercase());
                }
            }
        }
        Ok(())
    }

    pub fn linked_items(&self, book: i64) -> Result<Vec<(&'static Spec, Vec<i64>)>> {
        let mut out = Vec::new();
        for spec in [&AUTHORS, &TAGS, &SERIES, &PUBLISHERS, &LANGUAGES] {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {} FROM {} WHERE book=?1",
                spec.link_col, spec.link
            ))?;
            let ids: Vec<i64> = stmt
                .query_map([book], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            out.push((spec, ids));
        }
        Ok(out)
    }

    pub fn prune_all(&mut self, items: &[(&'static Spec, Vec<i64>)]) -> Result<()> {
        for (spec, ids) in items {
            self.prune(spec, ids)?;
        }
        self.conn.execute(
            "DELETE FROM ratings WHERE id NOT IN (SELECT rating FROM books_ratings_link)",
            [],
        )?;
        Ok(())
    }

    pub fn set_many(
        &mut self,
        spec: &Spec,
        book: i64,
        names: &[String],
        allow_case_change: bool,
    ) -> Result<bool> {
        let old: Vec<i64> = {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {} FROM {} WHERE book=?1 ORDER BY id",
                spec.link_col, spec.link
            ))?;
            stmt.query_map([book], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut ids = Vec::new();
        for n in names {
            let id = self.get_or_create(spec, n, allow_case_change)?;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids == old {
            return Ok(false);
        }
        self.conn
            .execute(&format!("DELETE FROM {} WHERE book=?1", spec.link), [book])?;
        for id in &ids {
            self.conn.execute(
                &format!(
                    "INSERT INTO {}(book, {}) VALUES (?1, ?2)",
                    spec.link, spec.link_col
                ),
                params![book, id],
            )?;
        }
        let removed: Vec<i64> = old.into_iter().filter(|o| !ids.contains(o)).collect();
        self.prune(spec, &removed)?;
        self.dirtied.insert(book);
        Ok(true)
    }

    pub fn set_one(
        &mut self,
        spec: &Spec,
        book: i64,
        name: Option<&str>,
        allow_case_change: bool,
    ) -> Result<bool> {
        let names: Vec<String> = name
            .filter(|n| !n.is_empty())
            .map(|n| vec![n.to_string()])
            .unwrap_or_default();
        self.set_many(spec, book, &names, allow_case_change)
    }

    pub fn set_rating(&mut self, book: i64, rating: u8) -> Result<()> {
        let rating = rating.min(10);
        let old: Option<i64> = self
            .conn
            .query_row("SELECT r.rating FROM books_ratings_link l JOIN ratings r ON r.id=l.rating WHERE l.book=?1", [book], |r| r.get(0))
            .optional()?;
        if old.unwrap_or(0) == i64::from(rating) {
            return Ok(());
        }
        self.conn
            .execute("DELETE FROM books_ratings_link WHERE book=?1", [book])?;
        if rating > 0 {
            self.conn.execute(
                "INSERT OR IGNORE INTO ratings(rating) VALUES (?1)",
                [rating],
            )?;
            let id: i64 =
                self.conn
                    .query_row("SELECT id FROM ratings WHERE rating=?1", [rating], |r| {
                        r.get(0)
                    })?;
            self.conn.execute(
                "INSERT INTO books_ratings_link(book, rating) VALUES (?1, ?2)",
                params![book, id],
            )?;
        }
        self.conn.execute(
            "DELETE FROM ratings WHERE id NOT IN (SELECT rating FROM books_ratings_link)",
            [],
        )?;
        self.dirtied.insert(book);
        Ok(())
    }

    pub fn set_authors(
        &mut self,
        book: i64,
        authors: &[String],
        allow_case_change: bool,
    ) -> Result<()> {
        self.set_many(&AUTHORS, book, authors, allow_case_change)?;
        let sort = self.author_sort_for_book(book)?;
        self.conn.execute(
            "UPDATE books SET author_sort=?1 WHERE id=?2",
            params![sort, book],
        )?;
        self.dirtied.insert(book);
        Ok(())
    }

    pub fn set_title(&mut self, book: i64, title: &str) -> Result<()> {
        let title = single_text(title);
        let title = if title.is_empty() {
            "Unknown".to_string()
        } else {
            title
        };
        self.conn.execute(
            "UPDATE books SET title=?1 WHERE id=?2",
            params![title, book],
        )?;
        self.conn.execute(
            "UPDATE books SET sort=?1 WHERE id=?2",
            params![text::title_sort(&title, None), book],
        )?;
        self.dirtied.insert(book);
        Ok(())
    }

    pub fn set_comments(&mut self, book: i64, comments: &str) -> Result<()> {
        match text::long_text(comments) {
            Some(c) => self.conn.execute(
                "INSERT OR REPLACE INTO comments(book, text) VALUES (?1, ?2)",
                params![book, c],
            )?,
            None => self
                .conn
                .execute("DELETE FROM comments WHERE book=?1", [book])?,
        };
        self.dirtied.insert(book);
        Ok(())
    }

    pub fn set_identifiers(&mut self, book: i64, ids: &BTreeMap<String, String>) -> Result<()> {
        let clean = clean_identifiers(ids);
        self.conn
            .execute("DELETE FROM identifiers WHERE book=?1", [book])?;
        for (k, v) in clean {
            self.conn.execute(
                "INSERT OR REPLACE INTO identifiers(book, type, val) VALUES (?1, ?2, ?3)",
                params![book, k, v],
            )?;
        }
        self.dirtied.insert(book);
        Ok(())
    }

    pub fn set_pubdate(&mut self, book: i64, value: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET pubdate=?1 WHERE id=?2",
            params![parse_pubdate(value)?, book],
        )?;
        self.dirtied.insert(book);
        Ok(())
    }

    pub fn set_series_index(&mut self, book: i64, index: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET series_index=?1 WHERE id=?2",
            params![index, book],
        )?;
        self.dirtied.insert(book);
        Ok(())
    }

    pub fn current_title(&self, book: i64) -> Result<String> {
        Ok(self
            .conn
            .query_row("SELECT title FROM books WHERE id=?1", [book], |r| r.get(0))?)
    }

    pub fn apply_patch(&mut self, id: BookId, p: &BookPatch) -> Result<bool> {
        let mut path_changed = false;
        if let Some(t) = &p.title {
            self.set_title(id, t)?;
            path_changed = true;
        }
        if let Some(a) = &p.authors {
            self.set_authors(id, &clean_authors(a), true)?;
            path_changed = true;
        }
        if let Some(r) = p.rating {
            self.set_rating(id, r)?;
        }
        if let Some(i) = p.series_index {
            self.set_series_index(id, i)?;
        }
        if let Some(s) = &p.author_sort {
            let s = s.trim();
            let sort = if s.is_empty() {
                self.author_sort_for_book(id)?
            } else {
                s.to_string()
            };
            self.conn.execute(
                "UPDATE books SET author_sort=?1 WHERE id=?2",
                params![sort, id],
            )?;
            self.dirtied.insert(id);
        }
        if let Some(s) = &p.publisher {
            self.set_one(&PUBLISHERS, id, Some(&single_text(s)), true)?;
        }
        if let Some(s) = &p.series {
            self.set_one(&SERIES, id, Some(&single_text(s)), true)?;
        }
        if let Some(t) = &p.tags {
            self.set_many(&TAGS, id, &clean_tags(t), true)?;
        }
        if let Some(c) = &p.comments {
            self.set_comments(id, c)?;
        }
        if let Some(l) = &p.languages {
            self.set_many(&LANGUAGES, id, &clean_languages(l), true)?;
        }
        if let Some(d) = &p.pubdate {
            self.set_pubdate(id, d)?;
        }
        if let Some(s) = &p.title_sort {
            let s = s.trim();
            let sort = if s.is_empty() {
                text::title_sort(&self.current_title(id)?, None)
            } else {
                s.to_string()
            };
            self.conn
                .execute("UPDATE books SET sort=?1 WHERE id=?2", params![sort, id])?;
            self.dirtied.insert(id);
        }
        if let Some(i) = &p.identifiers {
            self.set_identifiers(id, i)?;
        }
        Ok(path_changed)
    }

    pub fn finish(&mut self, books: impl IntoIterator<Item = i64>) -> Result<()> {
        let now = dates::now_db();
        let mut all: HashSet<i64> = std::mem::take(&mut self.dirtied);
        all.extend(books);
        for b in all {
            self.conn.execute(
                "UPDATE books SET last_modified=?1 WHERE id=?2",
                params![now, b],
            )?;
            self.conn.execute(
                "INSERT OR IGNORE INTO metadata_dirtied(book) VALUES (?1)",
                [b],
            )?;
        }
        Ok(())
    }
}

pub fn next_series_index(conn: &Connection, series: &str) -> Result<f64> {
    let mut stmt = conn.prepare(
        "SELECT b.series_index FROM books b JOIN books_series_link l ON l.book=b.id JOIN series s ON s.id=l.series WHERE s.name=?1 COLLATE NOCASE",
    )?;
    let max = stmt
        .query_map([series], |r| r.get::<_, f64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |x| x.max(v))));
    Ok(max.map_or(1.0, |m| m.floor() + 1.0))
}
