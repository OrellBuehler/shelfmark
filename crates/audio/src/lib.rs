mod error;
mod group;
mod names;
pub mod scan;
mod sidecar;
mod store;
pub mod tags;
mod walk;

pub use error::{Error, Result};
pub use group::{Group, group};
pub use scan::{ScanProgress, ScanSummary, scan};
pub use store::{
    AudioStore, IndexedBook, IndexedFile, Issue, NewRoot, RootPatch, ScannedBook, ScannedFile,
};
pub use tags::{FileTags, LoftyReader, TagReader};
pub use walk::{AUDIO_EXTENSIONS, Entry, Kind, walk};
