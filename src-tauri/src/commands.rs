use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use shelfmark_formats as formats;
use shelfmark_model::*;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, LibraryInfo, OpenLibrary, info};

type R<T> = Result<T, String>;

fn e(err: impl std::fmt::Display) -> String {
    err.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddOutcome {
    path: String,
    book_id: Option<BookId>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendOutcome {
    book_id: BookId,
    book: Option<DeviceBook>,
    converted: bool,
    error: Option<String>,
}

#[tauri::command(async)]
pub fn current_library(state: State<AppState>) -> Option<LibraryInfo> {
    state.current().ok().map(|o| info(&o.library))
}

#[tauri::command(async)]
pub fn recent_libraries(state: State<AppState>) -> Vec<LibraryInfo> {
    state.recent()
}

#[tauri::command(async)]
pub fn open_library(state: State<AppState>, path: String) -> R<LibraryInfo> {
    state.open(Path::new(&path), false)
}

#[tauri::command(async)]
pub fn create_library(state: State<AppState>, path: String) -> R<LibraryInfo> {
    state.open(Path::new(&path), true)
}

#[tauri::command(async)]
pub fn list_books(state: State<AppState>, query: ListQuery) -> R<Vec<BookSummary>> {
    state.current()?.library.list_books(&query).map_err(e)
}

#[tauri::command(async)]
pub fn get_book(state: State<AppState>, id: BookId) -> R<Book> {
    state.current()?.library.get_book(id).map_err(e)
}

#[tauri::command(async)]
pub fn get_categories(state: State<AppState>, search: Option<String>) -> R<Vec<Category>> {
    let search = search.filter(|s| !s.trim().is_empty());
    state
        .current()?
        .library
        .categories(search.as_deref())
        .map_err(e)
}

#[tauri::command(async)]
pub fn completions(state: State<AppState>, field: String) -> R<Vec<String>> {
    state.current()?.library.completions(&field).map_err(e)
}

#[tauri::command(async)]
pub fn saved_searches(state: State<AppState>) -> R<Vec<SavedSearch>> {
    state.current()?.library.saved_searches().map_err(e)
}

#[tauri::command(async)]
pub fn set_saved_searches(state: State<AppState>, searches: Vec<SavedSearch>) -> R<()> {
    state
        .current()?
        .library
        .set_saved_searches(&searches)
        .map_err(e)
}

#[tauri::command(async)]
pub fn update_book(state: State<AppState>, id: BookId, patch: BookPatch) -> R<Book> {
    state.current()?.library.update_book(id, &patch).map_err(e)
}

#[tauri::command(async)]
pub fn bulk_update(state: State<AppState>, ids: Vec<BookId>, patch: BulkPatch) -> R<()> {
    state
        .current()?
        .library
        .bulk_update(&ids, &patch)
        .map_err(e)
}

#[tauri::command(async)]
pub fn set_cover_from_file(state: State<AppState>, id: BookId, path: String) -> R<()> {
    let bytes = std::fs::read(&path).map_err(e)?;
    state.current()?.library.set_cover(id, &bytes).map_err(e)
}

#[tauri::command]
pub async fn set_cover_from_url(state: State<'_, AppState>, id: BookId, url: String) -> R<()> {
    let open = state.current()?;
    let bytes = formats::sources::fetch_cover(&url).await.map_err(e)?;
    tauri::async_runtime::spawn_blocking(move || open.library.set_cover(id, &bytes).map_err(e))
        .await
        .map_err(e)?
}

#[tauri::command(async)]
pub fn remove_cover(state: State<AppState>, id: BookId) -> R<()> {
    state.current()?.library.remove_cover(id).map_err(e)
}

#[tauri::command(async)]
pub fn embed_metadata(state: State<AppState>, ids: Vec<BookId>) -> R<u32> {
    let open = state.current()?;
    let lib = &open.library;
    let mut written = 0;
    for id in ids {
        let book = lib.get_book(id).map_err(e)?;
        let meta = lib.metadata(id).map_err(e)?;
        let cover = lib
            .cover_path(id)
            .ok()
            .flatten()
            .and_then(|p| std::fs::read(p).ok());
        for file in &book.files {
            let Ok(path) = lib.format_path(id, &file.format) else {
                continue;
            };
            if formats::write_metadata(&path, &meta, cover.as_deref()).map_err(e)? {
                written += 1;
            }
        }
    }
    Ok(written)
}

fn collect_books(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut entries: Vec<_> = entries.flatten().map(|d| d.path()).collect();
            entries.sort();
            for entry in entries {
                collect_books(&entry, out);
            }
        }
    } else if path
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| formats::BOOK_EXTENSIONS.contains(&x.to_lowercase().as_str()))
    {
        out.push(path.to_path_buf());
    }
}

fn import_file(open: &OpenLibrary, file: &Path) -> AddOutcome {
    let result = formats::read_metadata(file).map_err(e).and_then(|meta| {
        let cover = formats::read_cover(file).ok().flatten();
        open.library
            .add_book(&meta, cover.as_deref(), &[file.to_path_buf()])
            .map_err(e)
    });
    AddOutcome {
        path: file.to_string_lossy().into_owned(),
        book_id: result.as_ref().ok().copied(),
        error: result.err(),
    }
}

#[tauri::command(async)]
pub fn add_books(state: State<AppState>, paths: Vec<String>) -> R<Vec<AddOutcome>> {
    let open = state.current()?;
    let mut files = Vec::new();
    for path in &paths {
        let path = Path::new(path);
        if path.is_dir() {
            collect_books(path, &mut files);
        } else {
            files.push(path.to_path_buf());
        }
    }
    Ok(files.iter().map(|file| import_file(&open, file)).collect())
}

#[tauri::command(async)]
pub fn import_from_device(
    state: State<AppState>,
    device_id: String,
    paths: Vec<String>,
) -> R<Vec<AddOutcome>> {
    let device = device(&state, &device_id)?;
    let open = state.current()?;
    let root = Path::new(&device.root);
    Ok(paths
        .iter()
        .map(|path| {
            let file = Path::new(path);
            if file.starts_with(root) && file.is_file() {
                import_file(&open, file)
            } else {
                AddOutcome {
                    path: path.clone(),
                    book_id: None,
                    error: Some("File is not on this device".to_string()),
                }
            }
        })
        .collect())
}

#[tauri::command(async)]
pub fn add_format(state: State<AppState>, id: BookId, path: String) -> R<Book> {
    let open = state.current()?;
    open.library.add_format(id, Path::new(&path)).map_err(e)?;
    open.library.get_book(id).map_err(e)
}

#[tauri::command(async)]
pub fn remove_format(state: State<AppState>, id: BookId, format: String) -> R<Book> {
    let open = state.current()?;
    open.library.remove_format(id, &format).map_err(e)?;
    open.library.get_book(id).map_err(e)
}

#[tauri::command(async)]
pub fn delete_books(state: State<AppState>, ids: Vec<BookId>) -> R<()> {
    let open = state.current()?;
    open.library.delete_books(&ids).map_err(e)?;
    for id in ids {
        let _ = open.reading.delete_book(id);
    }
    Ok(())
}

#[tauri::command(async)]
pub fn show_in_folder(app: AppHandle, state: State<AppState>, id: BookId) -> R<()> {
    let open = state.current()?;
    let dir = open.library.book_dir(id).map_err(e)?;
    let target = std::fs::read_dir(&dir)
        .ok()
        .and_then(|mut d| d.next())
        .and_then(|f| f.ok())
        .map(|f| f.path())
        .unwrap_or(dir);
    app.opener().reveal_item_in_dir(target).map_err(e)
}

#[tauri::command(async)]
pub fn open_externally(
    app: AppHandle,
    state: State<AppState>,
    id: BookId,
    format: String,
) -> R<()> {
    let path = state
        .current()?
        .library
        .format_path(id, &format)
        .map_err(e)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(e)
}

#[tauri::command]
pub async fn identify(query: IdentifyQuery) -> R<Vec<SourceResult>> {
    Ok(formats::sources::identify(&query).await)
}

fn devices(state: &AppState) -> Vec<Device> {
    let mut devices = shelfmark_devices::detect();
    for path in state.folder_devices() {
        if let Ok(device) = shelfmark_devices::folder_device(Path::new(&path))
            && !devices.iter().any(|d| d.id == device.id)
        {
            devices.push(device);
        }
    }
    devices
}

fn device(state: &AppState, id: &str) -> R<Device> {
    devices(state)
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| "Device is no longer connected".to_string())
}

#[tauri::command(async)]
pub fn detect_devices(state: State<AppState>) -> Vec<Device> {
    devices(&state)
}

#[tauri::command(async)]
pub fn add_folder_device(state: State<AppState>, path: String) -> R<Device> {
    let device = shelfmark_devices::folder_device(Path::new(&path)).map_err(e)?;
    state.add_folder_device(&path);
    Ok(device)
}

#[tauri::command(async)]
pub fn device_books(state: State<AppState>, device_id: String) -> R<Vec<DeviceBook>> {
    let device = device(&state, &device_id)?;
    shelfmark_devices::list_books(&device, &|p| formats::read_metadata(p).ok()).map_err(e)
}

fn convert_for_device(
    state: &AppState,
    open: &OpenLibrary,
    device: &Device,
    book: &Book,
) -> R<String> {
    let lib = &open.library;
    let id = book.summary.id;
    let has = |f: &str| {
        book.summary
            .formats
            .iter()
            .any(|b| b.eq_ignore_ascii_case(f))
    };
    let source = ["EPUB", "KEPUB"].into_iter().find(|f| has(f));
    let target = device
        .supported_formats
        .iter()
        .find(|t| source.is_some_and(|s| shelfmark_convert::can_convert(s, t)));
    let (Some(source), Some(target)) = (source, target) else {
        return Err(format!(
            "No format this device can read (has {})",
            book.summary.formats.join(", ")
        ));
    };
    let src = lib.format_path(id, source).map_err(e)?;
    let meta = lib.metadata(id).map_err(e)?;
    let cover = lib
        .cover_path(id)
        .ok()
        .flatten()
        .and_then(|p| std::fs::read(p).ok());
    let dir = state.cache_dir.join("convert");
    std::fs::create_dir_all(&dir).map_err(e)?;
    let out = dir.join(format!("{id}.{}", target.to_lowercase()));
    shelfmark_convert::epub_to_azw3(&src, &out, &meta, cover.as_deref())
        .map_err(|err| format!("Could not convert {source} to {target}: {err}"))?;
    let added = lib.add_format(id, &out).map_err(e);
    let _ = std::fs::remove_file(&out);
    added?;
    Ok(target.clone())
}

fn kindle_thumbnail(device: &Device, cover: Option<PathBuf>, meta: &Metadata) {
    let (Some(cover), Some(uuid)) = (cover, meta.uuid.as_deref()) else {
        return;
    };
    let dir = Path::new(&device.root).join("system").join("thumbnails");
    if !dir.is_dir() || !uuid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return;
    }
    let Ok(img) = image::open(&cover) else {
        return;
    };
    let thumb = img.thumbnail(330, 470).to_rgb8();
    let _ = thumb.save_with_format(
        dir.join(format!("thumbnail_{uuid}_EBOK_portrait.jpg")),
        image::ImageFormat::Jpeg,
    );
}

#[tauri::command(async)]
pub fn send_to_device(
    state: State<AppState>,
    device_id: String,
    ids: Vec<BookId>,
) -> R<Vec<SendOutcome>> {
    let device = device(&state, &device_id)?;
    let open = state.current()?;
    let lib = &open.library;
    Ok(ids
        .into_iter()
        .map(|id| {
            let mut converted = false;
            let result = (|| {
                let book = lib.get_book(id).map_err(e)?;
                let direct = device
                    .supported_formats
                    .iter()
                    .find(|f| {
                        book.summary
                            .formats
                            .iter()
                            .any(|b| b.eq_ignore_ascii_case(f))
                    })
                    .cloned();
                let format = match direct {
                    Some(format) => format,
                    None => {
                        let format = convert_for_device(&state, &open, &device, &book)?;
                        converted = true;
                        format
                    }
                };
                let path = lib.format_path(id, &format).map_err(e)?;
                let meta = lib.metadata(id).map_err(e)?;
                let sent =
                    shelfmark_devices::send_book(&device, &path, &meta, &format).map_err(e)?;
                if converted && device.kind == DeviceKind::Kindle {
                    kindle_thumbnail(&device, lib.cover_path(id).ok().flatten(), &meta);
                }
                Ok(sent)
            })();
            SendOutcome {
                book_id: id,
                book: result.as_ref().ok().cloned(),
                converted,
                error: result.err(),
            }
        })
        .collect())
}

#[tauri::command(async)]
pub fn delete_device_book(state: State<AppState>, device_id: String, path: String) -> R<()> {
    let device = device(&state, &device_id)?;
    shelfmark_devices::delete_book(&device, Path::new(&path)).map_err(e)
}

#[tauri::command(async)]
pub fn reading_progress(state: State<AppState>) -> R<HashMap<BookId, f64>> {
    state.current()?.reading.progress().map_err(e)
}

#[tauri::command(async)]
pub fn reading_state(state: State<AppState>, id: BookId) -> R<Option<ReadingState>> {
    state.current()?.reading.state(id).map_err(e)
}

#[tauri::command(async)]
pub fn set_reading_state(app_state: State<AppState>, state: ReadingState) -> R<()> {
    app_state.current()?.reading.set_state(&state).map_err(e)
}

#[tauri::command(async)]
pub fn annotations(state: State<AppState>, id: BookId) -> R<Vec<Annotation>> {
    state.current()?.reading.annotations(id).map_err(e)
}

#[tauri::command(async)]
pub fn save_annotation(state: State<AppState>, annotation: Annotation) -> R<()> {
    state
        .current()?
        .reading
        .save_annotation(&annotation)
        .map_err(e)
}

#[tauri::command(async)]
pub fn delete_annotation(state: State<AppState>, id: String) -> R<()> {
    state.current()?.reading.delete_annotation(&id).map_err(e)
}
