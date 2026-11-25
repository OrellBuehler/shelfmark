use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use shelfmark_model::{AudioChapter, AudiobookId};

use crate::error::{Error, Result};
use crate::group::{Group, group};
use crate::names::{file_name, leading_number, parent, split_ext, strip_track, title_from};
use crate::sidecar;
use crate::store::{AudioStore, Issue, ScannedBook, ScannedFile};
use crate::tags::{FileTags, TagReader};
use crate::walk::{Kind, walk};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const BATCH: usize = 25;
const WORKERS: usize = 4;
const THUMB_SIZE: u32 = 500;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanProgress {
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
    pub current: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub audiobooks: u32,
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub missing: u32,
    pub issues: u32,
    pub complete: bool,
}

struct Built {
    book: ScannedBook,
    cover: Option<(Vec<u8>, &'static str, i64)>,
    chapters: Vec<AudioChapter>,
    issues: Vec<Issue>,
}

pub fn iso(secs: i64) -> String {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn probe_root(dir: &Path) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let path = dir.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(std::fs::read_dir(&path).map(|_| ()));
    });
    match rx.recv_timeout(PROBE_TIMEOUT) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(Error::Invalid(format!(
            "location is not reachable: {} ({e})",
            dir.display()
        ))),
        Err(_) => Err(Error::Invalid(format!(
            "location did not respond: {}",
            dir.display()
        ))),
    }
}

pub fn thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > THUMB_SIZE || img.height() > THUMB_SIZE {
        img.thumbnail(THUMB_SIZE, THUMB_SIZE)
    } else {
        img
    };
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
    img.to_rgb8().write_with_encoder(encoder).ok()?;
    Some(out)
}

fn fill(base: &mut FileTags, extra: &FileTags) {
    macro_rules! opt {
        ($($f:ident),*) => {$(
            if base.$f.is_none() { base.$f = extra.$f.clone(); }
        )*};
    }
    macro_rules! list {
        ($($f:ident),*) => {$(
            if base.$f.is_empty() { base.$f = extra.$f.clone(); }
        )*};
    }
    opt!(
        title, album, subtitle, series, series_index, publisher, language, description, asin,
        isbn, date
    );
    list!(authors, narrators, genres);
}

fn build(root: &Path, g: &Group, reader: &dyn TagReader) -> Built {
    let mut issues = Vec::new();
    let mut files = Vec::new();
    let mut meta: Option<FileTags> = None;
    let mut embedded_cover: Option<(Vec<u8>, i64)> = None;
    let mut duration = 0u64;
    let sidecar_cover = g.cover();
    let stems: Vec<&str> = g
        .audio
        .iter()
        .map(|e| split_ext(file_name(&e.rel)).0)
        .collect();
    let prefix = crate::names::common_prefix(&stems);
    for (i, e) in g.audio.iter().enumerate() {
        let want_cover = sidecar_cover.is_none() && embedded_cover.is_none();
        let path = root.join(&e.rel);
        let tags = match reader.read(&path, want_cover) {
            Ok(t) => Some(t),
            Err(err) => {
                issues.push(Issue {
                    path: e.rel.clone(),
                    message: err.to_string(),
                });
                None
            }
        };
        let stem = stems[i];
        let track = tags
            .as_ref()
            .and_then(|t| t.track)
            .or_else(|| leading_number(stem))
            .or_else(|| crate::names::number_after(stem, prefix))
            .unwrap_or(i as u32 + 1);
        files.push(ScannedFile {
            rel_path: e.rel.clone(),
            kind: Kind::Audio.as_str().into(),
            size: e.size,
            mtime: e.mtime,
            duration_ms: tags.as_ref().map(|t| t.duration_ms),
            track: g.multi.then_some(track),
            title: g
                .multi
                .then(|| tags.as_ref().and_then(|t| t.title.clone()))
                .flatten(),
        });
        if let Some(mut t) = tags {
            duration += t.duration_ms;
            if let Some(c) = t.cover.take()
                && embedded_cover.is_none()
            {
                embedded_cover = Some((c, e.mtime));
            }
            if meta.is_none() {
                meta = Some(t);
            }
        }
    }
    for s in &g.sidecars {
        files.push(ScannedFile {
            rel_path: s.rel.clone(),
            kind: s.kind.as_str().into(),
            size: s.size,
            mtime: s.mtime,
            ..Default::default()
        });
    }

    let had_tags = meta
        .as_ref()
        .is_some_and(|t| t.title.is_some() || t.album.is_some() || t.asin.is_some());
    let mut tags = meta.unwrap_or_default();
    let mut chapters = Vec::new();
    let mut had_sidecar = false;
    if let Some(m) = g.sidecars.iter().find(|s| s.kind == Kind::Meta)
        && let Ok(bytes) = std::fs::read(root.join(&m.rel))
        && let Some(side) = sidecar::parse(&bytes)
    {
        had_sidecar = true;
        if duration == 0 {
            duration = side.tags.duration_ms;
        }
        fill(&mut tags, &side.tags);
        chapters = side.chapters;
    }
    if tags.asin.is_none() {
        tags.asin = g.asin.clone();
    }

    let book_dir = if g.multi { g.key.as_str() } else { parent(&g.key) };
    let title = if g.multi {
        tags.album.clone().or_else(|| {
            let name = file_name(book_dir);
            if name.is_empty() {
                tags.title.clone()
            } else {
                Some(title_from(name))
            }
        })
    } else {
        tags.title.clone().or_else(|| tags.album.clone())
    }
    .unwrap_or_else(|| {
        if g.multi {
            strip_track(split_ext(file_name(&g.audio[0].rel)).0)
        } else {
            title_from(file_name(&g.key))
        }
    });
    if tags.authors.is_empty()
        && let Some((first, _)) = book_dir.split_once('/')
    {
        tags.authors = vec![first.to_string()];
    }

    let cover = match sidecar_cover {
        Some(c) => std::fs::read(root.join(&c.rel))
            .ok()
            .and_then(|b| thumbnail(&b))
            .map(|t| (t, "file", c.mtime)),
        None => None,
    }
    .or_else(|| {
        embedded_cover.and_then(|(bytes, mtime)| thumbnail(&bytes).map(|t| (t, "embedded", mtime)))
    });

    let added = g.audio.iter().map(|e| e.mtime).min().unwrap_or(0);
    let modified = g.entries().map(|e| e.mtime).max().unwrap_or(0);
    let format = split_ext(file_name(&g.audio[0].rel)).1.to_uppercase();
    Built {
        book: ScannedBook {
            key: g.key.clone(),
            title,
            subtitle: tags.subtitle,
            authors: tags.authors,
            narrators: tags.narrators,
            series: tags.series,
            series_index: tags.series_index,
            publisher: tags.publisher,
            language: tags.language,
            genres: tags.genres,
            description: tags.description,
            asin: tags.asin,
            isbn: tags.isbn,
            release_date: tags.date,
            duration_ms: duration,
            format,
            meta_source: if had_tags {
                "tags"
            } else if had_sidecar {
                "sidecar"
            } else {
                "path"
            }
            .into(),
            added: iso(added),
            modified: iso(modified),
            files,
        },
        cover,
        chapters,
        issues,
    }
}

pub fn scan(
    store: &AudioStore,
    root_id: i64,
    reader: &dyn TagReader,
    cancel: &AtomicBool,
    progress: &(dyn Fn(ScanProgress) + Sync),
) -> Result<ScanSummary> {
    let root = store.root(root_id)?;
    let dir: PathBuf = store.root_dir(&root);
    if let Err(e) = probe_root(&dir) {
        store.finish_scan(root_id, 0, false)?;
        return Err(e);
    }
    let cancelled = || cancel.load(Ordering::Relaxed);
    progress(ScanProgress {
        phase: "walking",
        ..Default::default()
    });
    let entries = walk(&dir, &cancelled, &|found| {
        if found % 200 == 0 {
            progress(ScanProgress {
                phase: "walking",
                done: found,
                ..Default::default()
            });
        }
    })?;
    let groups = group(&entries);
    let indexed_files = store.indexed_files(root_id)?;
    let indexed_books = store.indexed_books(root_id)?;
    let key_of: HashMap<AudiobookId, &str> =
        indexed_books.iter().map(|b| (b.id, b.key.as_str())).collect();
    let mut counts: HashMap<AudiobookId, usize> = HashMap::new();
    for f in indexed_files.values() {
        *counts.entry(f.audiobook_id).or_default() += 1;
    }
    let scan_gen = store.next_scan_gen(root_id)?;

    let mut summary = ScanSummary {
        audiobooks: groups.len() as u32,
        ..Default::default()
    };
    let mut unchanged = Vec::new();
    let mut changed: Vec<&Group> = Vec::new();
    for g in &groups {
        let mut id = None;
        let mut same = true;
        let mut n = 0;
        for e in g.entries() {
            n += 1;
            match indexed_files.get(&e.rel) {
                Some(f) if f.size == e.size && f.mtime == e.mtime => {
                    if *id.get_or_insert(f.audiobook_id) != f.audiobook_id {
                        same = false;
                    }
                }
                _ => same = false,
            }
            if !same {
                break;
            }
        }
        match id {
            Some(id)
                if same && counts.get(&id) == Some(&n) && key_of.get(&id) == Some(&g.key.as_str()) =>
            {
                unchanged.push(id)
            }
            _ => changed.push(g),
        }
    }
    summary.unchanged = unchanged.len() as u32;
    store.touch_books(scan_gen, &unchanged)?;

    let known_keys: HashSet<&str> = indexed_books.iter().map(|b| b.key.as_str()).collect();
    let current_keys: HashSet<&str> = groups.iter().map(|g| g.key.as_str()).collect();
    let mut movable: HashMap<&str, Vec<AudiobookId>> = HashMap::new();
    for b in &indexed_books {
        if let Some(asin) = b.asin.as_deref()
            && !current_keys.contains(b.key.as_str())
        {
            movable.entry(asin).or_default().push(b.id);
        }
    }

    let total = changed.len();
    let mut issues: Vec<Issue> = Vec::new();
    let mut pending: Vec<Built> = Vec::new();
    let flush = |pending: &mut Vec<Built>,
                     movable: &mut HashMap<&str, Vec<AudiobookId>>,
                     summary: &mut ScanSummary|
     -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        for b in pending.iter() {
            if known_keys.contains(b.book.key.as_str()) {
                summary.updated += 1;
            } else if let Some(asin) = b.book.asin.as_deref()
                && let Some(id) = movable.get_mut(asin).and_then(Vec::pop)
            {
                store.rekey_book(id, &b.book.key)?;
                summary.updated += 1;
            } else {
                summary.added += 1;
            }
        }
        let books: Vec<ScannedBook> = pending.iter().map(|b| b.book.clone()).collect();
        let ids = store.upsert_books(root_id, scan_gen, &books)?;
        for (id, b) in ids.into_iter().zip(pending.drain(..)) {
            if let Some((thumb, source, mtime)) = b.cover {
                store.set_cover(id, &thumb, source, mtime)?;
            }
            if !b.chapters.is_empty() {
                store.set_chapters(id, &b.chapters)?;
            }
        }
        Ok(())
    };

    let next = AtomicUsize::new(0);
    let result: Result<()> = std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel::<Built>();
        for _ in 0..WORKERS.min(total.max(1)) {
            let tx = tx.clone();
            let (next, changed, dir) = (&next, &changed, &dir);
            scope.spawn(move || {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(g) = changed.get(i) else { break };
                    if tx.send(build(dir, g, reader)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let mut done = 0;
        for mut built in rx {
            done += 1;
            progress(ScanProgress {
                phase: "reading",
                done,
                total,
                current: Some(built.book.title.clone()),
            });
            issues.append(&mut built.issues);
            pending.push(built);
            if pending.len() >= BATCH {
                flush(&mut pending, &mut movable, &mut summary)?;
            }
        }
        flush(&mut pending, &mut movable, &mut summary)
    });
    result?;
    if cancelled() {
        return Err(Error::Cancelled);
    }

    summary.complete = !(groups.is_empty() && !indexed_books.is_empty());
    summary.missing = store.finish_scan(root_id, scan_gen, summary.complete)?;
    summary.issues = issues.len() as u32;
    store.set_issues(root_id, &issues)?;
    progress(ScanProgress {
        phase: "done",
        done: total,
        total,
        current: None,
    });
    Ok(summary)
}
