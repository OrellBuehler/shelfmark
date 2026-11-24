use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database lock poisoned")]
    Poisoned,
    #[error("unsupported database schema version {0}")]
    SchemaVersion(i64),
    #[error("invalid data: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;
