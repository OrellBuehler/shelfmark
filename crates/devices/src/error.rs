use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("not a folder: {0}")]
    NotAFolder(PathBuf),
    #[error("{device} does not support the {format} format")]
    UnsupportedFormat { format: String, device: String },
    #[error("path is outside the device: {0}")]
    OutsideDevice(PathBuf),
    #[error("not found on device: {0}")]
    NotFound(PathBuf),
    #[error("not enough free space on the device: {needed} bytes needed, {free} free")]
    NoSpace { needed: u64, free: u64 },
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;
