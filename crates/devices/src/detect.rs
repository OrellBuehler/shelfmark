use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use shelfmark_model::{Device, DeviceKind};

use crate::error::{Error, Result};
use crate::info;

const KOBO_FORMATS: &[&str] = &["KEPUB", "EPUB", "PDF", "MOBI", "CBZ", "CBR", "TXT"];
const KINDLE_FORMATS: &[&str] = &[
    "AZW3", "MOBI", "AZW", "PRC", "AZW1", "TPZ", "AZW4", "KFX", "POBI", "PDF", "TXT",
];
const TOLINO_FORMATS: &[&str] = &["EPUB", "PDF", "TXT"];
const POCKETBOOK_FORMATS: &[&str] = &[
    "EPUB", "PDF", "FB2", "MOBI", "DJVU", "CBZ", "CBR", "TXT", "RTF", "DOCX", "DOC", "AZW3",
];
const GENERIC_FORMATS: &[&str] = &[
    "EPUB", "KEPUB", "PDF", "MOBI", "AZW3", "AZW", "FB2", "DJVU", "CBZ", "CBR", "TXT", "RTF",
];

const GENERIC_MARKERS: &[&str] = &[
    ".shelfmark-device",
    ".calibre",
    "metadata.calibre",
    ".metadata.calibre",
    "driveinfo.calibre",
];

const SKIP_FSTYPES: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "devpts",
    "tmpfs",
    "cgroup",
    "cgroup2",
    "overlay",
    "squashfs",
    "tracefs",
    "debugfs",
    "securityfs",
    "pstore",
    "bpf",
    "autofs",
    "mqueue",
    "hugetlbfs",
    "fusectl",
    "configfs",
    "binfmt_misc",
    "rpc_pipefs",
    "nsfs",
    "fuse.gvfsd-fuse",
    "fuse.portal",
    "fuse.lxcfs",
];

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn find_entry(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.exists() {
        return Some(direct);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

fn find_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    find_entry(dir, name).filter(|p| p.is_dir())
}

fn kobo_model_id(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join(".kobo").join("version")).ok()?;
    let last = text.trim().rsplit(',').next()?.trim().to_string();
    Some(last)
}

fn kobo_model(id: &str) -> Option<(&'static str, bool)> {
    let tail = id.get(id.len().saturating_sub(3)..)?;
    match tail {
        "390" => Some(("Kobo Libra Colour", false)),
        "391" => Some(("Kobo Clara BW", false)),
        "393" => Some(("Kobo Clara Colour", false)),
        "690" => Some(("Tolino Vision Color", true)),
        "691" => Some(("Tolino Shine 5", true)),
        "693" => Some(("Tolino Shine Color", true)),
        _ => None,
    }
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Marker {
    name: Option<String>,
    books_dir: Option<String>,
}

fn read_marker(root: &Path) -> Marker {
    fs::read_to_string(root.join(".shelfmark-device"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn sub_dir(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim().trim_matches(['/', '\\']);
    if rel.is_empty() {
        return None;
    }
    let p = Path::new(rel);
    if p.components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
    {
        Some(root.join(p))
    } else {
        None
    }
}

fn label_has(label: Option<&str>, needle: &str) -> bool {
    label.is_some_and(|l| l.to_ascii_lowercase().contains(needle))
}

fn build(
    root: &Path,
    kind: DeviceKind,
    name: String,
    books_dir: PathBuf,
    formats: &[&str],
) -> Device {
    let (total_bytes, free_bytes) = space(root);
    let root_s = root.to_string_lossy().into_owned();
    Device {
        id: root_s.clone(),
        name,
        kind,
        root: root_s,
        books_dir: books_dir.to_string_lossy().into_owned(),
        total_bytes,
        free_bytes,
        supported_formats: strings(formats),
        info: Vec::new(),
    }
}

pub fn space(root: &Path) -> (u64, u64) {
    (
        fs4::total_space(root).unwrap_or(0),
        fs4::available_space(root).unwrap_or(0),
    )
}

pub fn probe(root: &Path, label: Option<&str>) -> Option<Device> {
    probe_fs(root, label, None)
}

fn probe_fs(root: &Path, label: Option<&str>, fs_type: Option<&str>) -> Option<Device> {
    let mut device = probe_kind(root, label)?;
    if let Some(fs) = fs_type.and_then(info::file_system) {
        info::insert_file_system(&mut device.info, fs);
    }
    Some(device)
}

fn probe_kind(root: &Path, label: Option<&str>) -> Option<Device> {
    if !root.is_dir() {
        return None;
    }
    let label = label.filter(|l| !l.is_empty());
    let label_name = |default: &str| label.map(str::to_string).unwrap_or_else(|| default.into());

    let kobo = root.join(".kobo");
    if kobo.is_dir() && (kobo.join("KoboReader.sqlite").is_file() || kobo.join("version").is_file())
    {
        let model = kobo_model_id(root).as_deref().and_then(kobo_model);
        let mut device = match model {
            Some((name, true)) => build(
                root,
                DeviceKind::Tolino,
                name.into(),
                root.into(),
                TOLINO_FORMATS,
            ),
            Some((name, false)) => build(
                root,
                DeviceKind::Kobo,
                name.into(),
                root.into(),
                KOBO_FORMATS,
            ),
            None => build(
                root,
                DeviceKind::Kobo,
                "Kobo eReader".into(),
                root.into(),
                KOBO_FORMATS,
            ),
        };
        device.info = info::kobo(root, model.map(|(name, _)| name));
        return Some(device);
    }

    if let (Some(docs), Some(_)) = (find_dir(root, "documents"), find_dir(root, "system"))
        && find_dir(&root.join("system"), "config").is_none()
        && !label_has(label, "pocketbook")
    {
        let mut device = build(
            root,
            DeviceKind::Kindle,
            "Kindle".into(),
            docs.clone(),
            KINDLE_FORMATS,
        );
        device.info = info::kindle(root, &docs);
        return Some(device);
    }

    let system = root.join("system");
    if label_has(label, "pocketbook")
        || (system.is_dir()
            && ["config", "profiles", "explorer-3"]
                .iter()
                .any(|d| system.join(d).is_dir()))
    {
        let books = find_dir(root, "Books").unwrap_or_else(|| root.join("Books"));
        return Some(build(
            root,
            DeviceKind::PocketBook,
            label_name("PocketBook"),
            books,
            POCKETBOOK_FORMATS,
        ));
    }

    if label_has(label, "tolino") {
        return Some(build(
            root,
            DeviceKind::Tolino,
            label_name("Tolino"),
            root.into(),
            TOLINO_FORMATS,
        ));
    }

    if GENERIC_MARKERS.iter().any(|m| root.join(m).exists()) {
        let marker = read_marker(root);
        let books = marker
            .books_dir
            .as_deref()
            .and_then(|b| sub_dir(root, b))
            .unwrap_or_else(|| root.into());
        let name = marker.name.unwrap_or_else(|| label_name("E-reader"));
        return Some(build(
            root,
            DeviceKind::Generic,
            name,
            books,
            GENERIC_FORMATS,
        ));
    }

    None
}

pub fn folder_device(path: &Path) -> Result<Device> {
    let root = fs::canonicalize(path).map_err(|_| Error::NotAFolder(path.to_path_buf()))?;
    if !root.is_dir() {
        return Err(Error::NotAFolder(path.to_path_buf()));
    }
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    Ok(build(
        &root,
        DeviceKind::Folder,
        name,
        root.clone(),
        GENERIC_FORMATS,
    ))
}

pub fn detect_roots(candidates: &[(PathBuf, Option<String>)]) -> Vec<Device> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (root, label) in candidates {
        if !seen.insert(root.clone()) {
            continue;
        }
        if let Some(d) = probe(root, label.as_deref()) {
            out.push(d);
        }
    }
    out
}

fn detect_candidates(candidates: &[Candidate]) -> Vec<Device> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for c in candidates {
        if !seen.insert(c.root.clone()) {
            continue;
        }
        if let Some(d) = probe_fs(&c.root, c.label.as_deref(), c.fs_type.as_deref()) {
            out.push(d);
        }
    }
    out
}

struct Candidate {
    root: PathBuf,
    label: Option<String>,
    fs_type: Option<String>,
}

fn unescape_mount(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 4], 8)
        {
            out.push(v);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn parse_proc_mounts(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            let _dev = f.next()?;
            let mp = unescape_mount(f.next()?);
            let fstype = f.next()?;
            (!SKIP_FSTYPES.contains(&fstype)).then(|| PathBuf::from(mp))
        })
        .collect()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

fn plain(root: PathBuf) -> Candidate {
    Candidate {
        root,
        label: None,
        fs_type: None,
    }
}

fn candidate_roots() -> Vec<Candidate> {
    let mut list: Vec<Candidate> = Vec::new();

    if let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() {
        for d in disks.iter() {
            let name = d.name().to_string_lossy().into_owned();
            let label = (!name.is_empty() && !name.starts_with("/dev")).then_some(name);
            let fs_type = Some(d.file_system().to_string_lossy().into_owned());
            list.push(Candidate {
                root: d.mount_point().to_path_buf(),
                label,
                fs_type,
            });
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = fs::read_to_string("/proc/mounts") {
            list.extend(parse_proc_mounts(&text).into_iter().map(plain));
        }
        let mut dirs = children(Path::new("/media"));
        for user in children(Path::new("/media"))
            .iter()
            .chain(children(Path::new("/run/media")).iter())
        {
            dirs.extend(children(user));
        }
        dirs.extend(children(Path::new("/mnt")));
        list.extend(dirs.into_iter().map(plain));
    }

    #[cfg(target_os = "macos")]
    list.extend(children(Path::new("/Volumes")).into_iter().map(plain));

    let system_drive = std::env::var("SystemDrive").ok();
    list.retain(|Candidate { root: p, .. }| {
        if p.parent().is_none() && cfg!(unix) {
            return false;
        }
        if let Some(sd) = &system_drive {
            let s = p.to_string_lossy();
            if s.len() >= 2
                && s[..2].eq_ignore_ascii_case(sd)
                && s.trim_end_matches(['\\', '/']).len() == 2
            {
                return false;
            }
        }
        true
    });

    for c in &mut list {
        if c.label.is_none() {
            c.label = c.root.file_name().map(|n| n.to_string_lossy().into_owned());
        }
    }
    list
}

pub fn detect() -> Vec<Device> {
    detect_candidates(&candidate_roots())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_mounts() {
        let text = "/dev/sdb1 /media/me/KOBO\\040eReader vfat rw 0 0\nproc /proc proc rw 0 0\nC:\\134 /mnt/c 9p rw 0 0\n";
        let m = parse_proc_mounts(text);
        assert_eq!(
            m,
            [
                PathBuf::from("/media/me/KOBO eReader"),
                PathBuf::from("/mnt/c")
            ]
        );
    }
}
