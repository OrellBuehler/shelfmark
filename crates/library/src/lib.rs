mod catalog;
mod categories;
mod dates;
mod db;
mod error;
mod fsops;
mod langs;
mod record;
pub mod search;
mod text;
mod write;

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, params};
use shelfmark_model::{
    AudioIndex, AudiobookId, Book, BookId, BookPatch, BookSummary, BulkPatch, Category, ListQuery,
    Metadata, SavedSearch, Scope, SortField,
};

pub use error::{Error, Result};

use fsops::Ctx;
use record::Record;
use write::Writer;

type CommentMap = Arc<HashMap<i64, String>>;

struct Cache {
    version: i64,
    recs: Arc<Vec<Record>>,
    comments: Option<CommentMap>,
}

struct CatalogCache {
    recs: usize,
    generation: u64,
    built: Arc<catalog::Built>,
}

pub struct Library {
    dir: PathBuf,
    conn: Mutex<Connection>,
    cache: Mutex<Option<Cache>>,
    catalog: Mutex<Option<CatalogCache>>,
    rules: text::PathRules,
}

struct Invalidate<'a>(&'a Library);

impl Drop for Invalidate<'_> {
    fn drop(&mut self) {
        *self.0.cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

fn escape_json_ascii(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for c in json.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut buf = [0u16; 2];
            for u in c.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{u:04x}"));
            }
        }
    }
    out
}

fn saved_searches_map(conn: &Connection) -> Result<BTreeMap<String, String>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT val FROM preferences WHERE key='saved_searches'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(raw) = raw else {
        return Ok(BTreeMap::new());
    };
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    let mut map = BTreeMap::new();
    if let serde_json::Value::Object(o) = value {
        for (k, v) in o {
            if let serde_json::Value::String(q) = v {
                map.insert(k, q);
            }
        }
    }
    Ok(map)
}

fn compile_search(conn: &Connection, query: &str) -> Result<search::Compiled> {
    let saved: HashMap<String, String> = saved_searches_map(conn)?
        .into_iter()
        .map(|(k, v)| (k.to_lowercase(), v))
        .collect();
    search::compile(query, &|name| saved.get(&name.to_lowercase()).cloned())
}

fn nonblank(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

struct SortKey {
    missing: bool,
    text: String,
    num: f64,
}

fn sort_records(recs: Vec<&Record>, by: SortField, descending: bool) -> Vec<&Record> {
    let micros = |d: &Option<chrono::DateTime<chrono::Utc>>| d.map(|d| d.timestamp_micros() as f64);
    let keys: Vec<(SortKey, String)> = recs
        .iter()
        .map(|r| {
            let key = match by {
                SortField::Title => SortKey {
                    missing: false,
                    text: text::sort_key(&r.sort),
                    num: 0.0,
                },
                SortField::Author => SortKey {
                    missing: false,
                    text: text::sort_key(&r.author_sort),
                    num: 0.0,
                },
                SortField::Series => SortKey {
                    missing: r.series.is_none(),
                    text: r
                        .series
                        .as_deref()
                        .map(|s| {
                            text::sort_key(&text::title_sort(
                                s,
                                r.languages.first().map(String::as_str),
                            ))
                        })
                        .unwrap_or_default(),
                    num: r.series_index,
                },
                SortField::Added => SortKey {
                    missing: r.timestamp.is_none(),
                    text: String::new(),
                    num: micros(&r.timestamp).unwrap_or(0.0),
                },
                SortField::Published => SortKey {
                    missing: r.pubdate.is_none(),
                    text: String::new(),
                    num: micros(&r.pubdate).unwrap_or(0.0),
                },
                SortField::Modified => SortKey {
                    missing: r.last_modified.is_none(),
                    text: String::new(),
                    num: micros(&r.last_modified).unwrap_or(0.0),
                },
                SortField::Rating => SortKey {
                    missing: false,
                    text: String::new(),
                    num: f64::from(r.rating),
                },
                SortField::Size => SortKey {
                    missing: false,
                    text: String::new(),
                    num: r.size as f64,
                },
                SortField::Duration => SortKey {
                    missing: r.audio.is_none(),
                    text: String::new(),
                    num: r.audio.as_ref().map_or(0.0, |a| a.duration_ms as f64),
                },
                SortField::Publisher => SortKey {
                    missing: r.publisher.is_none(),
                    text: r
                        .publisher
                        .as_deref()
                        .map(text::sort_key)
                        .unwrap_or_default(),
                    num: 0.0,
                },
            };
            (key, text::sort_key(&r.sort))
        })
        .collect();
    let mut order: Vec<usize> = (0..recs.len()).collect();
    order.sort_by(|&a, &b| {
        let (ka, ta) = &keys[a];
        let (kb, tb) = &keys[b];
        if ka.missing != kb.missing {
            return ka.missing.cmp(&kb.missing);
        }
        let ord = ka
            .text
            .cmp(&kb.text)
            .then_with(|| ka.num.total_cmp(&kb.num));
        let ord = if descending { ord.reverse() } else { ord };
        ord.then_with(|| ta.cmp(tb)).then(Ordering::Equal)
    });
    order.into_iter().map(|i| recs[i]).collect()
}

impl Library {
    pub fn open(dir: &Path) -> Result<Self> {
        let db_path = dir.join("metadata.db");
        if !db_path.is_file() {
            return Err(Error::NotALibrary(dir.to_path_buf()));
        }
        let conn = db::open(&db_path)?;
        Ok(Library {
            dir: dir.to_path_buf(),
            conn: Mutex::new(conn),
            cache: Mutex::new(None),
            catalog: Mutex::new(None),
            rules: text::PathRules::default(),
        })
    }

    pub fn create(dir: &Path) -> Result<Self> {
        let db_path = dir.join("metadata.db");
        if db_path.exists() {
            return Err(Error::AlreadyExists(dir.to_path_buf()));
        }
        fs::create_dir_all(dir)?;
        let conn = match db::create(&db_path) {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&db_path);
                return Err(e);
            }
        };
        Ok(Library {
            dir: dir.to_path_buf(),
            conn: Mutex::new(conn),
            cache: Mutex::new(None),
            catalog: Mutex::new(None),
            rules: text::PathRules::default(),
        })
    }

    pub fn with_windows_path_rules(mut self) -> Self {
        self.rules = text::PathRules::WINDOWS;
        self
    }

    pub fn with_unix_path_rules(mut self) -> Self {
        self.rules = text::PathRules::UNIX;
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            dir: &self.dir,
            rules: self.rules,
        }
    }

    pub fn book_count(&self) -> Result<u64> {
        let n: i64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM books", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    fn invalidating(&self) -> Invalidate<'_> {
        Invalidate(self)
    }

    fn snapshot(
        &self,
        conn: &Connection,
        need_comments: bool,
    ) -> Result<(Arc<Vec<Record>>, Option<CommentMap>)> {
        let version: i64 = conn.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if guard.as_ref().is_some_and(|c| c.version != version) {
            *guard = None;
        }
        if guard.is_none() {
            *guard = Some(Cache {
                version,
                recs: Arc::new(record::load(conn, None)?),
                comments: None,
            });
        }
        let cache = guard.as_mut().unwrap();
        if need_comments && cache.comments.is_none() {
            cache.comments = Some(Arc::new(record::load_comments(conn)?));
        }
        Ok((
            cache.recs.clone(),
            if need_comments {
                cache.comments.clone()
            } else {
                None
            },
        ))
    }

    fn matching(
        &self,
        conn: &Connection,
        search: Option<&str>,
    ) -> Result<(Arc<Vec<Record>>, Vec<usize>)> {
        let compiled = match search.map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => Some(compile_search(conn, s)?),
            None => None,
        };
        let need_comments = compiled.as_ref().is_some_and(|c| c.needs_comments());
        let (recs, comments) = self.snapshot(conn, need_comments)?;
        let hits = match &compiled {
            None => (0..recs.len()).collect(),
            Some(c) => (0..recs.len())
                .filter(|&i| {
                    let r = &recs[i];
                    c.matches(
                        r,
                        comments
                            .as_ref()
                            .and_then(|m| m.get(&r.id))
                            .map(String::as_str),
                    )
                })
                .collect(),
        };
        Ok((recs, hits))
    }

    pub fn list_books(&self, q: &ListQuery) -> Result<Vec<BookSummary>> {
        let conn = self.conn();
        let (recs, hits) = self.matching(&conn, nonblank(&q.search))?;
        let sorted = sort_records(
            hits.into_iter().map(|i| &recs[i]).collect(),
            q.sort_by,
            q.descending,
        );
        Ok(sorted.into_iter().map(Record::summary).collect())
    }

    fn built(&self, recs: &Arc<Vec<Record>>, audio: &dyn AudioIndex) -> Result<Arc<catalog::Built>> {
        let key = Arc::as_ptr(recs) as usize;
        let generation = audio.generation();
        let mut guard = self.catalog.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = guard.as_ref()
            && c.recs == key
            && c.generation == generation
        {
            return Ok(c.built.clone());
        }
        let rows = audio.audiobooks().map_err(Error::Audio)?;
        let links = audio.links().map_err(Error::Audio)?;
        let built = Arc::new(catalog::build(recs, &rows, &links));
        *guard = Some(CatalogCache {
            recs: key,
            generation,
            built: built.clone(),
        });
        Ok(built)
    }

    fn entries(
        &self,
        conn: &Connection,
        scope: Scope,
        search: Option<&str>,
        audio: &dyn AudioIndex,
        then: &mut dyn FnMut(Vec<&Record>),
    ) -> Result<()> {
        let compiled = match search.map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => Some(compile_search(conn, s)?),
            None => None,
        };
        let need_comments = compiled.as_ref().is_some_and(|c| c.needs_comments());
        let (recs, comments) = self.snapshot(conn, need_comments)?;
        let built = self.built(&recs, audio)?;
        let ebooks = recs.iter().map(|r| built.overlay.get(&r.id).unwrap_or(r));
        let all: Vec<&Record> = match scope {
            Scope::Ebooks => ebooks.collect(),
            Scope::Audiobooks => built.audio.iter().collect(),
            Scope::All => ebooks
                .chain(built.unpaired.iter().map(|&i| &built.audio[i]))
                .collect(),
        };
        let hits = match &compiled {
            None => all,
            Some(c) => all
                .into_iter()
                .filter(|r| {
                    let text = if r.is_audio_entry() {
                        r.comments.as_deref()
                    } else {
                        comments
                            .as_ref()
                            .and_then(|m| m.get(&r.id))
                            .map(String::as_str)
                    };
                    c.matches(r, text)
                })
                .collect(),
        };
        then(hits);
        Ok(())
    }

    pub fn list_entries(&self, q: &ListQuery, audio: &dyn AudioIndex) -> Result<Vec<BookSummary>> {
        let conn = self.conn();
        let mut out = Vec::new();
        self.entries(&conn, q.scope, nonblank(&q.search), audio, &mut |hits| {
            out = sort_records(hits, q.sort_by, q.descending)
                .into_iter()
                .map(Record::summary)
                .collect();
        })?;
        Ok(out)
    }

    pub fn categories_entries(
        &self,
        scope: Scope,
        search: Option<&str>,
        audio: &dyn AudioIndex,
    ) -> Result<Vec<Category>> {
        let conn = self.conn();
        let mut out = Vec::new();
        let search = search.map(str::to_string);
        self.entries(&conn, scope, nonblank(&search), audio, &mut |hits| {
            out = categories::from_records(&hits);
        })?;
        Ok(out)
    }

    pub fn audio_pairs(&self, audio: &dyn AudioIndex) -> Result<HashMap<AudiobookId, (BookId, bool)>> {
        let conn = self.conn();
        let (recs, _) = self.snapshot(&conn, false)?;
        Ok(self.built(&recs, audio)?.pairs.clone())
    }

    pub fn audio_candidates(
        &self,
        audiobook: AudiobookId,
        audio: &dyn AudioIndex,
    ) -> Result<Vec<(BookSummary, f32)>> {
        let conn = self.conn();
        let (recs, _) = self.snapshot(&conn, false)?;
        let rows = audio.audiobooks().map_err(Error::Audio)?;
        let Some(row) = rows.iter().find(|r| r.id == audiobook) else {
            return Ok(Vec::new());
        };
        let index = catalog::pairing::Index::new(&recs);
        let by_id: HashMap<BookId, &Record> = recs.iter().map(|r| (r.id, r)).collect();
        Ok(index
            .candidates(row)
            .into_iter()
            .filter_map(|(id, score)| by_id.get(&id).map(|r| (r.summary(), score)))
            .take(8)
            .collect())
    }

    pub fn book_uuid(&self, id: BookId) -> Result<String> {
        Ok(Self::load_one(&self.conn(), id)?.uuid)
    }

    fn load_one(conn: &Connection, id: BookId) -> Result<Record> {
        record::load(conn, Some(id))?
            .into_iter()
            .next()
            .ok_or(Error::BookNotFound(id))
    }

    pub fn get_book(&self, id: BookId) -> Result<Book> {
        Ok(Self::load_one(&self.conn(), id)?.book())
    }

    pub fn metadata(&self, id: BookId) -> Result<Metadata> {
        Ok(Self::load_one(&self.conn(), id)?.metadata())
    }

    pub fn categories(&self, search: Option<&str>) -> Result<Vec<Category>> {
        let conn = self.conn();
        let visible: Option<HashSet<i64>> = match nonblank(&search.map(str::to_string)) {
            Some(s) => {
                let (recs, hits) = self.matching(&conn, Some(s))?;
                Some(hits.into_iter().map(|i| recs[i].id).collect())
            }
            None => None,
        };
        categories::build(&conn, visible.as_ref())
    }

    pub fn completions(&self, field: &str) -> Result<Vec<String>> {
        let sql = match field {
            "authors" => "SELECT name FROM authors",
            "series" => "SELECT name FROM series",
            "tags" => "SELECT name FROM tags",
            "publisher" | "publishers" => "SELECT name FROM publishers",
            "languages" => "SELECT lang_code FROM languages",
            other => {
                return Err(Error::Invalid {
                    field: "completion field",
                    value: other.to_string(),
                });
            }
        };
        let conn = self.conn();
        let mut stmt = conn.prepare(sql)?;
        let mut names: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|n| {
                n.map(|n| {
                    if field == "authors" {
                        n.replace('|', ",")
                    } else {
                        n
                    }
                })
            })
            .collect::<rusqlite::Result<_>>()?;
        names.sort_by_cached_key(|n| (text::sort_key(n), n.clone()));
        names.dedup();
        Ok(names)
    }

    pub fn saved_searches(&self) -> Result<Vec<SavedSearch>> {
        let map = saved_searches_map(&self.conn())?;
        let mut list: Vec<SavedSearch> = map
            .into_iter()
            .map(|(name, query)| SavedSearch { name, query })
            .collect();
        list.sort_by_cached_key(|s| (text::sort_key(&s.name), s.name.clone()));
        Ok(list)
    }

    pub fn set_saved_searches(&self, s: &[SavedSearch]) -> Result<()> {
        let _inv = self.invalidating();
        let mut map = BTreeMap::new();
        for item in s {
            let name = item.name.trim();
            if !name.is_empty() {
                map.insert(name.to_string(), item.query.trim().to_string());
            }
        }
        let raw = escape_json_ascii(&serde_json::to_string_pretty(&map)?);
        let conn = self.conn();
        let existing: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, val FROM preferences WHERE key='saved_searches'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match existing {
            Some((_, old)) if old == raw => {}
            Some((id, _)) => {
                conn.execute(
                    "UPDATE preferences SET val=?1 WHERE id=?2",
                    params![raw, id],
                )?;
            }
            None => {
                conn.execute(
                    "INSERT INTO preferences(key, val) VALUES ('saved_searches', ?1)",
                    [raw],
                )?;
            }
        }
        Ok(())
    }

    fn with_tx<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    pub fn update_book(&self, id: BookId, patch: &BookPatch) -> Result<Book> {
        let _inv = self.invalidating();
        let ctx = self.ctx();
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        Self::load_one(&tx, id)?;
        let mut w = Writer::new(&tx);
        let path_changed = w.apply_patch(id, patch)?;
        w.finish([id])?;
        drop(w);
        if path_changed {
            fsops::update_path(&ctx, &tx, id)?;
        }
        tx.commit()?;
        Ok(Self::load_one(&conn, id)?.book())
    }

    pub fn bulk_update(&self, ids: &[BookId], patch: &BulkPatch) -> Result<()> {
        let _inv = self.invalidating();
        let ctx = self.ctx();
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut touched: HashSet<i64> = HashSet::new();
        let mut path_ids: Vec<i64> = Vec::new();
        {
            let mut w = Writer::new(&tx);
            for (i, &id) in ids.iter().enumerate() {
                let rec = Self::load_one(&tx, id)?;
                if let Some(a) = &patch.authors {
                    w.set_authors(id, &write::clean_authors(a), true)?;
                    path_ids.push(id);
                }
                if let Some(s) = &patch.series {
                    w.set_one(&write::SERIES, id, Some(&write::single_text(s)), true)?;
                    if !s.trim().is_empty()
                        && let Some(start) = patch.series_start
                    {
                        w.set_series_index(id, start + i as f64)?;
                    }
                }
                if patch.clear_tags {
                    w.set_many(&write::TAGS, id, &[], true)?;
                }
                if !patch.remove_tags.is_empty() || !patch.add_tags.is_empty() {
                    let remove: HashSet<String> = patch
                        .remove_tags
                        .iter()
                        .map(|t| t.trim().to_lowercase())
                        .collect();
                    let current: Vec<String> = if patch.clear_tags {
                        Vec::new()
                    } else {
                        rec.tags
                            .iter()
                            .filter(|t| !remove.contains(&t.to_lowercase()))
                            .cloned()
                            .collect()
                    };
                    let mut all = current;
                    all.extend(patch.add_tags.iter().cloned());
                    w.set_many(&write::TAGS, id, &write::clean_tags(&all), true)?;
                }
                if let Some(p) = &patch.publisher {
                    w.set_one(&write::PUBLISHERS, id, Some(&write::single_text(p)), true)?;
                }
                if let Some(d) = &patch.pubdate {
                    w.set_pubdate(id, d)?;
                }
                if let Some(l) = &patch.languages {
                    w.set_many(&write::LANGUAGES, id, &write::clean_languages(l), true)?;
                }
                if let Some(r) = patch.rating {
                    w.set_rating(id, r)?;
                }
                touched.insert(id);
            }
            w.finish(touched.iter().copied())?;
        }
        for id in path_ids {
            fsops::update_path(&ctx, &tx, id)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_cover(&self, id: BookId, image_bytes: &[u8]) -> Result<()> {
        let _inv = self.invalidating();
        let jpeg = fsops::encode_cover(image_bytes)?;
        let ctx = self.ctx();
        self.with_tx(|tx| {
            Self::load_one(tx, id)?;
            fsops::write_cover(&ctx, tx, id, &jpeg)?;
            Writer::new(tx).finish([id])
        })
    }

    pub fn remove_cover(&self, id: BookId) -> Result<()> {
        let _inv = self.invalidating();
        let ctx = self.ctx();
        self.with_tx(|tx| {
            let row = fsops::book_row(tx, id)?;
            if !row.path.is_empty() {
                let file = ctx.join(&row.path).join(fsops::COVER_FILE_NAME);
                match fs::remove_file(&file) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            tx.execute("UPDATE books SET has_cover=0 WHERE id=?1", [id])?;
            Writer::new(tx).finish([id])
        })
    }

    pub fn cover_path(&self, id: BookId) -> Result<Option<PathBuf>> {
        let conn = self.conn();
        let (path, has_cover): (String, Option<i64>) = conn
            .query_row("SELECT path, has_cover FROM books WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?
            .ok_or(Error::BookNotFound(id))?;
        if path.is_empty() || has_cover.unwrap_or(0) == 0 {
            return Ok(None);
        }
        let file = self.ctx().join(&path).join(fsops::COVER_FILE_NAME);
        Ok(file.is_file().then_some(file))
    }

    pub fn book_dir(&self, id: BookId) -> Result<PathBuf> {
        let path: String = self
            .conn()
            .query_row("SELECT path FROM books WHERE id=?1", [id], |r| r.get(0))
            .optional()?
            .ok_or(Error::BookNotFound(id))?;
        Ok(self.ctx().join(&path))
    }

    pub fn format_path(&self, id: BookId, format: &str) -> Result<PathBuf> {
        let conn = self.conn();
        let row = fsops::book_row(&conn, id)?;
        let fmt = format.trim_start_matches('.').to_uppercase();
        let Some((_, name)) = row.formats.iter().find(|f| f.0 == fmt) else {
            return Err(Error::FormatNotFound { id, format: fmt });
        };
        let dir = self.ctx().join(&row.path);
        fsops::find_format_file(&dir, name, &fmt)
            .ok_or_else(|| Error::FileMissing(dir.join(format!("{name}.{}", fmt.to_lowercase()))))
    }

    pub fn add_book(
        &self,
        meta: &Metadata,
        cover: Option<&[u8]>,
        files: &[PathBuf],
    ) -> Result<BookId> {
        let _inv = self.invalidating();
        let jpeg = cover.map(fsops::encode_cover).transpose()?;
        let mut formats: Vec<(String, &PathBuf)> = Vec::new();
        for f in files {
            if !f.is_file() {
                return Err(Error::FileMissing(f.clone()));
            }
            let ext = f
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_uppercase)
                .filter(|e| !e.is_empty())
                .ok_or_else(|| Error::Invalid {
                    field: "file",
                    value: f.display().to_string(),
                })?;
            formats.retain(|(e, _)| *e != ext);
            formats.push((ext, f));
        }

        let ctx = self.ctx();
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut created_dir: Option<PathBuf> = None;
        let result = (|| -> Result<i64> {
            let mut w = Writer::new(&tx);
            let title = meta
                .title
                .as_deref()
                .map(write::single_text)
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "Unknown".to_string());
            let authors = write::clean_authors(&meta.authors);
            let series = meta
                .series
                .as_deref()
                .map(write::single_text)
                .filter(|s| !s.is_empty());
            let series_index = match (meta.series_index, &series) {
                (Some(i), _) => i,
                (None, Some(s)) => write::next_series_index(&tx, s)?,
                (None, None) => 1.0,
            };
            let author_sort = match nonblank(&meta.author_sort.clone()) {
                Some(s) => s.to_string(),
                None => w.author_sort_from_names(&authors)?,
            };
            tx.execute(
                "INSERT INTO books(title, series_index, author_sort) VALUES (?1, ?2, ?3)",
                params![title, series_index, author_sort],
            )?;
            let id = tx.last_insert_rowid();
            let now = dates::now_db();
            let sort = match nonblank(&meta.title_sort) {
                Some(s) => s.to_string(),
                None => text::title_sort(&title, meta.languages.first().map(String::as_str)),
            };
            tx.execute(
                "UPDATE books SET sort=?1, timestamp=?2, pubdate=?3, last_modified=?2 WHERE id=?4",
                params![sort, now, dates::UNDEFINED, id],
            )?;
            w.set_many(&write::AUTHORS, id, &authors, false)?;
            w.set_many(&write::TAGS, id, &write::clean_tags(&meta.tags), false)?;
            if let Some(s) = &series {
                w.set_one(&write::SERIES, id, Some(s), false)?;
            }
            if let Some(p) = meta.publisher.as_deref().map(write::single_text) {
                w.set_one(&write::PUBLISHERS, id, Some(&p), false)?;
            }
            w.set_many(
                &write::LANGUAGES,
                id,
                &write::clean_languages(&meta.languages),
                false,
            )?;
            if let Some(r) = meta.rating {
                w.set_rating(id, r)?;
            }
            if let Some(c) = &meta.comments {
                w.set_comments(id, c)?;
            }
            if let Some(d) = &meta.pubdate {
                w.set_pubdate(id, d)?;
            }
            w.set_identifiers(id, &meta.identifiers)?;
            w.finish([id])?;
            drop(w);

            fsops::update_path(&ctx, &tx, id)?;
            let row = fsops::book_row(&tx, id)?;
            let book_dir = ctx.join(&row.path);
            if !book_dir.exists() {
                created_dir = Some(book_dir.clone());
            }
            for (ext, file) in &formats {
                fsops::add_format_file(&ctx, &tx, id, file, ext)?;
            }
            if let Some(j) = &jpeg {
                fsops::write_cover(&ctx, &tx, id, j)?;
            }
            Ok(id)
        })();
        let id = match result {
            Ok(i) => i,
            Err(e) => {
                drop(tx);
                if let Some(d) = created_dir {
                    let _ = fs::remove_dir_all(&d);
                    if let Some(p) = d.parent() {
                        fsops::remove_dir_if_empty(p);
                    }
                }
                return Err(e);
            }
        };
        tx.commit()?;
        Ok(id)
    }

    pub fn add_format(&self, id: BookId, file: &Path) -> Result<()> {
        let _inv = self.invalidating();
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_uppercase)
            .filter(|e| !e.is_empty())
            .ok_or_else(|| Error::Invalid {
                field: "file",
                value: file.display().to_string(),
            })?;
        let ctx = self.ctx();
        self.with_tx(|tx| {
            fsops::add_format_file(&ctx, tx, id, file, &ext)?;
            Writer::new(tx).finish([id])
        })
    }

    pub fn remove_format(&self, id: BookId, format: &str) -> Result<()> {
        let _inv = self.invalidating();
        let ctx = self.ctx();
        self.with_tx(|tx| {
            fsops::remove_format_file(&ctx, tx, id, format)?;
            Writer::new(tx).finish([id])
        })
    }

    pub fn delete_books(&self, ids: &[BookId]) -> Result<()> {
        let _inv = self.invalidating();
        let ctx = self.ctx();
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        let result = (|| -> Result<()> {
            let mut w = Writer::new(&tx);
            for &id in ids {
                let Some(rec) = record::load(&tx, Some(id))?.into_iter().next() else {
                    continue;
                };
                let items = w.linked_items(id)?;
                if let Some(pair) = fsops::trash_book(&ctx, &rec)? {
                    moved.push(pair);
                }
                delete_custom_column_rows(&tx, id)?;
                tx.execute("DELETE FROM books WHERE id=?1", [id])?;
                tx.execute("DELETE FROM metadata_dirtied WHERE book=?1", [id])?;
                w.prune_all(&items)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            drop(tx);
            for (src, dest) in moved.into_iter().rev() {
                let _ = fsops::move_path(&dest, &src);
            }
            return Err(e);
        }
        tx.commit()?;
        Ok(())
    }
}

fn delete_custom_column_rows(conn: &Connection, id: BookId) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id FROM custom_columns")?;
    let cols: Vec<i64> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for c in cols {
        for (table, col) in [
            (format!("custom_column_{c}"), "book"),
            (format!("books_custom_column_{c}_link"), "book"),
        ] {
            if db::has_table(conn, &table) {
                let has_book_col = conn
                    .prepare(&format!("SELECT {col} FROM {table} LIMIT 0"))
                    .is_ok();
                if has_book_col {
                    conn.execute(&format!("DELETE FROM {table} WHERE {col}=?1"), [id])?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computed_paths_match_a_real_calibre_library() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/library/metadata.db");
        if !src.is_file() {
            return;
        }
        let tmp = std::env::temp_dir().join(format!("shelfmark-path-test-{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        fs::copy(&src, tmp.join("metadata.db")).unwrap();
        let lib = Library::open(&tmp).unwrap();
        let recs = record::load(&lib.conn(), None).unwrap();
        let mut matching = 0;
        let mut name_total = 0;
        let mut name_matching = 0;
        for r in &recs {
            let first = r.authors.first().map(String::as_str).unwrap_or("Unknown");
            if text::construct_path_name(40, r.id, &r.title, first) == r.path {
                matching += 1;
            }
            for f in &r.formats {
                name_total += 1;
                if text::construct_file_name(
                    text::PathRules::WINDOWS,
                    &r.title,
                    first,
                    f.format.len() + 1,
                ) == f.name
                {
                    name_matching += 1;
                }
            }
        }
        assert!(
            name_matching as f64 >= 0.97 * name_total as f64,
            "only {name_matching} of {name_total} computed file names match calibre's"
        );
        drop(lib);
        let _ = fs::remove_dir_all(&tmp);
        assert!(
            matching as f64 >= 0.97 * recs.len() as f64,
            "only {matching} of {} computed paths match calibre's",
            recs.len()
        );
    }
}
