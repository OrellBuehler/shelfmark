use std::path::PathBuf;

use shelfmark_model::BookId;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no calibre library (metadata.db) in {0}")]
    NotALibrary(PathBuf),
    #[error("a library already exists in {0}")]
    AlreadyExists(PathBuf),
    #[error("library schema version {0} is too old, open it in calibre once to upgrade it")]
    UnsupportedSchema(i64),
    #[error("book {0} not found")]
    BookNotFound(BookId),
    #[error("book {id} has no {format} format")]
    FormatNotFound { id: BookId, format: String },
    #[error("file not found: {0}")]
    FileMissing(PathBuf),
    #[error("invalid search: {0}")]
    Search(String),
    #[error("invalid {field}: {value}")]
    Invalid { field: &'static str, value: String },
}
