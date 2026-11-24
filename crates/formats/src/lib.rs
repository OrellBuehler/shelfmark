mod cbz;
mod epub;
mod error;
mod fb2;
mod lang;
mod mobi;
mod pdf;
pub mod sources;
mod util;
mod xml;

use std::path::Path;

use shelfmark_model::Metadata;

pub use error::{Error, Result};

pub const BOOK_EXTENSIONS: &[&str] = &[
    "epub", "kepub", "pdf", "mobi", "azw", "azw3", "azw4", "prc", "fb2", "cbz", "cbr", "cb7",
    "txt", "rtf", "docx", "odt", "htm", "html", "htmlz", "txtz", "djvu", "lit", "lrf", "pdb",
    "pml", "snb",
];

pub fn read_metadata(path: &Path) -> Result<Metadata> {
    std::fs::metadata(path)?;
    let parsed = match util::extension(path).as_str() {
        "epub" | "kepub" => epub::read_metadata(path),
        "pdf" => pdf::read_metadata(path),
        "mobi" | "azw" | "azw3" | "prc" => mobi::read_metadata(path),
        "fb2" => fb2::read_metadata(path),
        "cbz" => cbz::read_metadata(path),
        _ => Ok(Metadata::default()),
    };
    let mut meta = match parsed {
        Ok(m) => m,
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::Io(e)),
        Err(_) => Metadata::default(),
    };
    util::fill_from_file_name(&mut meta, path);
    Ok(meta)
}

pub fn read_cover(path: &Path) -> Result<Option<Vec<u8>>> {
    std::fs::metadata(path)?;
    let cover = match util::extension(path).as_str() {
        "epub" | "kepub" => epub::read_cover(path),
        "mobi" | "azw" | "azw3" | "prc" => mobi::read_cover(path),
        "fb2" => fb2::read_cover(path),
        "cbz" => cbz::read_cover(path),
        _ => Ok(None),
    };
    match cover {
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Io(e)),
        Err(_) => Ok(None),
        ok => ok,
    }
}

pub fn write_metadata(path: &Path, meta: &Metadata, cover: Option<&[u8]>) -> Result<bool> {
    match util::extension(path).as_str() {
        "epub" | "kepub" => epub::write_metadata(path, meta, cover),
        "pdf" => pdf::write_metadata(path, meta),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests;
