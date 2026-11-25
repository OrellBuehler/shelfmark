use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;

const MIN_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum JobEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        phase: String,
        done: u64,
        total: u64,
        current: Option<String>,
    },
    Done {
        result: serde_json::Value,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

#[derive(Default)]
pub struct Jobs {
    next: AtomicU64,
    running: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl Jobs {
    pub fn start(&self) -> (u64, Arc<AtomicBool>) {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let flag = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap().insert(id, flag.clone());
        (id, flag)
    }

    pub fn finish(&self, id: u64) {
        self.running.lock().unwrap().remove(&id);
    }

    pub fn cancel(&self, id: u64) {
        if let Some(flag) = self.running.lock().unwrap().get(&id) {
            flag.store(true, Ordering::SeqCst);
        }
    }

    pub fn cancel_all(&self) {
        for flag in self.running.lock().unwrap().values() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

pub struct Reporter {
    channel: Channel<JobEvent>,
    last: Mutex<Option<Instant>>,
}

impl Reporter {
    pub fn new(channel: Channel<JobEvent>) -> Self {
        Self {
            channel,
            last: Mutex::new(None),
        }
    }

    pub fn progress(&self, phase: &str, done: u64, total: u64, current: Option<String>) {
        let mut last = self.last.lock().unwrap();
        let now = Instant::now();
        if last.is_some_and(|t| now.duration_since(t) < MIN_INTERVAL) && done != total {
            return;
        }
        *last = Some(now);
        let _ = self.channel.send(JobEvent::Progress {
            phase: phase.to_string(),
            done,
            total,
            current,
        });
    }

    pub fn end(&self, event: JobEvent) {
        let _ = self.channel.send(event);
    }
}
