use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, Row, params};
use shelfmark_model::{
    AudioChapter, AudioFile, AudioIndex, AudioLayout, AudioLink, AudioRoot, AudioState,
    AudiobookDetail, AudiobookId, AudiobookRow, BookId, LinkState,
};

use crate::error::{Error, Result};

const SCHEMA_VERSION: i64 = 1;
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

const BOOK_COLUMNS: &str = "id, root_id, state, title, subtitle, authors, narrators, series, \
     series_index, publisher, language, genres, description, asin, isbn, release_date, \
     duration_ms, size, format, file_count, has_cover, added, modified";

pub struct AudioStore {
    dir: PathBuf,
    conn: Mutex<Connection>,
    generation: AtomicU64,
    cache: Mutex<Option<(u64, Arc<Vec<AudiobookRow>>)>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewRoot {
    pub path: String,
    pub in_library: bool,
    pub managed: bool,
    pub layout: AudioLayout,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RootPatch {
    pub path: Option<String>,
    pub managed: Option<bool>,
    pub layout: Option<AudioLayout>,
    pub label: Option<Option<String>>,
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexedFile {
    pub audiobook_id: AudiobookId,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexedBook {
    pub id: AudiobookId,
    pub key: String,
    pub asin: Option<String>,
    pub state: AudioState,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScannedFile {
    pub rel_path: String,
    pub kind: String,
    pub size: u64,
    pub mtime: i64,
    pub duration_ms: Option<u64>,
    pub track: Option<u32>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScannedBook {
    pub key: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub genres: Vec<String>,
    pub description: Option<String>,
    pub asin: Option<String>,
    pub isbn: Option<String>,
    pub release_date: Option<String>,
    pub duration_ms: u64,
    pub format: String,
    pub meta_source: String,
    pub added: String,
    pub modified: String,
    pub files: Vec<ScannedFile>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteBook {
    pub asin: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub release_date: Option<String>,
    pub duration_ms: u64,
    pub description: Option<String>,
    pub added: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteSync {
    pub total: u32,
    pub added: u32,
    pub removed: u32,
    pub without_cover: Vec<(AudiobookId, String)>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub path: String,
    pub message: String,
}

fn json_list(raw: Option<String>) -> Vec<String> {
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn book_from_row(r: &Row) -> rusqlite::Result<AudiobookRow> {
    Ok(AudiobookRow {
        id: r.get(0)?,
        root_id: r.get(1)?,
        state: AudioState::parse(&r.get::<_, String>(2)?),
        title: r.get(3)?,
        subtitle: r.get(4)?,
        authors: json_list(r.get(5)?),
        narrators: json_list(r.get(6)?),
        series: r.get(7)?,
        series_index: r.get(8)?,
        publisher: r.get(9)?,
        language: r.get(10)?,
        genres: json_list(r.get(11)?),
        description: r.get(12)?,
        asin: r.get(13)?,
        isbn: r.get(14)?,
        release_date: r.get(15)?,
        duration_ms: r.get::<_, i64>(16)?.max(0) as u64,
        size: r.get::<_, i64>(17)?.max(0) as u64,
        format: r.get(18)?,
        file_count: r.get::<_, i64>(19)?.max(0) as u32,
        has_cover: r.get::<_, i64>(20)? != 0,
        added: r.get(21)?,
        modified: r.get(22)?,
    })
}

fn root_from_row(r: &Row) -> rusqlite::Result<AudioRoot> {
    Ok(AudioRoot {
        id: r.get(0)?,
        path: r.get(1)?,
        in_library: r.get::<_, i64>(2)? != 0,
        managed: r.get::<_, i64>(3)? != 0,
        layout: AudioLayout::parse(&r.get::<_, String>(4)?),
        label: r.get(5)?,
        enabled: r.get::<_, i64>(6)? != 0,
        last_scan: r.get(7)?,
        last_scan_ok: r.get::<_, Option<i64>>(8)?.map(|v| v != 0),
        audiobooks: r.get::<_, i64>(9)?.max(0) as u32,
        missing: r.get::<_, i64>(10)?.max(0) as u32,
    })
}

const ROOT_SELECT: &str = "SELECT r.id, r.path, r.in_library, r.managed, r.layout, r.label, \
     r.enabled, r.last_scan, r.last_scan_ok, \
     (SELECT COUNT(*) FROM audiobook a WHERE a.root_id = r.id AND a.state = 'present'), \
     (SELECT COUNT(*) FROM audiobook a WHERE a.root_id = r.id AND a.state = 'missing') \
     FROM audio_root r";

impl AudioStore {
    pub fn open(library_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(library_dir)?;
        let conn = Connection::open(library_dir.join("shelfmark.db"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        shelfmark_reading::set_journal_mode(&conn, library_dir)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&conn)?;
        Ok(Self {
            dir: library_dir.to_path_buf(),
            conn: Mutex::new(conn),
            generation: AtomicU64::new(1),
            cache: Mutex::new(None),
        })
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| Error::Poisoned)
    }

    fn bump(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    pub fn library_dir(&self) -> &Path {
        &self.dir
    }

    pub fn root_dir(&self, root: &AudioRoot) -> PathBuf {
        if root.in_library {
            self.dir.join(&root.path)
        } else {
            PathBuf::from(&root.path)
        }
    }

    pub fn roots(&self) -> Result<Vec<AudioRoot>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("{ROOT_SELECT} ORDER BY r.id"))?;
        let rows = stmt.query_map([], root_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn root(&self, id: i64) -> Result<AudioRoot> {
        self.conn()?
            .query_row(&format!("{ROOT_SELECT} WHERE r.id = ?1"), [id], root_from_row)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("audiobook location {id}")))
    }

    pub fn add_root(&self, new: &NewRoot) -> Result<AudioRoot> {
        let path = new.path.trim();
        if path.is_empty() {
            return Err(Error::Invalid("location path is empty".into()));
        }
        let id = {
            let conn = self.conn()?;
            let existing: Option<i64> = conn
                .query_row(
                    "SELECT id FROM audio_root WHERE path = ?1 AND in_library = ?2",
                    params![path, new.in_library],
                    |r| r.get(0),
                )
                .optional()?;
            if existing.is_some() {
                return Err(Error::Invalid(format!(
                    "location already added: {path}"
                )));
            }
            conn.execute(
                "INSERT INTO audio_root (path, in_library, managed, layout, label)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    path,
                    new.in_library,
                    new.managed,
                    new.layout.as_str(),
                    new.label
                ],
            )?;
            conn.last_insert_rowid()
        };
        self.bump();
        self.root(id)
    }

    pub fn update_root(&self, id: i64, patch: &RootPatch) -> Result<AudioRoot> {
        {
            let conn = self.conn()?;
            if let Some(path) = &patch.path {
                let path = path.trim();
                if path.is_empty() {
                    return Err(Error::Invalid("location path is empty".into()));
                }
                conn.execute(
                    "UPDATE audio_root SET path = ?2 WHERE id = ?1",
                    params![id, path],
                )?;
            }
            if let Some(managed) = patch.managed {
                conn.execute(
                    "UPDATE audio_root SET managed = ?2 WHERE id = ?1",
                    params![id, managed],
                )?;
            }
            if let Some(layout) = patch.layout {
                conn.execute(
                    "UPDATE audio_root SET layout = ?2 WHERE id = ?1",
                    params![id, layout.as_str()],
                )?;
            }
            if let Some(label) = &patch.label {
                conn.execute(
                    "UPDATE audio_root SET label = ?2 WHERE id = ?1",
                    params![id, label],
                )?;
            }
            if let Some(enabled) = patch.enabled {
                conn.execute(
                    "UPDATE audio_root SET enabled = ?2 WHERE id = ?1",
                    params![id, enabled],
                )?;
            }
        }
        self.bump();
        self.root(id)
    }

    pub fn remove_root(&self, id: i64) -> Result<()> {
        self.conn()?
            .execute("DELETE FROM audio_root WHERE id = ?1", [id])?;
        self.bump();
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT value FROM audio_setting WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
        let conn = self.conn()?;
        match value {
            Some(v) => conn.execute(
                "INSERT INTO audio_setting (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?,
            None => conn.execute("DELETE FROM audio_setting WHERE key = ?1", [key])?,
        };
        Ok(())
    }

    pub fn indexed_files(&self, root_id: i64) -> Result<HashMap<String, IndexedFile>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT f.rel_path, f.audiobook_id, f.size, f.mtime
             FROM audio_file f JOIN audiobook a ON a.id = f.audiobook_id
             WHERE a.root_id = ?1",
        )?;
        let rows = stmt.query_map([root_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                IndexedFile {
                    audiobook_id: r.get(1)?,
                    size: r.get::<_, i64>(2)?.max(0) as u64,
                    mtime: r.get(3)?,
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn indexed_books(&self, root_id: i64) -> Result<Vec<IndexedBook>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT id, key, asin, state FROM audiobook WHERE root_id = ?1")?;
        let rows = stmt.query_map([root_id], |r| {
            Ok(IndexedBook {
                id: r.get(0)?,
                key: r.get(1)?,
                asin: r.get(2)?,
                state: AudioState::parse(&r.get::<_, String>(3)?),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn next_scan_gen(&self, root_id: i64) -> Result<i64> {
        let max: Option<i64> = self.conn()?.query_row(
            "SELECT MAX(scan_gen) FROM audiobook WHERE root_id = ?1",
            [root_id],
            |r| r.get(0),
        )?;
        Ok(max.unwrap_or(0) + 1)
    }

    pub fn rekey_book(&self, id: AudiobookId, key: &str) -> Result<()> {
        self.conn()?.execute(
            "UPDATE audiobook SET key = ?2 WHERE id = ?1",
            params![id, key],
        )?;
        self.bump();
        Ok(())
    }

    pub fn upsert_books(
        &self,
        root_id: i64,
        scan_gen: i64,
        books: &[ScannedBook],
    ) -> Result<Vec<AudiobookId>> {
        let mut ids = Vec::with_capacity(books.len());
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            for b in books {
                let size: u64 = b
                    .files
                    .iter()
                    .filter(|f| f.kind == "audio")
                    .map(|f| f.size)
                    .sum();
                let count = b.files.iter().filter(|f| f.kind == "audio").count() as i64;
                tx.execute(
                    "INSERT INTO audiobook
                       (root_id, key, state, asin, isbn, title, subtitle, authors, narrators,
                        series, series_index, publisher, language, release_date, genres,
                        description, duration_ms, size, file_count, format, meta_source,
                        added, modified, scan_gen)
                     VALUES (?1, ?2, 'present', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                             ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
                     ON CONFLICT(root_id, key) DO UPDATE SET
                       state = 'present', asin = excluded.asin, isbn = excluded.isbn,
                       title = excluded.title, subtitle = excluded.subtitle,
                       authors = excluded.authors, narrators = excluded.narrators,
                       series = excluded.series, series_index = excluded.series_index,
                       publisher = excluded.publisher, language = excluded.language,
                       release_date = excluded.release_date, genres = excluded.genres,
                       description = excluded.description, duration_ms = excluded.duration_ms,
                       size = excluded.size, file_count = excluded.file_count,
                       format = excluded.format, meta_source = excluded.meta_source,
                       modified = excluded.modified, scan_gen = excluded.scan_gen,
                       chapters = NULL",
                    params![
                        root_id,
                        b.key,
                        b.asin,
                        b.isbn,
                        b.title,
                        b.subtitle,
                        serde_json::to_string(&b.authors)?,
                        serde_json::to_string(&b.narrators)?,
                        b.series,
                        b.series_index,
                        b.publisher,
                        b.language,
                        b.release_date,
                        serde_json::to_string(&b.genres)?,
                        b.description,
                        b.duration_ms as i64,
                        size as i64,
                        count,
                        b.format,
                        b.meta_source,
                        b.added,
                        b.modified,
                        scan_gen,
                    ],
                )?;
                let id: AudiobookId = tx.query_row(
                    "SELECT id FROM audiobook WHERE root_id = ?1 AND key = ?2",
                    params![root_id, b.key],
                    |r| r.get(0),
                )?;
                tx.execute("DELETE FROM audio_file WHERE audiobook_id = ?1", [id])?;
                for f in &b.files {
                    tx.execute(
                        "INSERT OR REPLACE INTO audio_file
                           (audiobook_id, rel_path, kind, size, mtime, duration_ms, track, title)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            id,
                            f.rel_path,
                            f.kind,
                            f.size as i64,
                            f.mtime,
                            f.duration_ms.map(|d| d as i64),
                            f.track,
                            f.title
                        ],
                    )?;
                }
                ids.push(id);
            }
            tx.commit()?;
        }
        self.bump();
        Ok(ids)
    }

    pub fn touch_books(&self, scan_gen: i64, ids: &[AudiobookId]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut changed = 0;
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            for id in ids {
                changed += tx.execute(
                    "UPDATE audiobook SET scan_gen = ?2, state = 'present'
                     WHERE id = ?1 AND (state <> 'present' OR scan_gen <> ?2)",
                    params![id, scan_gen],
                )?;
            }
            tx.commit()?;
        }
        if changed > 0 {
            self.bump();
        }
        Ok(())
    }

    pub fn finish_scan(&self, root_id: i64, scan_gen: i64, complete: bool) -> Result<u32> {
        let missing = {
            let conn = self.conn()?;
            let missing = if complete {
                conn.execute(
                    "UPDATE audiobook SET state = 'missing'
                     WHERE root_id = ?1 AND scan_gen < ?2 AND state = 'present'",
                    params![root_id, scan_gen],
                )?
            } else {
                0
            };
            conn.execute(
                &format!(
                    "UPDATE audio_root SET last_scan = {NOW}, last_scan_ok = ?2 WHERE id = ?1"
                ),
                params![root_id, complete],
            )?;
            missing
        };
        self.bump();
        Ok(missing as u32)
    }

    pub fn forget_missing(&self, root_id: i64) -> Result<u32> {
        let n = self.conn()?.execute(
            "DELETE FROM audiobook WHERE root_id = ?1 AND state = 'missing'",
            [root_id],
        )?;
        self.bump();
        Ok(n as u32)
    }

    pub fn set_cover(&self, id: AudiobookId, thumb: &[u8], source: &str, mtime: i64) -> Result<()> {
        {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO audio_cover (audiobook_id, thumb, source, src_mtime)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(audiobook_id) DO UPDATE SET
                   thumb = excluded.thumb, source = excluded.source,
                   src_mtime = excluded.src_mtime",
                params![id, thumb, source, mtime],
            )?;
            conn.execute("UPDATE audiobook SET has_cover = 1 WHERE id = ?1", [id])?;
        }
        self.bump();
        Ok(())
    }

    pub fn cover(&self, id: AudiobookId) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT thumb FROM audio_cover WHERE audiobook_id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn cover_mtime(&self, id: AudiobookId) -> Result<Option<(String, i64)>> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT source, COALESCE(src_mtime, 0) FROM audio_cover WHERE audiobook_id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn set_issues(&self, root_id: i64, issues: &[Issue]) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM audio_issue WHERE root_id = ?1", [root_id])?;
        for i in issues {
            tx.execute(
                &format!(
                    "INSERT INTO audio_issue (root_id, path, message, seen)
                     VALUES (?1, ?2, ?3, {NOW})"
                ),
                params![root_id, i.path, i.message],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn issues_in(&self, root_id: i64) -> Result<Vec<Issue>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT path, message FROM audio_issue WHERE root_id = ?1 ORDER BY path")?;
        let rows = stmt.query_map([root_id], |r| {
            Ok(Issue {
                path: r.get(0)?,
                message: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn issues(&self) -> Result<Vec<Issue>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT path, message FROM audio_issue ORDER BY path")?;
        let rows = stmt.query_map([], |r| {
            Ok(Issue {
                path: r.get(0)?,
                message: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn list(&self) -> Result<Arc<Vec<AudiobookRow>>> {
        let generation = self.generation.load(Ordering::SeqCst);
        let mut cache = self.cache.lock().map_err(|_| Error::Poisoned)?;
        if let Some((g, rows)) = cache.as_ref()
            && *g == generation
        {
            return Ok(rows.clone());
        }
        let rows = {
            let conn = self.conn()?;
            let mut stmt = conn.prepare(&format!(
                "SELECT {BOOK_COLUMNS} FROM audiobook a
                 WHERE (a.root_id IS NULL AND NOT EXISTS (
                          SELECT 1 FROM audiobook d JOIN audio_root r ON r.id = d.root_id
                          WHERE d.asin = a.asin AND d.state = 'present' AND r.enabled = 1))
                    OR EXISTS (SELECT 1 FROM audio_root r WHERE r.id = a.root_id AND r.enabled = 1)
                 ORDER BY a.id"
            ))?;
            let rows = stmt.query_map([], book_from_row)?;
            Arc::new(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        };
        *cache = Some((generation, rows.clone()));
        Ok(rows)
    }

    pub fn detail(&self, id: AudiobookId) -> Result<AudiobookDetail> {
        let conn = self.conn()?;
        let (row, key, chapters): (AudiobookRow, String, Option<String>) = conn
            .query_row(
                &format!("SELECT {BOOK_COLUMNS}, key, chapters FROM audiobook WHERE id = ?1"),
                [id],
                |r| Ok((book_from_row(r)?, r.get(23)?, r.get(24)?)),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("audiobook {id}")))?;
        let mut stmt = conn.prepare(
            "SELECT rel_path, kind, size, duration_ms, track, title FROM audio_file
             WHERE audiobook_id = ?1 ORDER BY kind <> 'audio', track IS NULL, track, rel_path",
        )?;
        let files: Vec<AudioFile> = stmt
            .query_map([id], |r| {
                Ok(AudioFile {
                    rel_path: r.get(0)?,
                    kind: r.get(1)?,
                    size: r.get::<_, i64>(2)?.max(0) as u64,
                    duration_ms: r.get::<_, Option<i64>>(3)?.map(|d| d.max(0) as u64),
                    track: r.get(4)?,
                    title: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let multi = files.iter().filter(|f| f.kind == "audio").count() > 1;
        let dir = if multi {
            Some(key)
        } else {
            Some(match key.rsplit_once('/') {
                Some((d, _)) => d.to_string(),
                None => String::new(),
            })
        };
        let chapters: Vec<AudioChapter> = chapters
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default();
        Ok(AudiobookDetail {
            row,
            dir,
            files,
            chapters,
        })
    }

    pub fn has_chapters(&self, id: AudiobookId) -> Result<bool> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT chapters IS NOT NULL FROM audiobook WHERE id = ?1",
                [id],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false))
    }

    pub fn set_chapters(&self, id: AudiobookId, chapters: &[AudioChapter]) -> Result<()> {
        self.conn()?.execute(
            "UPDATE audiobook SET chapters = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(chapters)?],
        )?;
        Ok(())
    }

    pub fn relocate(
        &self,
        id: AudiobookId,
        root_id: i64,
        key: &str,
        files: &[(String, String, i64)],
    ) -> Result<()> {
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE audiobook SET root_id = ?2, key = ?3, state = 'present' WHERE id = ?1",
                params![id, root_id, key],
            )?;
            for (old, new, mtime) in files {
                tx.execute(
                    "UPDATE audio_file SET rel_path = ?3, mtime = ?4
                     WHERE audiobook_id = ?1 AND rel_path = ?2",
                    params![id, old, new, mtime],
                )?;
            }
            tx.commit()?;
        }
        self.bump();
        Ok(())
    }

    pub fn sync_remote(&self, account: &str, books: &[RemoteBook]) -> Result<RemoteSync> {
        let prefix = format!("audible:{account}:");
        let mut out = RemoteSync {
            total: books.len() as u32,
            ..Default::default()
        };
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            let mut keep: Vec<String> = Vec::with_capacity(books.len());
            for b in books {
                let key = format!("{prefix}{}", b.asin);
                let existing: Option<AudiobookId> = tx
                    .query_row(
                        "SELECT id FROM audiobook WHERE root_id IS NULL AND key = ?1",
                        [&key],
                        |r| r.get(0),
                    )
                    .optional()?;
                let authors = serde_json::to_string(&b.authors)?;
                let narrators = serde_json::to_string(&b.narrators)?;
                match existing {
                    Some(id) => {
                        tx.execute(
                            "UPDATE audiobook SET title = ?2, subtitle = ?3, authors = ?4,
                               narrators = ?5, series = ?6, series_index = ?7, publisher = ?8,
                               language = ?9, release_date = ?10, description = ?11,
                               duration_ms = ?12, modified = {NOW_SQL}
                             WHERE id = ?1"
                                .replace("{NOW_SQL}", NOW)
                                .as_str(),
                            params![
                                id,
                                b.title,
                                b.subtitle,
                                authors,
                                narrators,
                                b.series,
                                b.series_index,
                                b.publisher,
                                b.language,
                                b.release_date,
                                b.description,
                                b.duration_ms as i64
                            ],
                        )?;
                    }
                    None => {
                        tx.execute(
                            "INSERT INTO audiobook
                               (root_id, key, source, state, asin, title, subtitle, authors,
                                narrators, series, series_index, publisher, language,
                                release_date, description, duration_ms, format, meta_source,
                                added, modified)
                             VALUES (NULL, ?1, 'audible', 'remote', ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                                     ?9, ?10, ?11, ?12, ?13, 'AUDIBLE', 'audible',
                                     COALESCE(?14, {NOW_SQL}), {NOW_SQL})"
                                .replace("{NOW_SQL}", NOW)
                                .as_str(),
                            params![
                                key,
                                b.asin,
                                b.title,
                                b.subtitle,
                                authors,
                                narrators,
                                b.series,
                                b.series_index,
                                b.publisher,
                                b.language,
                                b.release_date,
                                b.description,
                                b.duration_ms as i64,
                                b.added
                            ],
                        )?;
                        out.added += 1;
                    }
                }
                keep.push(key);
            }
            let mut stmt = tx.prepare(
                "SELECT id, key FROM audiobook WHERE root_id IS NULL AND key LIKE ?1 || '%'",
            )?;
            let stale: Vec<AudiobookId> = stmt
                .query_map([&prefix], |r| Ok((r.get::<_, AudiobookId>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .filter(|(_, key)| !keep.contains(key))
                .map(|(id, _)| id)
                .collect();
            drop(stmt);
            for id in stale {
                tx.execute("DELETE FROM audiobook WHERE id = ?1", [id])?;
                out.removed += 1;
            }
            let mut stmt = tx.prepare(
                "SELECT id, asin FROM audiobook
                 WHERE root_id IS NULL AND key LIKE ?1 || '%' AND has_cover = 0",
            )?;
            out.without_cover = stmt
                .query_map([&prefix], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            tx.commit()?;
        }
        self.bump();
        Ok(out)
    }

    pub fn clear_remote(&self) -> Result<u32> {
        let n = self
            .conn()?
            .execute("DELETE FROM audiobook WHERE root_id IS NULL", [])?;
        self.bump();
        Ok(n as u32)
    }

    pub fn remote_count(&self) -> Result<u32> {
        let n: i64 = self.conn()?.query_row(
            "SELECT COUNT(*) FROM audiobook WHERE root_id IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(n as u32)
    }

    pub fn all_links(&self) -> Result<Vec<AudioLink>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT audiobook_id, book_id, book_uuid, state FROM audio_link ORDER BY rowid")?;
        let rows = stmt.query_map([], |r| {
            Ok(AudioLink {
                audiobook_id: r.get(0)?,
                book_id: r.get(1)?,
                book_uuid: r.get(2)?,
                state: if r.get::<_, String>(3)? == "rejected" {
                    LinkState::Rejected
                } else {
                    LinkState::Manual
                },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_link(
        &self,
        audiobook: AudiobookId,
        book: BookId,
        uuid: Option<&str>,
        state: LinkState,
    ) -> Result<()> {
        let state = match state {
            LinkState::Manual => "manual",
            LinkState::Rejected => "rejected",
        };
        self.conn()?.execute(
            &format!(
                "INSERT INTO audio_link (audiobook_id, book_id, book_uuid, state, updated)
                 VALUES (?1, ?2, ?3, ?4, {NOW})
                 ON CONFLICT(audiobook_id, book_id) DO UPDATE SET
                   book_uuid = excluded.book_uuid, state = excluded.state,
                   updated = excluded.updated"
            ),
            params![audiobook, book, uuid, state],
        )?;
        self.bump();
        Ok(())
    }

    pub fn clear_link(&self, audiobook: AudiobookId, book: BookId) -> Result<()> {
        self.conn()?.execute(
            "DELETE FROM audio_link WHERE audiobook_id = ?1 AND book_id = ?2",
            params![audiobook, book],
        )?;
        self.bump();
        Ok(())
    }

    pub fn clear_links_for_audiobook(&self, audiobook: AudiobookId) -> Result<()> {
        self.conn()?
            .execute("DELETE FROM audio_link WHERE audiobook_id = ?1", [audiobook])?;
        self.bump();
        Ok(())
    }

    pub fn delete_book_links(&self, book: BookId) -> Result<()> {
        self.conn()?
            .execute("DELETE FROM audio_link WHERE book_id = ?1", [book])?;
        self.bump();
        Ok(())
    }
}

impl AudioIndex for AudioStore {
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn audiobooks(&self) -> std::result::Result<Arc<Vec<AudiobookRow>>, String> {
        self.list().map_err(|e| e.to_string())
    }

    fn links(&self) -> std::result::Result<Vec<AudioLink>, String> {
        self.all_links().map_err(|e| e.to_string())
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS audio_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    let version: i64 = conn
        .query_row(
            "SELECT value FROM audio_meta WHERE key = 'schema'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if version > SCHEMA_VERSION {
        return Err(Error::SchemaVersion(version));
    }
    if version < 1 {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE audio_setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE audio_root (
               id           INTEGER PRIMARY KEY,
               path         TEXT    NOT NULL,
               in_library   INTEGER NOT NULL DEFAULT 0,
               managed      INTEGER NOT NULL DEFAULT 0,
               layout       TEXT    NOT NULL DEFAULT 'libation',
               label        TEXT,
               enabled      INTEGER NOT NULL DEFAULT 1,
               last_scan    TEXT,
               last_scan_ok INTEGER,
               UNIQUE (path, in_library)
             );
             CREATE TABLE audiobook (
               id           INTEGER PRIMARY KEY AUTOINCREMENT,
               root_id      INTEGER REFERENCES audio_root(id) ON DELETE CASCADE,
               key          TEXT    NOT NULL,
               source       TEXT    NOT NULL DEFAULT 'disk',
               state        TEXT    NOT NULL DEFAULT 'present',
               asin         TEXT,
               isbn         TEXT,
               title        TEXT    NOT NULL,
               subtitle     TEXT,
               authors      TEXT    NOT NULL DEFAULT '[]',
               narrators    TEXT    NOT NULL DEFAULT '[]',
               series       TEXT,
               series_index REAL,
               publisher    TEXT,
               language     TEXT,
               release_date TEXT,
               genres       TEXT    NOT NULL DEFAULT '[]',
               description  TEXT,
               duration_ms  INTEGER NOT NULL DEFAULT 0,
               size         INTEGER NOT NULL DEFAULT 0,
               file_count   INTEGER NOT NULL DEFAULT 0,
               format       TEXT    NOT NULL,
               meta_source  TEXT    NOT NULL,
               chapters     TEXT,
               has_cover    INTEGER NOT NULL DEFAULT 0,
               added        TEXT    NOT NULL,
               modified     TEXT    NOT NULL,
               scan_gen     INTEGER NOT NULL DEFAULT 0,
               UNIQUE (root_id, key)
             );
             CREATE INDEX audiobook_asin ON audiobook (asin);
             CREATE TABLE audio_file (
               id           INTEGER PRIMARY KEY,
               audiobook_id INTEGER NOT NULL REFERENCES audiobook(id) ON DELETE CASCADE,
               rel_path     TEXT    NOT NULL,
               kind         TEXT    NOT NULL,
               size         INTEGER NOT NULL,
               mtime        INTEGER NOT NULL,
               duration_ms  INTEGER,
               track        INTEGER,
               title        TEXT,
               UNIQUE (audiobook_id, rel_path)
             );
             CREATE TABLE audio_cover (
               audiobook_id INTEGER PRIMARY KEY REFERENCES audiobook(id) ON DELETE CASCADE,
               thumb        BLOB NOT NULL,
               source       TEXT NOT NULL,
               src_mtime    INTEGER
             );
             CREATE TABLE audio_link (
               audiobook_id INTEGER NOT NULL REFERENCES audiobook(id) ON DELETE CASCADE,
               book_id      INTEGER NOT NULL,
               book_uuid    TEXT,
               state        TEXT    NOT NULL CHECK (state IN ('manual', 'rejected')),
               updated      TEXT    NOT NULL,
               PRIMARY KEY (audiobook_id, book_id)
             );
             CREATE INDEX audio_link_book ON audio_link (book_id);
             CREATE TABLE audio_issue (
               root_id INTEGER REFERENCES audio_root(id) ON DELETE CASCADE,
               path    TEXT NOT NULL,
               message TEXT NOT NULL,
               seen    TEXT NOT NULL
             );
             INSERT OR REPLACE INTO audio_meta (key, value) VALUES ('schema', '1');
             COMMIT;",
        )?;
    }
    Ok(())
}
