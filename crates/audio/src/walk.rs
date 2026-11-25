use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::names::split_ext;

pub const AUDIO_EXTENSIONS: &[&str] = &["m4b", "m4a", "mp3", "ogg", "opus", "flac"];
const SKIP_DIRS: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "@eadir",
    "#recycle",
    "lost+found",
];
const SETTLE_SECS: i64 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Cover,
    Pdf,
    Cue,
    Meta,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Audio => "audio",
            Kind::Cover => "cover",
            Kind::Pdf => "pdf",
            Kind::Cue => "cue",
            Kind::Meta => "meta",
        }
    }
}

pub fn kind_of(ext: &str) -> Option<Kind> {
    Some(match ext {
        e if AUDIO_EXTENSIONS.contains(&e) => Kind::Audio,
        "jpg" | "jpeg" | "png" => Kind::Cover,
        "pdf" => Kind::Pdf,
        "cue" => Kind::Cue,
        "metadata.json" => Kind::Meta,
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub rel: String,
    pub kind: Kind,
    pub size: u64,
    pub mtime: i64,
}

pub fn mtime_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

pub fn walk(
    root: &Path,
    cancelled: &dyn Fn() -> bool,
    on_found: &dyn Fn(usize),
) -> Result<Vec<Entry>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let mut out = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, String)> = vec![(root.to_path_buf(), String::new())];
    let mut first = true;
    while let Some((dir, rel)) = stack.pop() {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if first => return Err(e.into()),
            Err(_) => continue,
        };
        first = false;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let child = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if file_type.is_dir() {
                let lower = name.to_lowercase();
                if name.starts_with('.') || SKIP_DIRS.contains(&lower.as_str()) {
                    continue;
                }
                stack.push((entry.path(), child));
                continue;
            }
            if !file_type.is_file() || name.starts_with('.') {
                continue;
            }
            let (_, ext) = split_ext(&name);
            let Some(kind) = kind_of(&ext) else { continue };
            let Ok(meta) = entry.metadata() else { continue };
            let mtime = mtime_secs(&meta);
            if kind == Kind::Audio && (meta.len() == 0 || now - mtime < SETTLE_SECS) {
                continue;
            }
            out.push(Entry {
                rel: child,
                kind,
                size: meta.len(),
                mtime,
            });
            if kind == Kind::Audio {
                on_found(out.len());
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}
