use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use shelfmark_model::{Device, DeviceBook, DeviceKind, Metadata};

use crate::cache::{Cache, Entry};
use crate::detect::space;
use crate::error::{Error, Result};
use crate::time;

pub type MetaReader<'a> = &'a dyn Fn(&Path) -> Option<Metadata>;

const SKIP_DIRS: &[&str] = &[
    "system",
    "system volume information",
    "$recycle.bin",
    "lost+found",
];
const KINDLE_SIDECARS: &[&str] = &[
    ".sdr", ".mbp", ".tan", ".pdr", ".ea", ".apnx", ".phl", ".mbp1", ".mbs", ".han",
];
const MAX_DEPTH: usize = 16;
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn supports(device: &Device, format: &str) -> bool {
    device.supported_formats.iter().any(|f| f == format)
}

fn format_of(device: &Device, name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let format = if lower.ends_with(".kepub.epub") && supports(device, "KEPUB") {
        "KEPUB".to_string()
    } else {
        Path::new(&lower)
            .extension()?
            .to_str()?
            .to_ascii_uppercase()
    };
    supports(device, &format).then_some(format)
}

fn rel_string(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn mtime_secs(md: &fs::Metadata) -> i64 {
    md.modified().map(time::secs).unwrap_or(0)
}

fn skipped_dir(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.starts_with('.') || lower.ends_with(".sdr") || SKIP_DIRS.contains(&lower.as_str())
}

fn fallback_title(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let cut = if lower.ends_with(".kepub.epub") {
        name.len() - ".kepub.epub".len()
    } else {
        name.rfind('.').unwrap_or(name.len())
    };
    name[..cut].to_string()
}

pub fn list_books(device: &Device, read_meta: MetaReader) -> Result<Vec<DeviceBook>> {
    let root = PathBuf::from(&device.root);
    if !root.is_dir() {
        return Err(Error::NotAFolder(root));
    }
    let start = if device.kind == DeviceKind::Kindle {
        PathBuf::from(&device.books_dir)
    } else {
        root.clone()
    };

    let mut cache = Cache::load(&root);
    let mut seen = HashSet::new();
    let mut books = Vec::new();
    let mut stack = vec![(start, 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                if depth < MAX_DEPTH && !skipped_dir(&name) {
                    stack.push((entry.path(), depth + 1));
                }
                continue;
            }
            if !ft.is_file() || name.starts_with('.') {
                continue;
            }
            let Some(format) = format_of(device, &name) else {
                continue;
            };
            let path = entry.path();
            let Ok(md) = entry.metadata() else { continue };
            let Ok(rel) = path.strip_prefix(&root) else {
                continue;
            };
            let rel = rel_string(rel);
            let (size, mtime) = (md.len(), mtime_secs(&md));
            let cached = cache.get(&rel, size, mtime).cloned();
            let e = match cached {
                Some(e) => e,
                None => {
                    let meta = read_meta(&path);
                    let e = Entry {
                        size,
                        mtime,
                        title: meta
                            .as_ref()
                            .and_then(|m| m.title.clone())
                            .filter(|t| !t.trim().is_empty()),
                        authors: meta.as_ref().map(|m| m.authors.clone()).unwrap_or_default(),
                        uuid: meta.and_then(|m| m.uuid),
                    };
                    cache.put(rel.clone(), e.clone());
                    e
                }
            };
            books.push(DeviceBook {
                path: path.to_string_lossy().into_owned(),
                rel_path: rel.clone(),
                title: e.title.unwrap_or_else(|| fallback_title(&name)),
                authors: e.authors,
                format,
                size,
                modified: Some(time::iso8601(mtime)),
                uuid: e.uuid,
            });
            seen.insert(rel);
        }
    }

    cache.retain_only(&seen);
    cache.save(&root);
    books.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(books)
}

fn sanitize(s: &str, max_chars: usize, fallback: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let collapsed = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(max_chars).collect();
    let mut out = truncated.trim_matches([' ', '.']).to_string();
    if out.is_empty() {
        out = fallback.to_string();
    }
    let base = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&base.as_str()) {
        out.insert(0, '_');
    }
    out
}

fn sync_dir(_dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(_dir) {
        let _ = d.sync_all();
    }
}

pub fn send_book(device: &Device, src: &Path, meta: &Metadata, format: &str) -> Result<DeviceBook> {
    let root = PathBuf::from(&device.root);
    let books_dir = PathBuf::from(&device.books_dir);
    if !books_dir.starts_with(&root) {
        return Err(Error::Invalid("books folder is outside the device".into()));
    }
    let src_md = fs::metadata(src)?;
    if !src_md.is_file() {
        return Err(Error::NotFound(src.to_path_buf()));
    }

    let src_name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut format = format.trim().trim_start_matches('.').to_ascii_uppercase();
    if format == "EPUB" && src_name.ends_with(".kepub.epub") && supports(device, "KEPUB") {
        format = "KEPUB".into();
    }
    if !supports(device, &format) {
        return Err(Error::UnsupportedFormat {
            format,
            device: device.name.clone(),
        });
    }
    let ext = if format == "KEPUB" {
        "kepub.epub".to_string()
    } else {
        format.to_ascii_lowercase()
    };

    let stem = src
        .file_name()
        .map(|n| fallback_title(&n.to_string_lossy()))
        .unwrap_or_default();
    let title_raw = meta
        .title
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(&stem);
    let author_raw = meta.authors.first().map(String::as_str).unwrap_or("");
    let author_dir = sanitize(author_raw, 60, "Unknown");
    let title = sanitize(title_raw, 100, "Unknown");
    let file_name = format!("{title} - {author_dir}.{ext}");

    let dir = books_dir.join(&author_dir);
    let target = dir.join(&file_name);

    let existing = fs::metadata(&target)
        .ok()
        .filter(|m| m.is_file())
        .map_or(0, |m| m.len());
    let needed = src_md.len().saturating_sub(existing);
    let (total, free) = space(&root);
    if total > 0 && needed > free {
        return Err(Error::NoSpace { needed, free });
    }

    fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!(".{file_name}.shelfmark-part"));
    let written = (|| -> io::Result<()> {
        let mut input = fs::File::open(src)?;
        let mut out = fs::File::create(&tmp)?;
        io::copy(&mut input, &mut out)?;
        out.flush()?;
        out.sync_all()?;
        drop(out);
        if fs::rename(&tmp, &target).is_err() {
            if target.is_file() {
                fs::remove_file(&target)?;
            }
            fs::rename(&tmp, &target)?;
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    sync_dir(&dir);

    let md = fs::metadata(&target)?;
    let rel = rel_string(target.strip_prefix(&root).unwrap_or(&target));
    let (size, mtime) = (md.len(), mtime_secs(&md));
    let mut cache = Cache::load(&root);
    cache.put(
        rel.clone(),
        Entry {
            size,
            mtime,
            title: Some(title_raw.to_string()),
            authors: meta.authors.clone(),
            uuid: meta.uuid.clone(),
        },
    );
    cache.save(&root);

    Ok(DeviceBook {
        path: target.to_string_lossy().into_owned(),
        rel_path: rel,
        title: title_raw.to_string(),
        authors: meta.authors.clone(),
        format,
        size,
        modified: Some(time::iso8601(mtime)),
        uuid: meta.uuid.clone(),
    })
}

pub fn delete_book(device: &Device, path: &Path) -> Result<()> {
    let root = fs::canonicalize(&device.root)?;
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(&device.root).join(path)
    };
    let file = fs::canonicalize(&full).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => Error::NotFound(full.clone()),
        _ => Error::Io(e),
    })?;
    if file == root || !file.starts_with(&root) {
        return Err(Error::OutsideDevice(path.to_path_buf()));
    }
    if !file.is_file() {
        return Err(Error::Invalid(format!("not a file: {}", path.display())));
    }
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let rel_comps = file.strip_prefix(&root).unwrap_or(&file);
    if name.starts_with('.')
        || rel_comps.components().any(|c| match c {
            Component::Normal(s) => skipped_dir(&s.to_string_lossy()),
            _ => false,
        })
        || format_of(device, &name).is_none()
    {
        return Err(Error::Invalid(format!(
            "not a book on this device: {}",
            path.display()
        )));
    }
    let rel = rel_string(rel_comps);

    fs::remove_file(&file)?;

    if device.kind == DeviceKind::Kindle {
        let base = file.with_extension("");
        for ext in KINDLE_SIDECARS {
            let mut s = base.clone().into_os_string();
            s.push(ext);
            let side = PathBuf::from(s);
            if side.is_dir() {
                let _ = fs::remove_dir_all(&side);
            } else if side.is_file() {
                let _ = fs::remove_file(&side);
            }
        }
    }

    let books_dir = fs::canonicalize(&device.books_dir).unwrap_or_else(|_| root.clone());
    let mut dir = file.parent().map(Path::to_path_buf);
    while let Some(d) = dir {
        if d == root || d == books_dir || !d.starts_with(&root) || fs::remove_dir(&d).is_err() {
            break;
        }
        dir = d.parent().map(Path::to_path_buf);
    }

    let mut cache = Cache::load(&root);
    cache.remove(&rel);
    cache.save(&root);
    Ok(())
}
