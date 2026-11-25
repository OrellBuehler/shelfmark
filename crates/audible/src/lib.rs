mod client;
mod identity;
mod sign;

pub use client::{RemoteBook, fetch_cover, fetch_library, parse_library};
pub use identity::{Account, default_libation_path, load_libation};
pub use sign::{Signer, signature_headers};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not read the Libation accounts file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Account(String),
    #[error("Audible answered with status {status}: {message}")]
    Api { status: u16, message: String },
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;
