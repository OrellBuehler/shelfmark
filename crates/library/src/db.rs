use std::path::Path;
use std::time::Duration;

use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use rusqlite::{Connection, OpenFlags};

use crate::error::Result;
use crate::text;

pub const MIN_USER_VERSION: i64 = 27;
pub const SCHEMA: &str = include_str!("schema.sql");

struct SortedConcat {
    sep: &'static str,
}

impl Aggregate<std::collections::BTreeMap<i64, String>, Option<String>> for SortedConcat {
    fn init(
        &self,
        _: &mut Context<'_>,
    ) -> rusqlite::Result<std::collections::BTreeMap<i64, String>> {
        Ok(Default::default())
    }

    fn step(
        &self,
        ctx: &mut Context<'_>,
        acc: &mut std::collections::BTreeMap<i64, String>,
    ) -> rusqlite::Result<()> {
        let ndx: i64 = ctx.get(0)?;
        if let Some(v) = ctx.get::<Option<String>>(1)? {
            acc.insert(ndx, v);
        }
        Ok(())
    }

    fn finalize(
        &self,
        _: &mut Context<'_>,
        acc: Option<std::collections::BTreeMap<i64, String>>,
    ) -> rusqlite::Result<Option<String>> {
        Ok(acc
            .filter(|m| !m.is_empty())
            .map(|m| m.into_values().collect::<Vec<_>>().join(self.sep)))
    }
}

struct Concat;

impl Aggregate<Vec<String>, Option<String>> for Concat {
    fn init(&self, _: &mut Context<'_>) -> rusqlite::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Vec<String>) -> rusqlite::Result<()> {
        if let Some(v) = ctx.get::<Option<String>>(0)? {
            acc.push(v);
        }
        Ok(())
    }

    fn finalize(
        &self,
        _: &mut Context<'_>,
        acc: Option<Vec<String>>,
    ) -> rusqlite::Result<Option<String>> {
        Ok(acc.filter(|v| !v.is_empty()).map(|v| v.join(",")))
    }
}

struct IdentifiersConcat;

impl Aggregate<Vec<String>, String> for IdentifiersConcat {
    fn init(&self, _: &mut Context<'_>) -> rusqlite::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Vec<String>) -> rusqlite::Result<()> {
        let k: String = ctx.get(0)?;
        let v: String = ctx.get(1)?;
        acc.push(format!("{k}:{v}"));
        Ok(())
    }

    fn finalize(&self, _: &mut Context<'_>, acc: Option<Vec<String>>) -> rusqlite::Result<String> {
        Ok(acc.unwrap_or_default().join(","))
    }
}

struct AumSortedConcat;

impl Aggregate<std::collections::BTreeMap<i64, String>, Option<String>> for AumSortedConcat {
    fn init(
        &self,
        _: &mut Context<'_>,
    ) -> rusqlite::Result<std::collections::BTreeMap<i64, String>> {
        Ok(Default::default())
    }

    fn step(
        &self,
        ctx: &mut Context<'_>,
        acc: &mut std::collections::BTreeMap<i64, String>,
    ) -> rusqlite::Result<()> {
        let ndx: i64 = ctx.get(0)?;
        if let Some(author) = ctx.get::<Option<String>>(1)? {
            let sort: String = ctx.get::<Option<String>>(2)?.unwrap_or_default();
            let link: String = ctx.get::<Option<String>>(3)?.unwrap_or_default();
            acc.insert(ndx, format!("{author}:::{sort}:::{link}"));
        }
        Ok(())
    }

    fn finalize(
        &self,
        _: &mut Context<'_>,
        acc: Option<std::collections::BTreeMap<i64, String>>,
    ) -> rusqlite::Result<Option<String>> {
        Ok(acc
            .filter(|m| !m.is_empty())
            .map(|m| m.into_values().collect::<Vec<_>>().join(":#:")))
    }
}

fn register(conn: &Connection) -> rusqlite::Result<()> {
    let det = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    conn.create_scalar_function("title_sort", -1, det, |ctx| {
        let t: Option<String> = ctx.get(0)?;
        let lang: Option<String> = if ctx.len() > 2 { ctx.get(2)? } else { None };
        Ok(text::title_sort(&t.unwrap_or_default(), lang.as_deref()))
    })?;
    conn.create_scalar_function("author_to_author_sort", 1, det, |ctx| {
        let a: Option<String> = ctx.get(0)?;
        Ok(text::author_to_author_sort(
            &a.unwrap_or_default().replace('|', ","),
        ))
    })?;
    conn.create_scalar_function("uuid4", 0, FunctionFlags::SQLITE_UTF8, |_| {
        Ok(uuid::Uuid::new_v4().to_string())
    })?;
    conn.create_scalar_function("books_list_filter", 1, det, |_| Ok(1i64))?;
    conn.create_aggregate_function(
        "sortconcat",
        2,
        FunctionFlags::SQLITE_UTF8,
        SortedConcat { sep: "," },
    )?;
    conn.create_aggregate_function(
        "sortconcat_bar",
        2,
        FunctionFlags::SQLITE_UTF8,
        SortedConcat { sep: "|" },
    )?;
    conn.create_aggregate_function(
        "sortconcat_amper",
        2,
        FunctionFlags::SQLITE_UTF8,
        SortedConcat { sep: "&" },
    )?;
    conn.create_aggregate_function(
        "identifiers_concat",
        2,
        FunctionFlags::SQLITE_UTF8,
        IdentifiersConcat,
    )?;
    conn.create_aggregate_function("concat", 1, FunctionFlags::SQLITE_UTF8, Concat)?;
    conn.create_aggregate_function(
        "aum_sortconcat",
        4,
        FunctionFlags::SQLITE_UTF8,
        AumSortedConcat,
    )?;
    conn.create_collation("PYNOCASE", |a, b| a.to_lowercase().cmp(&b.to_lowercase()))?;
    conn.create_collation("icucollate", |a, b| {
        text::sort_key(a).cmp(&text::sort_key(b))
    })?;
    Ok(())
}

fn pragmas(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_millis(10_000))?;
    conn.execute_batch("PRAGMA cache_size=-5000; PRAGMA temp_store=2; PRAGMA foreign_keys=ON;")?;
    register(conn)?;
    Ok(())
}

fn temp_triggers(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS author_insert_trg;
         CREATE TEMP TRIGGER author_insert_trg AFTER INSERT ON authors
         BEGIN
             UPDATE authors SET sort=author_to_author_sort(NEW.name) WHERE id=NEW.id;
         END;
         DROP TRIGGER IF EXISTS author_update_trg;
         CREATE TEMP TRIGGER author_update_trg BEFORE UPDATE ON authors
         BEGIN
             UPDATE authors SET sort=author_to_author_sort(NEW.name) WHERE id=NEW.id AND name <> NEW.name;
         END;",
    )?;
    Ok(())
}

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    pragmas(&conn)?;
    let uv = user_version(&conn)?;
    if uv < MIN_USER_VERSION {
        return Err(crate::error::Error::UnsupportedSchema(uv));
    }
    temp_triggers(&conn)?;
    Ok(conn)
}

pub fn create(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    pragmas(&conn)?;
    conn.execute_batch(SCHEMA)?;
    conn.execute(
        "INSERT INTO library_id(uuid) VALUES (?1)",
        [uuid::Uuid::new_v4().to_string()],
    )?;
    temp_triggers(&conn)?;
    Ok(conn)
}

pub fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

pub fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [name],
        |_| Ok(()),
    )
    .is_ok()
}
