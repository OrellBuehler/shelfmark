mod error;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, Row, params};
use shelfmark_model::{Annotation, AnnotationKind, BookId, ReadingState};

pub use error::{Error, Result};

const SCHEMA_VERSION: i64 = 1;
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

pub struct ReadingStore {
    conn: Mutex<Connection>,
}

impl ReadingStore {
    pub fn open(library_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(library_dir)?;
        let conn = Connection::open(library_dir.join("shelfmark.db"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| Error::Poisoned)
    }

    pub fn progress(&self) -> Result<HashMap<BookId, f64>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT book_id, MAX(progress) FROM reading_state GROUP BY book_id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, BookId>(0)?, r.get::<_, f64>(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn state(&self, book: BookId) -> Result<Option<ReadingState>> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                "SELECT book_id, format, position, progress, updated FROM reading_state
                 WHERE book_id = ?1 ORDER BY updated DESC, rowid DESC LIMIT 1",
                [book],
                |r| {
                    Ok(ReadingState {
                        book_id: r.get(0)?,
                        format: r.get(1)?,
                        position: r.get(2)?,
                        progress: r.get(3)?,
                        updated: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn set_state(&self, state: &ReadingState) -> Result<()> {
        let conn = self.conn()?;
        let progress = if state.progress.is_finite() {
            state.progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let updated = (!state.updated.is_empty()).then_some(state.updated.as_str());
        conn.execute(
            &format!(
                "INSERT INTO reading_state (book_id, format, position, progress, updated)
                 VALUES (?1, ?2, ?3, ?4, COALESCE(?5, {NOW}))
                 ON CONFLICT(book_id, format) DO UPDATE SET
                   position = excluded.position,
                   progress = excluded.progress,
                   updated = excluded.updated"
            ),
            params![
                state.book_id,
                state.format.to_uppercase(),
                state.position,
                progress,
                updated
            ],
        )?;
        Ok(())
    }

    pub fn annotations(&self, book: BookId) -> Result<Vec<Annotation>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, book_id, kind, position, text, note, color, chapter, created, updated
             FROM annotation WHERE book_id = ?1 ORDER BY created, rowid",
        )?;
        let rows = stmt.query_map([book], annotation_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_annotation(&self, a: &Annotation) -> Result<()> {
        if a.id.is_empty() {
            return Err(Error::Invalid("annotation id is empty".into()));
        }
        let conn = self.conn()?;
        let created = (!a.created.is_empty()).then_some(a.created.as_str());
        let updated = (!a.updated.is_empty()).then_some(a.updated.as_str());
        conn.execute(
            &format!(
                "INSERT INTO annotation
                   (id, book_id, kind, position, text, note, color, chapter, created, updated)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                         COALESCE(?9, {NOW}), COALESCE(?10, ?9, {NOW}))
                 ON CONFLICT(id) DO UPDATE SET
                   book_id = excluded.book_id,
                   kind = excluded.kind,
                   position = excluded.position,
                   text = excluded.text,
                   note = excluded.note,
                   color = excluded.color,
                   chapter = excluded.chapter,
                   created = excluded.created,
                   updated = excluded.updated"
            ),
            params![
                a.id,
                a.book_id,
                kind_to_str(a.kind),
                a.position,
                a.text,
                a.note,
                a.color,
                a.chapter,
                created,
                updated
            ],
        )?;
        Ok(())
    }

    pub fn delete_annotation(&self, id: &str) -> Result<()> {
        self.conn()?
            .execute("DELETE FROM annotation WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_book(&self, book: BookId) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM reading_state WHERE book_id = ?1", [book])?;
        tx.execute("DELETE FROM annotation WHERE book_id = ?1", [book])?;
        tx.commit()?;
        Ok(())
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(Error::SchemaVersion(version));
    }
    if version < 1 {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE reading_state (
               book_id  INTEGER NOT NULL,
               format   TEXT    NOT NULL,
               position TEXT,
               progress REAL    NOT NULL DEFAULT 0,
               updated  TEXT    NOT NULL,
               PRIMARY KEY (book_id, format)
             );
             CREATE TABLE annotation (
               id       TEXT PRIMARY KEY,
               book_id  INTEGER NOT NULL,
               kind     TEXT NOT NULL,
               position TEXT NOT NULL,
               text     TEXT,
               note     TEXT,
               color    TEXT,
               chapter  TEXT,
               created  TEXT NOT NULL,
               updated  TEXT NOT NULL
             );
             CREATE INDEX annotation_book ON annotation (book_id, created);
             PRAGMA user_version = 1;
             COMMIT;",
        )?;
    }
    Ok(())
}

fn kind_to_str(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Highlight => "highlight",
        AnnotationKind::Bookmark => "bookmark",
    }
}

fn annotation_from_row(r: &Row) -> rusqlite::Result<Annotation> {
    let kind: String = r.get(2)?;
    Ok(Annotation {
        id: r.get(0)?,
        book_id: r.get(1)?,
        kind: if kind == "bookmark" {
            AnnotationKind::Bookmark
        } else {
            AnnotationKind::Highlight
        },
        position: r.get(3)?,
        text: r.get(4)?,
        note: r.get(5)?,
        color: r.get(6)?,
        chapter: r.get(7)?,
        created: r.get(8)?,
        updated: r.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempdir::Dir, ReadingStore) {
        let dir = tempdir::Dir::new();
        let store = ReadingStore::open(dir.path()).unwrap();
        (dir, store)
    }

    mod tempdir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};

        static N: AtomicU32 = AtomicU32::new(0);

        pub struct Dir(PathBuf);

        impl Dir {
            pub fn new() -> Self {
                let p = std::env::temp_dir().join(format!(
                    "shelfmark-reading-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::SeqCst)
                ));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    fn state(book: BookId, format: &str, progress: f64, updated: &str) -> ReadingState {
        ReadingState {
            book_id: book,
            format: format.into(),
            position: Some(format!("epubcfi(/6/{progress})")),
            progress,
            updated: updated.into(),
        }
    }

    fn annotation(id: &str, book: BookId, created: &str) -> Annotation {
        Annotation {
            id: id.into(),
            book_id: book,
            kind: AnnotationKind::Highlight,
            position: "epubcfi(/6/4!/4,/1:0,/1:5)".into(),
            text: Some("hello".into()),
            note: None,
            color: Some("yellow".into()),
            chapter: Some("One".into()),
            created: created.into(),
            updated: created.into(),
        }
    }

    #[test]
    fn open_creates_db_and_reopens() {
        let (dir, store) = store();
        assert!(dir.path().join("shelfmark.db").exists());
        store.set_state(&state(1, "EPUB", 0.5, "")).unwrap();
        drop(store);
        let again = ReadingStore::open(dir.path()).unwrap();
        assert_eq!(again.state(1).unwrap().unwrap().progress, 0.5);
    }

    #[test]
    fn rejects_newer_schema() {
        let dir = tempdir::Dir::new();
        {
            let c = Connection::open(dir.path().join("shelfmark.db")).unwrap();
            c.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(
            ReadingStore::open(dir.path()),
            Err(Error::SchemaVersion(99))
        ));
    }

    #[test]
    fn state_upsert_and_latest() {
        let (_d, s) = store();
        assert!(s.state(1).unwrap().is_none());
        s.set_state(&state(1, "EPUB", 0.2, "2026-01-01T00:00:00Z"))
            .unwrap();
        s.set_state(&state(1, "PDF", 0.6, "2026-01-02T00:00:00Z"))
            .unwrap();
        assert_eq!(s.state(1).unwrap().unwrap().format, "PDF");
        s.set_state(&state(1, "EPUB", 0.3, "2026-01-03T00:00:00Z"))
            .unwrap();
        let latest = s.state(1).unwrap().unwrap();
        assert_eq!(latest.format, "EPUB");
        assert_eq!(latest.progress, 0.3);
    }

    #[test]
    fn empty_updated_is_filled() {
        let (_d, s) = store();
        s.set_state(&state(1, "EPUB", 0.1, "")).unwrap();
        let st = s.state(1).unwrap().unwrap();
        assert!(st.updated.starts_with("20") && st.updated.ends_with('Z'));
    }

    #[test]
    fn progress_is_max_per_book() {
        let (_d, s) = store();
        s.set_state(&state(1, "EPUB", 0.2, "")).unwrap();
        s.set_state(&state(1, "PDF", 0.7, "")).unwrap();
        s.set_state(&state(2, "EPUB", 1.0, "")).unwrap();
        let p = s.progress().unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[&1], 0.7);
        assert_eq!(p[&2], 1.0);
    }

    #[test]
    fn progress_is_clamped() {
        let (_d, s) = store();
        s.set_state(&state(1, "EPUB", 3.0, "")).unwrap();
        s.set_state(&state(2, "EPUB", f64::NAN, "")).unwrap();
        let p = s.progress().unwrap();
        assert_eq!(p[&1], 1.0);
        assert_eq!(p[&2], 0.0);
    }

    #[test]
    fn annotations_crud_ordered_by_created() {
        let (_d, s) = store();
        s.save_annotation(&annotation("b", 1, "2026-01-02T00:00:00Z"))
            .unwrap();
        s.save_annotation(&annotation("a", 1, "2026-01-01T00:00:00Z"))
            .unwrap();
        s.save_annotation(&annotation("c", 2, "2026-01-01T00:00:00Z"))
            .unwrap();
        let ids: Vec<_> = s
            .annotations(1)
            .unwrap()
            .into_iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(ids, ["a", "b"]);

        let mut a = annotation("a", 1, "2026-01-01T00:00:00Z");
        a.note = Some("note".into());
        a.kind = AnnotationKind::Bookmark;
        s.save_annotation(&a).unwrap();
        let got = s.annotations(1).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], a);

        s.delete_annotation("a").unwrap();
        s.delete_annotation("missing").unwrap();
        assert_eq!(s.annotations(1).unwrap().len(), 1);
    }

    #[test]
    fn empty_timestamps_are_filled() {
        let (_d, s) = store();
        s.save_annotation(&annotation("a", 1, "")).unwrap();
        let got = s.annotations(1).unwrap().remove(0);
        assert!(!got.created.is_empty());
        assert_eq!(got.created, got.updated);
    }

    #[test]
    fn empty_id_rejected() {
        let (_d, s) = store();
        assert!(s.save_annotation(&annotation("", 1, "")).is_err());
    }

    #[test]
    fn delete_book_removes_everything() {
        let (_d, s) = store();
        s.set_state(&state(1, "EPUB", 0.2, "")).unwrap();
        s.set_state(&state(2, "EPUB", 0.2, "")).unwrap();
        s.save_annotation(&annotation("a", 1, "")).unwrap();
        s.save_annotation(&annotation("b", 2, "")).unwrap();
        s.delete_book(1).unwrap();
        assert!(s.state(1).unwrap().is_none());
        assert!(s.annotations(1).unwrap().is_empty());
        assert!(s.state(2).unwrap().is_some());
        assert_eq!(s.annotations(2).unwrap().len(), 1);
    }

    #[test]
    fn store_is_send_sync() {
        fn check<T: Send + Sync>() {}
        check::<ReadingStore>();
    }
}
