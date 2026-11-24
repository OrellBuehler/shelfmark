mod convert;
mod css;
mod dom;
mod entities;
mod epub;
mod error;
mod font;
mod image;
mod index;
mod lang;
mod langtable;
mod markup;
mod palmdoc;
mod skeleton;
mod tbs;
mod util;
mod writer;

#[cfg(test)]
mod reference;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod verify;

use std::path::Path;

use shelfmark_model::Metadata;

pub use error::{Error, Result};

pub fn can_convert(from: &str, to: &str) -> bool {
    let from = from.trim().trim_start_matches('.').to_ascii_lowercase();
    let to = to.trim().trim_start_matches('.').to_ascii_lowercase();
    matches!(from.as_str(), "epub" | "kepub") && to == "azw3"
}

pub fn epub_to_azw3(src: &Path, dst: &Path, meta: &Metadata, cover: Option<&[u8]>) -> Result<()> {
    convert::convert_to_file(src, dst, meta, cover)
}
