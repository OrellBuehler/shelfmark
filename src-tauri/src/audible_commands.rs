use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde::Serialize;
use shelfmark_audible::{Account, default_libation_path, fetch_cover, fetch_library, load_libation};
use shelfmark_audio::{AudioStore, RemoteBook};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::jobs::{JobEvent, Reporter};
use crate::state::{AppState, OpenLibrary};

type R<T> = Result<T, String>;

fn e(err: impl std::fmt::Display) -> String {
    err.to_string()
}

const SOURCE: &str = "audible_source";
const LAST_SYNC: &str = "audible_last_sync";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudibleAccount {
    key: String,
    name: String,
    locale: String,
    problem: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudibleStatus {
    source: Option<String>,
    detected: Option<String>,
    accounts: Vec<AudibleAccount>,
    error: Option<String>,
    last_sync: Option<String>,
    titles: u32,
    not_downloaded: u32,
    imported: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct SyncSummary {
    accounts: u32,
    titles: u32,
    added: u32,
    removed: u32,
    failed: Vec<SyncFailure>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncFailure {
    name: String,
    message: String,
}

fn describe(a: &Account) -> AudibleAccount {
    AudibleAccount {
        key: a.key.clone(),
        name: a.name.clone(),
        locale: a.locale.clone(),
        problem: a.problem.clone(),
    }
}

fn import(from: &Path, to: &Path) -> R<()> {
    if from == to {
        return Ok(());
    }
    let accounts = load_libation(from).map_err(e)?;
    if accounts.is_empty() {
        return Err("Libation has no accounts".into());
    }
    let tmp = to.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(e)?;
    file.write_all(&std::fs::read(from).map_err(e)?).map_err(e)?;
    file.sync_all().map_err(e)?;
    drop(file);
    std::fs::rename(&tmp, to).map_err(e)
}

fn status(state: &AppState, store: &AudioStore) -> R<AudibleStatus> {
    let own = state.audible_file();
    let mut source = store.setting(SOURCE).map_err(e)?;
    if let Some(path) = &source
        && Path::new(path) != own
        && import(Path::new(path), &own).is_ok()
    {
        let moved = own.to_string_lossy().into_owned();
        store.set_setting(SOURCE, Some(&moved)).map_err(e)?;
        source = Some(moved);
    }
    let (accounts, error) = match &source {
        Some(path) => match load_libation(&PathBuf::from(path)) {
            Ok(list) => (list.iter().map(describe).collect(), None),
            Err(err) => (Vec::new(), Some(err.to_string())),
        },
        None => (Vec::new(), None),
    };
    Ok(AudibleStatus {
        imported: source.as_deref().map(Path::new) == Some(own.as_path()),
        source,
        detected: default_libation_path()
            .or_else(|| own.is_file().then_some(own))
            .map(|p| p.to_string_lossy().into_owned()),
        accounts,
        error,
        last_sync: store.setting(LAST_SYNC).map_err(e)?,
        titles: store.remote_count().map_err(e)?,
        not_downloaded: store.remote_missing_count().map_err(e)?,
    })
}

#[tauri::command(async)]
pub fn audible_status(state: State<AppState>) -> R<AudibleStatus> {
    status(&state, &state.current()?.audio)
}

#[tauri::command(async)]
pub fn audible_connect(state: State<AppState>, path: Option<String>) -> R<AudibleStatus> {
    let open = state.current()?;
    let own = state.audible_file();
    let path = path
        .map(PathBuf::from)
        .or_else(default_libation_path)
        .or_else(|| own.is_file().then(|| own.clone()))
        .ok_or("No Libation accounts file was found")?;
    import(&path, &own)?;
    open.audio
        .set_setting(SOURCE, Some(&own.to_string_lossy()))
        .map_err(e)?;
    status(&state, &open.audio)
}

#[tauri::command(async)]
pub fn audible_disconnect(state: State<AppState>) -> R<AudibleStatus> {
    let open = state.current()?;
    open.audio.set_setting(SOURCE, None).map_err(e)?;
    open.audio.set_setting(LAST_SYNC, None).map_err(e)?;
    open.audio.clear_remote().map_err(e)?;
    status(&state, &open.audio)
}

#[tauri::command(async)]
pub fn audible_sync(
    app: AppHandle,
    state: State<AppState>,
    on_event: Channel<JobEvent>,
) -> R<u64> {
    let open: Arc<OpenLibrary> = state.current()?;
    let source = open
        .audio
        .setting(SOURCE)
        .map_err(e)?
        .ok_or("Audible is not connected")?;
    let accounts = load_libation(&PathBuf::from(source)).map_err(e)?;
    let (job, cancel) = state.jobs.start();
    std::thread::spawn(move || {
        let reporter = Reporter::new(on_event);
        let mut summary = SyncSummary::default();
        let mut cancelled = false;
        for account in &accounts {
            if cancel.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let fetched = tauri::async_runtime::block_on(fetch_library(account, &cancel, &|n| {
                reporter.progress("library", n as u64, 0, Some(account.name.clone()));
            }));
            let books = match fetched {
                Ok(b) => b,
                Err(shelfmark_audible::Error::Cancelled) => {
                    cancelled = true;
                    break;
                }
                Err(err) => {
                    summary.failed.push(SyncFailure {
                        name: account.name.clone(),
                        message: err.to_string(),
                    });
                    continue;
                }
            };
            let covers: HashMap<&str, &str> = books
                .iter()
                .filter_map(|b| Some((b.asin.as_str(), b.cover_url.as_deref()?)))
                .collect();
            let rows: Vec<RemoteBook> = books
                .iter()
                .map(|b| RemoteBook {
                    asin: b.asin.clone(),
                    title: b.title.clone(),
                    subtitle: b.subtitle.clone(),
                    authors: b.authors.clone(),
                    narrators: b.narrators.clone(),
                    series: b.series.clone(),
                    series_index: b.series_index,
                    publisher: b.publisher.clone(),
                    language: b.language.clone(),
                    release_date: b.release_date.clone(),
                    duration_ms: b.duration_ms,
                    description: b.description.clone(),
                    added: b.purchase_date.clone(),
                })
                .collect();
            let synced = match open.audio.sync_remote(&account.key, &rows) {
                Ok(s) => s,
                Err(err) => {
                    summary.failed.push(SyncFailure {
                        name: account.name.clone(),
                        message: err.to_string(),
                    });
                    continue;
                }
            };
            summary.accounts += 1;
            summary.titles += synced.total;
            summary.added += synced.added;
            summary.removed += synced.removed;
            let total = synced.without_cover.len() as u64;
            for (i, (id, asin)) in synced.without_cover.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    break;
                }
                reporter.progress("covers", i as u64 + 1, total, Some(account.name.clone()));
                let Some(url) = covers.get(asin.as_str()) else {
                    continue;
                };
                if let Some(bytes) = tauri::async_runtime::block_on(fetch_cover(url))
                    && let Some(thumb) = shelfmark_audio::scan::thumbnail(&bytes)
                {
                    let _ = open.audio.set_cover(*id, &thumb, "audible", 0);
                }
            }
        }
        if !cancelled {
            let now = shelfmark_audio::scan::iso(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64),
            );
            let _ = open.audio.set_setting(LAST_SYNC, Some(&now));
        }
        reporter.end(if cancelled {
            JobEvent::Cancelled
        } else {
            JobEvent::Done {
                result: serde_json::to_value(&summary).unwrap_or_default(),
            }
        });
        app.state::<AppState>().jobs.finish(job);
    });
    Ok(job)
}
