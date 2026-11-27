use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use shelfmark_audio::transfer::{self, Dest, Mode, TransferPlan, Verify};
use shelfmark_audio::duplicates;
use shelfmark_audio::{AudioStore, Issue, LoftyReader, NewRoot, RootPatch, ScanSummary, scan};
use shelfmark_model::*;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::jobs::{JobEvent, Reporter};
use crate::state::{AppState, OpenLibrary};

type R<T> = Result<T, String>;

fn e(err: impl std::fmt::Display) -> String {
    err.to_string()
}

const LIBRARY_ROOT: &str = "Audiobooks";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    roots: Vec<AudioRoot>,
    audiobooks: u32,
    issues: Vec<IssueInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueInfo {
    #[serde(flatten)]
    issue: Issue,
    set: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootUpdate {
    path: Option<String>,
    managed: Option<bool>,
    layout: Option<AudioLayout>,
    label: Option<String>,
    enabled: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    book: BookSummary,
    manual: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    book: BookSummary,
    score: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookInfo {
    #[serde(flatten)]
    detail: AudiobookDetail,
    location: Option<String>,
    link: Option<LinkInfo>,
    candidates: Vec<Candidate>,
}

fn status(store: &AudioStore) -> R<AudioStatus> {
    let roots = store.roots().map_err(e)?;
    Ok(AudioStatus {
        audiobooks: roots.iter().filter(|r| r.enabled).map(|r| r.audiobooks).sum(),
        roots,
        issues: store
            .issues()
            .map_err(e)?
            .into_iter()
            .map(|issue| IssueInfo {
                set: (issue.kind == "duplicate")
                    .then(|| duplicates::set_key(issue.root_id, &issue.path))
                    .flatten(),
                issue,
            })
            .collect(),
    })
}

#[tauri::command(async)]
pub fn audio_status(state: State<AppState>) -> R<AudioStatus> {
    status(&state.current()?.audio)
}

#[tauri::command(async)]
pub fn audio_add_root(
    state: State<AppState>,
    path: Option<String>,
    in_library: bool,
) -> R<AudioRoot> {
    let open = state.current()?;
    let new = if in_library {
        let dir = open.library.dir().join(LIBRARY_ROOT);
        std::fs::create_dir_all(&dir).map_err(e)?;
        NewRoot {
            path: LIBRARY_ROOT.into(),
            in_library: true,
            managed: true,
            layout: AudioLayout::AuthorTitle,
            label: None,
        }
    } else {
        let path = path.ok_or("No folder was chosen")?;
        if !PathBuf::from(&path).is_dir() {
            return Err(format!("Not a folder: {path}"));
        }
        NewRoot {
            path,
            ..Default::default()
        }
    };
    open.audio.add_root(&new).map_err(e)
}

#[tauri::command(async)]
pub fn audio_update_root(state: State<AppState>, id: i64, patch: RootUpdate) -> R<AudioRoot> {
    let patch = RootPatch {
        path: patch.path,
        managed: patch.managed,
        layout: patch.layout,
        label: patch.label.map(|l| Some(l).filter(|l| !l.trim().is_empty())),
        enabled: patch.enabled,
    };
    state.current()?.audio.update_root(id, &patch).map_err(e)
}

#[tauri::command(async)]
pub fn audio_remove_root(state: State<AppState>, id: i64) -> R<()> {
    state.current()?.audio.remove_root(id).map_err(e)
}

#[tauri::command(async)]
pub fn audio_forget_missing(state: State<AppState>, id: i64) -> R<u32> {
    state.current()?.audio.forget_missing(id).map_err(e)
}

#[tauri::command(async)]
pub fn audio_issue_copies(
    state: State<AppState>,
    root_id: i64,
    path: String,
) -> R<Vec<duplicates::Copy>> {
    duplicates::copies(&state.current()?.audio, root_id, &path, &LoftyReader).map_err(e)
}

#[tauri::command(async)]
pub fn audio_issue_pick(state: State<AppState>, root_id: i64, path: String) -> R<()> {
    let open = state.current()?;
    let others = duplicates::siblings(&open.audio, root_id, &path).map_err(e)?;
    if !others.contains(&path) {
        return Err(format!("File is not reachable: {path}"));
    }
    open.audio.set_pick(root_id, &path, &others).map_err(e)
}

#[tauri::command(async)]
pub fn audio_issue_ignore(state: State<AppState>, root_id: i64, paths: Vec<String>) -> R<()> {
    state.current()?.audio.ignore_issues(root_id, &paths).map_err(e)
}

#[tauri::command(async)]
pub fn audio_issue_delete(state: State<AppState>, root_id: i64, path: String) -> R<()> {
    let open = state.current()?;
    let copies = duplicates::siblings(&open.audio, root_id, &path).map_err(e)?;
    if !copies.contains(&path) {
        return Err(format!("File is not reachable: {path}"));
    }
    if copies.len() < 2 {
        return Err("This is the only copy of the audiobook, it was not deleted".into());
    }
    std::fs::remove_file(duplicates::resolve(&open.audio, root_id, &path).map_err(e)?).map_err(e)
}

#[tauri::command(async)]
pub fn audio_issue_reveal(
    app: AppHandle,
    state: State<AppState>,
    root_id: i64,
    path: String,
) -> R<()> {
    let full = duplicates::resolve(&state.current()?.audio, root_id, &path).map_err(e)?;
    if !full.exists() {
        return Err(format!("File is not reachable: {}", full.display()));
    }
    app.opener().reveal_item_in_dir(full).map_err(e)
}

#[tauri::command(async)]
pub fn audio_scan(
    app: AppHandle,
    state: State<AppState>,
    root_ids: Option<Vec<i64>>,
    on_event: Channel<JobEvent>,
) -> R<u64> {
    let open: Arc<OpenLibrary> = state.current()?;
    let roots: Vec<i64> = match root_ids {
        Some(ids) => ids,
        None => open
            .audio
            .roots()
            .map_err(e)?
            .into_iter()
            .filter(|r| r.enabled)
            .map(|r| r.id)
            .collect(),
    };
    let (job, cancel) = state.jobs.start();
    std::thread::spawn(move || {
        let reporter = Reporter::new(on_event);
        let mut total = ScanSummary {
            complete: true,
            ..Default::default()
        };
        let mut outcome = None;
        for root in roots {
            let result = scan(&open.audio, root, &LoftyReader, &cancel, &|p| {
                reporter.progress(p.phase, p.done as u64, p.total as u64, p.current);
            });
            match result {
                Ok(s) => {
                    total.audiobooks += s.audiobooks;
                    total.added += s.added;
                    total.updated += s.updated;
                    total.unchanged += s.unchanged;
                    total.missing += s.missing;
                    total.issues += s.issues;
                    total.complete &= s.complete;
                }
                Err(shelfmark_audio::Error::Cancelled) => {
                    outcome = Some(JobEvent::Cancelled);
                    break;
                }
                Err(err) => {
                    outcome = Some(JobEvent::Failed {
                        message: err.to_string(),
                    });
                    break;
                }
            }
        }
        reporter.end(outcome.unwrap_or_else(|| JobEvent::Done {
            result: serde_json::to_value(&total).unwrap_or_default(),
        }));
        app.state::<AppState>().jobs.finish(job);
    });
    Ok(job)
}

#[tauri::command(async)]
pub fn job_cancel(state: State<AppState>, id: u64) {
    state.jobs.cancel(id);
}

#[tauri::command(async)]
pub fn audiobook(state: State<AppState>, id: AudiobookId) -> R<AudiobookInfo> {
    let open = state.current()?;
    let detail = open.audio.detail(id).map_err(e)?;
    let location = match detail.row.root_id {
        Some(root) => open.audio.root(root).ok().map(|r| {
            let base = open.audio.root_dir(&r);
            match detail.dir.as_deref() {
                Some(d) if !d.is_empty() => base.join(d),
                _ => base,
            }
            .to_string_lossy()
            .into_owned()
        }),
        None => None,
    };
    let pairs = open.library.audio_pairs(&open.audio).map_err(e)?;
    let link = match pairs.get(&id) {
        Some(&(book, manual)) => open.library.get_book(book).ok().map(|b| LinkInfo {
            book: b.summary,
            manual,
        }),
        None => None,
    };
    let candidates = if link.is_some() {
        Vec::new()
    } else {
        open.library
            .audio_candidates(id, &open.audio)
            .map_err(e)?
            .into_iter()
            .map(|(book, score)| Candidate { book, score })
            .collect()
    };
    Ok(AudiobookInfo {
        detail,
        location,
        link,
        candidates,
    })
}

#[tauri::command(async)]
pub fn audio_link(state: State<AppState>, audiobook_id: AudiobookId, book_id: BookId) -> R<()> {
    let open = state.current()?;
    let uuid = open.library.book_uuid(book_id).map_err(e)?;
    open.audio.clear_links_for_audiobook(audiobook_id).map_err(e)?;
    open.audio
        .set_link(audiobook_id, book_id, Some(&uuid), LinkState::Manual)
        .map_err(e)
}

#[tauri::command(async)]
pub fn audio_unlink(state: State<AppState>, audiobook_id: AudiobookId, book_id: BookId) -> R<()> {
    let open = state.current()?;
    let uuid = open.library.book_uuid(book_id).ok();
    open.audio
        .set_link(audiobook_id, book_id, uuid.as_deref(), LinkState::Rejected)
        .map_err(e)
}

#[tauri::command(async)]
pub fn audio_reset_link(state: State<AppState>, audiobook_id: AudiobookId) -> R<()> {
    state
        .current()?
        .audio
        .clear_links_for_audiobook(audiobook_id)
        .map_err(e)
}

fn audio_paths(open: &OpenLibrary, id: AudiobookId) -> R<Vec<PathBuf>> {
    let detail = open.audio.detail(id).map_err(e)?;
    let root = detail
        .row
        .root_id
        .ok_or("This audiobook has not been downloaded")?;
    let base = open.audio.root_dir(&open.audio.root(root).map_err(e)?);
    Ok(detail
        .files
        .iter()
        .filter(|f| f.kind == "audio")
        .map(|f| base.join(&f.rel_path))
        .collect())
}

#[tauri::command(async)]
pub fn audio_show_in_folder(app: AppHandle, state: State<AppState>, id: AudiobookId) -> R<()> {
    let open = state.current()?;
    let paths = audio_paths(&open, id)?;
    let first = paths.first().ok_or("This audiobook has no files")?;
    if !first.exists() {
        return Err(format!("File is not reachable: {}", first.display()));
    }
    app.opener().reveal_item_in_dir(first).map_err(e)
}

#[tauri::command(async)]
pub fn audio_open(app: AppHandle, state: State<AppState>, id: AudiobookId) -> R<()> {
    let open = state.current()?;
    let paths = audio_paths(&open, id)?;
    let first = paths.first().ok_or("This audiobook has no files")?;
    if !first.exists() {
        return Err(format!("File is not reachable: {}", first.display()));
    }
    let target = if paths.len() == 1 {
        first.clone()
    } else {
        let dir = state.cache_dir.join("playlists");
        std::fs::create_dir_all(&dir).map_err(e)?;
        let list = dir.join(format!("audiobook-{id}.m3u8"));
        let body: String = paths
            .iter()
            .map(|p| format!("{}\n", p.display()))
            .collect();
        std::fs::write(&list, format!("#EXTM3U\n{body}")).map_err(e)?;
        list
    };
    app.opener()
        .open_path(target.to_string_lossy(), None::<&str>)
        .map_err(e)
}

fn destination(root_id: Option<i64>, folder: Option<String>) -> R<Dest> {
    match (root_id, folder) {
        (Some(id), _) => Ok(Dest::Root(id)),
        (None, Some(f)) if !f.trim().is_empty() => Ok(Dest::Folder(PathBuf::from(f))),
        _ => Err("No destination was chosen".into()),
    }
}

#[tauri::command(async)]
pub fn audio_transfer_plan(
    state: State<AppState>,
    ids: Vec<AudiobookId>,
    dest_root_id: Option<i64>,
    dest_folder: Option<String>,
) -> R<TransferPlan> {
    let open = state.current()?;
    transfer::plan(&open.audio, &ids, &destination(dest_root_id, dest_folder)?).map_err(e)
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub fn audio_transfer(
    app: AppHandle,
    state: State<AppState>,
    ids: Vec<AudiobookId>,
    dest_root_id: Option<i64>,
    dest_folder: Option<String>,
    mode: Mode,
    verify: Verify,
    on_event: Channel<JobEvent>,
) -> R<u64> {
    let open: Arc<OpenLibrary> = state.current()?;
    let plan = transfer::plan(&open.audio, &ids, &destination(dest_root_id, dest_folder)?)
        .map_err(e)?;
    let (job, cancel) = state.jobs.start();
    std::thread::spawn(move || {
        let reporter = Reporter::new(on_event);
        let result = transfer::run(&open.audio, &plan, mode, verify, &cancel, &|p| {
            reporter.progress("copying", p.done_bytes, p.total_bytes, p.current);
        });
        let event = match result {
            Ok(summary) => {
                if mode == Mode::Copy
                    && summary.done > 0
                    && let Some(root) = dest_root_id
                {
                    let _ = scan(&open.audio, root, &LoftyReader, &cancel, &|p| {
                        reporter.progress(p.phase, p.done as u64, p.total as u64, p.current);
                    });
                }
                JobEvent::Done {
                    result: serde_json::to_value(&summary).unwrap_or_default(),
                }
            }
            Err(err) => JobEvent::Failed {
                message: err.to_string(),
            },
        };
        reporter.end(event);
        app.state::<AppState>().jobs.finish(job);
    });
    Ok(job)
}
