use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use rusqlite::{Connection, OpenFlags};
use shelfmark_model::DeviceInfo;

use crate::time;

const MAX_CLIPPINGS_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VERSION_BYTES: u64 = 4096;

type Stamp = (i64, u64);
type Counts = HashMap<(PathBuf, &'static str), (Stamp, Option<u64>)>;

static COUNTS: LazyLock<Mutex<Counts>> = LazyLock::new(Default::default);

pub fn item(label: &str, value: impl Into<String>) -> DeviceInfo {
    DeviceInfo {
        label: label.into(),
        value: value.into(),
    }
}

fn read_small(path: &Path, max: u64) -> Option<Vec<u8>> {
    let md = fs::metadata(path).ok()?;
    if !md.is_file() || md.len() > max {
        return None;
    }
    fs::read(path).ok()
}

fn read_text(path: &Path) -> Option<String> {
    String::from_utf8(read_small(path, MAX_VERSION_BYTES)?).ok()
}

fn cached_count(path: &Path, kind: &'static str, count: fn(&Path) -> Option<u64>) -> Option<u64> {
    let md = fs::metadata(path).ok()?;
    if !md.is_file() {
        return None;
    }
    let stamp = (md.modified().map(time::secs).unwrap_or(0), md.len());
    let key = (path.to_path_buf(), kind);
    if let Some((s, v)) = COUNTS.lock().ok()?.get(&key)
        && *s == stamp
    {
        return *v;
    }
    let v = count(path);
    if let Ok(mut m) = COUNTS.lock() {
        m.insert(key, (stamp, v));
    }
    v
}

pub fn kobo_version(root: &Path) -> Vec<String> {
    read_text(&root.join(".kobo").join("version"))
        .map(|t| t.trim().split(',').map(|f| f.trim().to_string()).collect())
        .unwrap_or_default()
}

pub fn kobo(root: &Path, model: Option<&str>) -> Vec<DeviceInfo> {
    let fields = kobo_version(root);
    let mut out = Vec::new();
    if let Some(m) = model {
        out.push(item("Model", m));
    }
    if let Some(fw) = fields.get(2).filter(|s| !s.is_empty()) {
        out.push(item("Firmware", fw.as_str()));
    }
    if let Some(sn) = fields.first().filter(|s| !s.is_empty()) {
        out.push(item("Serial number", sn.as_str()));
    }
    out
}

fn kindle_firmware(root: &Path) -> Option<String> {
    let text = read_text(&root.join("system").join("version.txt"))?;
    let line = text.lines().next()?.trim();
    let fw = line.strip_prefix("Kindle").unwrap_or(line).trim();
    (!fw.is_empty() && fw.len() <= 64 && !fw.chars().any(char::is_control)).then(|| fw.to_string())
}

fn kindle_serial(root: &Path) -> Option<String> {
    let bytes = read_small(&root.join("system").join("AudibleActivation.sys"), 1 << 20)?;
    let serial = bytes.get(10..26)?;
    serial
        .iter()
        .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
        .then(|| String::from_utf8_lossy(serial).into_owned())
}

fn sqlite_uri(path: &Path) -> String {
    let mut s = String::from("file:");
    let raw = path.to_string_lossy().replace('\\', "/");
    if !raw.starts_with('/') {
        s.push('/');
    }
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~:".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s.push_str("?mode=ro&immutable=1");
    s
}

fn count_vocabulary(path: &Path) -> Option<u64> {
    let conn = Connection::open_with_flags(
        sqlite_uri(path),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .ok()?;
    conn.query_row("SELECT COUNT(*) FROM WORDS", [], |r| r.get::<_, i64>(0))
        .ok()
        .map(|n| n.max(0) as u64)
}

fn count_clippings(path: &Path) -> Option<u64> {
    let bytes = read_small(path, MAX_CLIPPINGS_BYTES)?;
    let n = bytes
        .split(|&b| b == b'\n')
        .filter(|l| l.trim_ascii() == b"==========")
        .count();
    Some(n as u64)
}

pub fn kindle(root: &Path, docs: &Path) -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    if let Some(fw) = kindle_firmware(root) {
        out.push(item("Firmware", fw));
    }
    if let Some(sn) = kindle_serial(root) {
        out.push(item("Serial number", sn));
    }
    let vocab = root.join("system").join("vocabulary").join("vocab.db");
    if let Some(n) = cached_count(&vocab, "vocabulary", count_vocabulary) {
        out.push(item("Vocabulary words", n.to_string()));
    }
    let clippings = docs.join("My Clippings.txt");
    if let Some(n) = cached_count(&clippings, "clippings", count_clippings) {
        out.push(item("Clippings", n.to_string()));
    }
    out
}

pub fn file_system(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    Some(
        match raw.to_ascii_lowercase().as_str() {
            "vfat" | "fat32" => "FAT32",
            "msdos" | "fat" => "FAT",
            "exfat" => "exFAT",
            "ntfs" | "ntfs3" => "NTFS",
            "hfs" | "hfsplus" => "HFS+",
            "apfs" => "APFS",
            _ => raw,
        }
        .to_string(),
    )
}

pub fn insert_file_system(info: &mut Vec<DeviceInfo>, fs: String) {
    let at = info
        .iter()
        .position(|i| !matches!(i.label.as_str(), "Model" | "Firmware" | "Serial number"))
        .unwrap_or(info.len());
    info.insert(at, item("File system", fs));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_system_names() {
        assert_eq!(file_system("vfat").as_deref(), Some("FAT32"));
        assert_eq!(file_system("exFAT").as_deref(), Some("exFAT"));
        assert_eq!(file_system("ext4").as_deref(), Some("ext4"));
        assert_eq!(file_system(" "), None);
    }

    #[test]
    fn file_system_goes_after_identity() {
        let mut v = vec![item("Firmware", "1"), item("Clippings", "2")];
        insert_file_system(&mut v, "FAT32".into());
        let labels: Vec<_> = v.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Firmware", "File system", "Clippings"]);
    }

    #[test]
    fn sqlite_uri_escapes() {
        assert_eq!(
            sqlite_uri(Path::new("E:\\a b\\v.db")),
            "file:/E:/a%20b/v.db?mode=ro&immutable=1"
        );
        assert_eq!(
            sqlite_uri(Path::new("/media/x#y/v.db")),
            "file:/media/x%23y/v.db?mode=ro&immutable=1"
        );
    }
}
