use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("pdf: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid {format} file: {reason}")]
    Invalid {
        format: &'static str,
        reason: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn invalid(format: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid {
        format,
        reason: reason.into(),
    }
}
