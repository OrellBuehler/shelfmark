use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use shelfmark_model::{AudioState, AudiobookId};

use crate::error::{Error, Result};
use crate::layout::book_dir;
use crate::names::{file_name, split_ext};
use crate::store::AudioStore;

const CHUNK: usize = 4 * 1024 * 1024;
const EDGE: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Copy,
    Move,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verify {
    Quick,
    Full,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Dest {
    Root(i64),
    Folder(PathBuf),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFile {
    pub src: String,
    pub dest: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub audiobook_id: AudiobookId,
    pub title: String,
    pub dest_dir: String,
    pub key: String,
    pub files: Vec<PlanFile>,
    pub bytes: u64,
    pub skip: Option<String>,
    #[serde(skip)]
    pub src_root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPlan {
    pub items: Vec<PlanItem>,
    pub total_bytes: u64,
    pub free_bytes: Option<u64>,
    pub enough_space: bool,
    pub dest_path: String,
    #[serde(skip)]
    pub dest_root: PathBuf,
    #[serde(skip)]
    pub dest_root_id: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransferProgress {
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub item: usize,
    pub items: usize,
    pub current: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSummary {
    pub done: u32,
    pub skipped: Vec<TransferNote>,
    pub failed: Vec<TransferNote>,
    pub bytes: u64,
    pub cancelled: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferNote {
    pub audiobook_id: AudiobookId,
    pub title: String,
    pub message: String,
}

fn existing_ancestor(path: &Path) -> Option<&Path> {
    path.ancestors().find(|p| p.exists())
}

pub fn plan(store: &AudioStore, ids: &[AudiobookId], dest: &Dest) -> Result<TransferPlan> {
    let (dest_root, dest_root_id, layout) = match dest {
        Dest::Root(id) => {
            let root = store.root(*id)?;
            if !root.managed {
                return Err(Error::Invalid(
                    "Shelfmark is not allowed to add files to this location".into(),
                ));
            }
            (store.root_dir(&root), Some(root.id), root.layout)
        }
        Dest::Folder(p) => (p.clone(), None, shelfmark_model::AudioLayout::AuthorTitle),
    };
    let mut taken: HashSet<String> = HashSet::new();
    let mut items = Vec::new();
    let mut total = 0u64;
    for &id in ids {
        let detail = store.detail(id)?;
        let row = &detail.row;
        let dir = book_dir(layout, &row.title, &row.authors, row.asin.as_deref());
        let audio: Vec<_> = detail.files.iter().filter(|f| f.kind == "audio").collect();
        let files: Vec<PlanFile> = detail
            .files
            .iter()
            .map(|f| PlanFile {
                src: f.rel_path.clone(),
                dest: format!("{dir}/{}", file_name(&f.rel_path)),
                size: f.size,
            })
            .collect();
        let key = match audio.as_slice() {
            [one] => format!("{dir}/{}", split_ext(file_name(&one.rel_path)).0),
            _ => dir.clone(),
        };
        let bytes: u64 = files.iter().map(|f| f.size).sum();
        let src_root = match row.root_id {
            Some(r) => store.root_dir(&store.root(r)?),
            None => PathBuf::new(),
        };
        let skip = if row.root_id.is_none() || row.state != AudioState::Present {
            Some("files are not available".to_string())
        } else if audio.is_empty() {
            Some("no audio files".to_string())
        } else if dest_root_id.is_some() && row.root_id == dest_root_id {
            Some("already in this location".to_string())
        } else if !taken.insert(dir.to_lowercase()) {
            Some("another selected audiobook uses the same folder name".to_string())
        } else if dest_root.join(&dir).exists() {
            Some(format!("folder already exists: {dir}"))
        } else {
            None
        };
        if skip.is_none() {
            total += bytes;
        }
        items.push(PlanItem {
            audiobook_id: id,
            title: row.title.clone(),
            dest_dir: dir,
            key,
            files,
            bytes,
            skip,
            src_root,
        });
    }
    let free = existing_ancestor(&dest_root).and_then(|p| fs4::available_space(p).ok());
    Ok(TransferPlan {
        items,
        total_bytes: total,
        enough_space: free.is_none_or(|f| f > total),
        free_bytes: free,
        dest_path: dest_root.to_string_lossy().into_owned(),
        dest_root,
        dest_root_id,
    })
}

fn copy_hashed(
    src: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<blake3::Hash> {
    let mut input = File::open(src)?;
    let mut output = File::create(dest)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        output.write_all(&buf[..n])?;
        on_bytes(n as u64);
    }
    output.sync_all()?;
    if let Ok(modified) = input.metadata().and_then(|m| m.modified()) {
        let _ = output.set_modified(modified);
    }
    Ok(hasher.finalize())
}

fn hash_file(path: &Path, cancel: &AtomicBool) -> Result<blake3::Hash> {
    let mut input = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
}

fn edges(path: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let mut head = vec![0u8; EDGE.min(len) as usize];
    f.read_exact(&mut head)?;
    let tail_len = EDGE.min(len);
    f.seek(SeekFrom::Start(len - tail_len))?;
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact(&mut tail)?;
    Ok((head, tail))
}

fn verify(src: &Path, dest: &Path, hash: blake3::Hash, mode: Verify, cancel: &AtomicBool) -> Result<()> {
    let mismatch = || Error::Invalid(format!("copy does not match the original: {}", dest.display()));
    if fs::metadata(src)?.len() != fs::metadata(dest)?.len() {
        return Err(mismatch());
    }
    let same = match mode {
        Verify::Full => hash_file(dest, cancel)? == hash,
        Verify::Quick => edges(src)? == edges(dest)?,
    };
    if same { Ok(()) } else { Err(mismatch()) }
}

fn mtime(path: &Path) -> i64 {
    fs::metadata(path)
        .map(|m| crate::walk::mtime_secs(&m))
        .unwrap_or(0)
}

fn prune_empty(mut dir: &Path, stop: &Path) {
    while dir != stop && dir.starts_with(stop) {
        if fs::remove_dir(dir).is_err() {
            break;
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => break,
        }
    }
}

struct Staged {
    staging: PathBuf,
    renamed: Vec<(PathBuf, PathBuf)>,
    copied: Vec<PathBuf>,
}

impl Staged {
    fn rollback(self) {
        for (from, to) in self.renamed.into_iter().rev() {
            let _ = fs::rename(to, from);
        }
        let _ = fs::remove_dir_all(&self.staging);
    }
}

fn stage(
    item: &PlanItem,
    plan: &TransferPlan,
    mode: Mode,
    check: Verify,
    cancel: &AtomicBool,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<Staged> {
    let final_dir = plan.dest_root.join(&item.dest_dir);
    let parent = final_dir.parent().unwrap_or(&plan.dest_root).to_path_buf();
    fs::create_dir_all(&parent)?;
    let staging = parent.join(format!(".shelfmark-part-{}", item.audiobook_id));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)?;
    let mut staged = Staged {
        staging: staging.clone(),
        renamed: Vec::new(),
        copied: Vec::new(),
    };
    let result = (|| -> Result<()> {
        for f in &item.files {
            let src = item.src_root.join(&f.src);
            let dest = staging.join(file_name(&f.dest));
            if mode == Mode::Move && fs::rename(&src, &dest).is_ok() {
                staged.renamed.push((src, dest));
                on_bytes(f.size);
                continue;
            }
            let hash = copy_hashed(&src, &dest, cancel, on_bytes)?;
            verify(&src, &dest, hash, check, cancel)?;
            staged.copied.push(src);
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(staged),
        Err(e) => {
            staged.rollback();
            Err(e)
        }
    }
}

pub fn run(
    store: &AudioStore,
    plan: &TransferPlan,
    mode: Mode,
    check: Verify,
    cancel: &AtomicBool,
    progress: &dyn Fn(TransferProgress),
) -> Result<TransferSummary> {
    if !plan.enough_space {
        return Err(Error::Invalid("not enough free space at the destination".into()));
    }
    if mode == Mode::Move && plan.dest_root_id.is_none() {
        return Err(Error::Invalid("moving is only possible between audiobook locations".into()));
    }
    let mut summary = TransferSummary::default();
    let items = plan.items.len();
    let mut done_bytes = 0u64;
    for (i, item) in plan.items.iter().enumerate() {
        let note = |message: String| TransferNote {
            audiobook_id: item.audiobook_id,
            title: item.title.clone(),
            message,
        };
        if let Some(reason) = &item.skip {
            summary.skipped.push(note(reason.clone()));
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            break;
        }
        let before = done_bytes;
        let mut on_bytes = |n: u64| {
            done_bytes += n;
            progress(TransferProgress {
                done_bytes,
                total_bytes: plan.total_bytes,
                item: i + 1,
                items,
                current: Some(item.title.clone()),
            });
        };
        let staged = match stage(item, plan, mode, check, cancel, &mut on_bytes) {
            Ok(s) => s,
            Err(Error::Cancelled) => {
                summary.cancelled = true;
                break;
            }
            Err(e) => {
                done_bytes = before + item.bytes;
                summary.failed.push(note(e.to_string()));
                continue;
            }
        };
        let final_dir = plan.dest_root.join(&item.dest_dir);
        if let Err(e) = fs::rename(&staged.staging, &final_dir) {
            staged.rollback();
            done_bytes = before + item.bytes;
            summary.failed.push(note(e.to_string()));
            continue;
        }
        if mode == Mode::Move
            && let Some(root_id) = plan.dest_root_id
        {
            let files: Vec<(String, String, i64)> = item
                .files
                .iter()
                .map(|f| {
                    (
                        f.src.clone(),
                        f.dest.clone(),
                        mtime(&plan.dest_root.join(&f.dest)),
                    )
                })
                .collect();
            store.relocate(item.audiobook_id, root_id, &item.key, &files)?;
            for src in &staged.copied {
                let _ = fs::remove_file(src);
            }
            let sources = staged
                .copied
                .iter()
                .chain(staged.renamed.iter().map(|(from, _)| from));
            let dirs: HashSet<PathBuf> = sources
                .filter_map(|p| p.parent().map(Path::to_path_buf))
                .collect();
            for d in dirs {
                prune_empty(&d, &item.src_root);
            }
        }
        done_bytes = before + item.bytes;
        summary.done += 1;
        summary.bytes += item.bytes;
    }
    Ok(summary)
}
