use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("the book is protected by DRM and cannot be converted")]
    Drm,
    #[error("invalid epub: {0}")]
    Invalid(String),
    #[error("conversion failed: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn invalid(reason: impl Into<String>) -> Error {
    Error::Invalid(reason.into())
}

pub(crate) fn internal(reason: impl Into<String>) -> Error {
    Error::Internal(reason.into())
}
