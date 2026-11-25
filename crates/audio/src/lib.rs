mod error;
mod store;

pub use error::{Error, Result};
pub use store::{
    AudioStore, IndexedBook, IndexedFile, Issue, NewRoot, RootPatch, ScannedBook, ScannedFile,
};
