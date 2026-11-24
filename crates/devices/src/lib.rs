mod books;
mod cache;
mod detect;
mod error;
mod info;
mod time;

pub use books::{MetaReader, delete_book, list_books, send_book};
pub use detect::{detect, detect_roots, folder_device, probe};
pub use error::{Error, Result};
