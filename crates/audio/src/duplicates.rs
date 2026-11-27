use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};
use crate::group::stem_asin;
use crate::names::{file_name, parent, split_ext};
use crate::scan::iso;
use crate::store::AudioStore;
use crate::tags::TagReader;
use crate::walk::{AUDIO_EXTENSIONS, mtime_secs};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    pub path: String,
    pub full_path: String,
    pub in_library: bool,
    pub size: u64,
    pub modified: String,
    pub format: String,
    pub duration_ms: Option<u64>,
    pub bitrate: Option<u32>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub genres: Vec<String>,
    pub release_date: Option<String>,
    pub asin: Option<String>,
    pub has_cover: bool,
    pub error: Option<String>,
}

pub fn set_key(root_id: i64, path: &str) -> Option<String> {
    let asin = stem_asin(split_ext(file_name(path)).0)?;
    Some(format!("{root_id}|{}|{asin}", parent(path)))
}

pub fn resolve(store: &AudioStore, root_id: i64, rel: &str) -> Result<PathBuf> {
    let plain = Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    if rel.is_empty() || !plain {
        return Err(Error::Invalid(format!("not a path inside the location: {rel}")));
    }
    Ok(store.root_dir(&store.root(root_id)?).join(rel))
}

pub fn siblings(store: &AudioStore, root_id: i64, rel: &str) -> Result<Vec<String>> {
    let full = resolve(store, root_id, rel)?;
    let dir = parent(rel);
    let Some(asin) = stem_asin(split_ext(file_name(rel)).0) else {
        return Ok(vec![rel.to_string()]);
    };
    let mut out = Vec::new();
    for entry in std::fs::read_dir(full.parent().unwrap_or(&full))?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let (stem, ext) = split_ext(&name);
        if !AUDIO_EXTENSIONS.contains(&ext.as_str())
            || stem_asin(stem).as_deref() != Some(asin.as_str())
            || !entry.file_type().is_ok_and(|t| t.is_file())
        {
            continue;
        }
        out.push(if dir.is_empty() {
            name
        } else {
            format!("{dir}/{name}")
        });
    }
    out.sort();
    Ok(out)
}

pub fn copies(
    store: &AudioStore,
    root_id: i64,
    rel: &str,
    reader: &dyn TagReader,
) -> Result<Vec<Copy>> {
    let indexed = store.indexed_files(root_id)?;
    let mut out = Vec::new();
    for path in siblings(store, root_id, rel)? {
        let full = resolve(store, root_id, &path)?;
        let meta = std::fs::metadata(&full)?;
        let mut copy = Copy {
            full_path: full.to_string_lossy().into_owned(),
            in_library: indexed.contains_key(&path),
            size: meta.len(),
            modified: iso(mtime_secs(&meta)),
            format: split_ext(file_name(&path)).1.to_uppercase(),
            path,
            ..Default::default()
        };
        match reader.read(&full, true) {
            Ok(t) => {
                if t.duration_ms > 0 {
                    copy.duration_ms = Some(t.duration_ms);
                    copy.bitrate = Some((copy.size * 8 / t.duration_ms) as u32);
                }
                copy.title = t.title;
                copy.album = t.album;
                copy.subtitle = t.subtitle;
                copy.authors = t.authors;
                copy.narrators = t.narrators;
                copy.series = t.series;
                copy.series_index = t.series_index;
                copy.publisher = t.publisher;
                copy.language = t.language;
                copy.genres = t.genres;
                copy.release_date = t.date;
                copy.asin = t.asin;
                copy.has_cover = t.cover.is_some();
            }
            Err(e) => copy.error = Some(e.to_string()),
        }
        out.push(copy);
    }
    Ok(out)
}
