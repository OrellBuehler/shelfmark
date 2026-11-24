use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use shelfmark_model::Metadata;

use crate::error::Result;

pub fn sniff_image(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if data.starts_with(b"GIF8") {
        Some("image/gif")
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub fn image_ext(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    }
}

pub fn is_image_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp"]
        .iter()
        .any(|e| l.ends_with(e))
}

pub fn file_name_lower(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn extension(path: &Path) -> String {
    let name = file_name_lower(path);
    if name.ends_with(".kepub.epub") {
        return "kepub".into();
    }
    name.rsplit_once('.')
        .map(|(_, e)| e.to_string())
        .unwrap_or_default()
}

pub fn clean(s: &str) -> Option<String> {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then_some(t)
}

pub fn push_unique(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

pub fn normalize_date(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let digits = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
    let head = s.split(['T', ' ']).next().unwrap_or(s);
    let parts: Vec<&str> = head.split('-').collect();
    if digits(parts[0])
        && parts[0].len() == 4
        && parts.iter().all(|p| digits(p))
        && parts.len() <= 3
    {
        let y: u32 = parts[0].parse().ok()?;
        if y < 1000 {
            return None;
        }
        let m: u32 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(1);
        let d: u32 = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(1);
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        if s.len() > head.len() && s.contains('T') && parts.len() == 3 {
            return Some(s.to_string());
        }
        return Some(format!("{y:04}-{m:02}-{d:02}"));
    }
    let lower = s.to_lowercase().replace(',', " ");
    let toks: Vec<&str> = lower.split_whitespace().collect();
    let month_of = |t: &str| {
        MONTHS
            .iter()
            .position(|m| t.starts_with(m))
            .map(|i| i as u32 + 1)
    };
    let year_of = |t: &str| {
        (digits(t) && t.len() == 4)
            .then(|| t.parse::<u32>().ok())
            .flatten()
    };
    let day_of = |t: &str| {
        let t = t.trim_end_matches(['.', 's', 't', 'n', 'd', 'r', 'h']);
        digits(t)
            .then(|| t.parse::<u32>().ok())
            .flatten()
            .filter(|d| (1..=31).contains(d))
    };
    let (y, m, d) = match toks.as_slice() {
        [a, b, y] => match month_of(a) {
            Some(m) => (year_of(y)?, m, day_of(b)?),
            None => (year_of(y)?, month_of(b)?, day_of(a)?),
        },
        [m, y] => (year_of(y)?, month_of(m)?, 1),
        _ => return None,
    };
    (y >= 1000).then(|| format!("{y:04}-{m:02}-{d:02}"))
}

pub fn parse_series(s: &str) -> Option<(String, Option<f64>)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    for sep in [" ; ", ";", " #", ", #", ", Book ", ", book ", " - Book "] {
        if let Some((name, idx)) = s.rsplit_once(sep)
            && let Ok(n) = idx.trim().trim_start_matches('#').parse::<f64>()
        {
            let name = name.trim().trim_end_matches(',').trim();
            if !name.is_empty() {
                return Some((name.to_string(), Some(n)));
            }
        }
    }
    if let Some(stripped) = s.strip_suffix(')')
        && let Some((name, idx)) = stripped.rsplit_once('(')
        && let Ok(n) = idx.trim().trim_start_matches('#').parse::<f64>()
    {
        return Some((name.trim().to_string(), Some(n)));
    }
    Some((s.to_string(), None))
}

pub fn from_file_name(path: &Path) -> Metadata {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_lowercase();
    let stem = if lower.ends_with(".kepub.epub") {
        &name[..name.len() - 11]
    } else {
        name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s)
    };
    let mut meta = Metadata::default();
    let (title, authors) = match stem.rsplit_once(" - ") {
        Some((t, a)) if !t.trim().is_empty() && !a.trim().is_empty() && !a.contains('_') => {
            (t, Some(a))
        }
        _ => (stem, None),
    };
    meta.title = clean(title).or_else(|| clean(stem));
    if let Some(a) = authors {
        for part in a.split(" & ").flat_map(|p| p.split(" and ")) {
            if let Some(p) = clean(part) {
                push_unique(&mut meta.authors, p);
            }
        }
    }
    meta
}

pub fn fill_from_file_name(meta: &mut Metadata, path: &Path) {
    if meta.title.as_deref().is_none_or(|t| t.trim().is_empty()) {
        let f = from_file_name(path);
        meta.title = f.title;
        if meta.authors.is_empty() {
            meta.authors = f.authors;
        }
    }
}

pub fn base64_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

#[cfg(test)]
pub fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn random_uuid() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        chunk.copy_from_slice(&h.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.to_lowercase().chars() {
            let d = c.is_ascii_digit();
            match out.last_mut() {
                Some((ld, t)) if *ld == d => t.push(c),
                _ => out.push((d, c.to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let o = if x.0 && y.0 {
            let (nx, ny) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
            nx.len().cmp(&ny.len()).then(nx.cmp(ny))
        } else {
            x.1.cmp(&y.1)
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    ca.len().cmp(&cb.len())
}

pub fn atomic_write(path: &Path, write: impl FnOnce(&mut fs::File) -> Result<()>) -> Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp: PathBuf = path.with_file_name(format!(".{name}.shelfmark.tmp"));
    let result = (|| -> Result<()> {
        let mut f = fs::File::create(&tmp)?;
        write(&mut f)?;
        f.flush()?;
        f.sync_all()?;
        if let Ok(meta) = fs::metadata(path) {
            let _ = fs::set_permissions(&tmp, meta.permissions());
        }
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
