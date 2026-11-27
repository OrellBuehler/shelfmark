use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use shelfmark_audio::AudioStore;
use shelfmark_library::Library;
use shelfmark_reading::ReadingStore;
use tauri::{AppHandle, Manager};

pub struct OpenLibrary {
    pub library: Library,
    pub reading: ReadingStore,
    pub audio: AudioStore,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub path: String,
    pub name: String,
    pub book_count: u64,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Config {
    recent: Vec<LibraryInfo>,
    folder_devices: Vec<String>,
}

pub struct AppState {
    open: RwLock<Option<Arc<OpenLibrary>>>,
    config: Mutex<Config>,
    config_path: PathBuf,
    pub cache_dir: PathBuf,
    pub jobs: crate::jobs::Jobs,
}

impl AppState {
    pub fn load(app: &AppHandle) -> tauri::Result<Self> {
        let config_dir = app.path().app_config_dir()?;
        let cache_dir = app.path().app_cache_dir()?.join("covers");
        let _ = std::fs::create_dir_all(&config_dir);
        let config_path = config_dir.join("config.json");
        let config: Config = std::fs::read(&config_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let state = Self {
            open: RwLock::new(None),
            config: Mutex::new(config),
            config_path,
            cache_dir,
            jobs: crate::jobs::Jobs::default(),
        };
        let last = state.config.lock().unwrap().recent.first().cloned();
        if let Some(last) = last {
            let _ = state.open(Path::new(&last.path), false);
        }
        Ok(state)
    }

    pub fn audible_file(&self) -> PathBuf {
        self.config_path.with_file_name("audible-accounts.json")
    }

    pub fn current(&self) -> Result<Arc<OpenLibrary>, String> {
        self.open
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| "No library is open".to_string())
    }

    pub fn open(&self, dir: &Path, create: bool) -> Result<LibraryInfo, String> {
        let library = if create {
            Library::create(dir)
        } else {
            Library::open(dir)
        }
        .map_err(|e| e.to_string())?;
        let reading = ReadingStore::open(dir).map_err(|e| e.to_string())?;
        let audio = AudioStore::open(dir).map_err(|e| e.to_string())?;
        let info = info(&library);
        self.jobs.cancel_all();
        *self.open.write().unwrap() = Some(Arc::new(OpenLibrary {
            library,
            reading,
            audio,
        }));
        let mut config = self.config.lock().unwrap();
        config.recent.retain(|r| r.path != info.path);
        config.recent.insert(0, info.clone());
        config.recent.truncate(10);
        self.save(&config);
        Ok(info)
    }

    pub fn recent(&self) -> Vec<LibraryInfo> {
        let config = self.config.lock().unwrap();
        config
            .recent
            .iter()
            .filter(|r| Path::new(&r.path).join("metadata.db").exists())
            .cloned()
            .collect()
    }

    pub fn folder_devices(&self) -> Vec<String> {
        self.config.lock().unwrap().folder_devices.clone()
    }

    pub fn add_folder_device(&self, path: &str) {
        let mut config = self.config.lock().unwrap();
        if !config.folder_devices.iter().any(|p| p == path) {
            config.folder_devices.push(path.to_string());
            self.save(&config);
        }
    }

    fn save(&self, config: &Config) {
        if let Ok(json) = serde_json::to_vec_pretty(config) {
            let _ = std::fs::write(&self.config_path, json);
        }
    }
}

pub fn info(library: &Library) -> LibraryInfo {
    let dir = library.dir();
    LibraryInfo {
        path: dir.to_string_lossy().into_owned(),
        name: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| dir.to_string_lossy().into_owned()),
        book_count: library.book_count().unwrap_or(0),
    }
}
