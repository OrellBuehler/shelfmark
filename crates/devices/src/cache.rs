use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub size: u64,
    pub mtime: i64,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub uuid: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Cache {
    #[serde(default)]
    pub entries: BTreeMap<String, Entry>,
    #[serde(skip)]
    pub dirty: bool,
}

fn file(root: &Path) -> PathBuf {
    root.join(".shelfmark").join("cache.json")
}

impl Cache {
    pub fn load(root: &Path) -> Cache {
        fs::read_to_string(file(root))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn get(&self, rel: &str, size: u64, mtime: i64) -> Option<&Entry> {
        self.entries
            .get(rel)
            .filter(|e| e.size == size && e.mtime == mtime)
    }

    pub fn put(&mut self, rel: String, entry: Entry) {
        if self.entries.get(&rel) != Some(&entry) {
            self.entries.insert(rel, entry);
            self.dirty = true;
        }
    }

    pub fn remove(&mut self, rel: &str) {
        if self.entries.remove(rel).is_some() {
            self.dirty = true;
        }
    }

    pub fn retain_only(&mut self, keep: &std::collections::HashSet<String>) {
        let before = self.entries.len();
        self.entries.retain(|k, _| keep.contains(k));
        if self.entries.len() != before {
            self.dirty = true;
        }
    }

    pub fn save(&mut self, root: &Path) {
        if !self.dirty {
            return;
        }
        let path = file(root);
        let Some(dir) = path.parent() else { return };
        let Ok(text) = serde_json::to_string(self) else {
            return;
        };
        if fs::create_dir_all(dir).is_err() {
            return;
        }
        let tmp = dir.join("cache.json.tmp");
        let ok = fs::write(&tmp, text).is_ok() && fs::rename(&tmp, &path).is_ok();
        if ok {
            self.dirty = false;
        } else {
            let _ = fs::remove_file(&tmp);
        }
    }
}
