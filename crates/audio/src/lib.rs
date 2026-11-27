pub mod duplicates;
mod error;
mod group;
mod layout;
mod names;
pub mod scan;
mod sidecar;
mod store;
pub mod tags;
pub mod transfer;
mod walk;

pub use error::{Error, Result};
pub use group::{Group, group};
pub use layout::{book_dir, sanitize};
pub use scan::{ScanProgress, ScanSummary, scan};
pub use store::{
    AudioStore, IndexedBook, IndexedFile, Issue, NewRoot, RemoteBook, RemoteSync, RootPatch,
    ScannedBook, ScannedFile,
};
pub use tags::{FileTags, LoftyReader, TagReader};
pub use walk::{AUDIO_EXTENSIONS, Entry, Kind, walk};
