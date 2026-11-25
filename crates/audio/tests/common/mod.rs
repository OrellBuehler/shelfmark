#![allow(dead_code)]

pub mod audio_gen;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

pub struct Dir(PathBuf);

impl Dir {
    pub fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "shelfmark-audio-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }
}

pub fn age(path: &Path, secs_ago: u64) {
    let t = std::time::SystemTime::now() - std::time::Duration::from_secs(secs_ago);
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_modified(t).unwrap();
}

impl Dir {
    pub fn old(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.write(rel, bytes);
        age(&p, 3600);
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
