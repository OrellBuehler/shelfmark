#![allow(dead_code)]

use std::io::Cursor;
use std::path::{Path, PathBuf};

use shelfmark_library::Library;
use shelfmark_model::Metadata;
use tempfile::TempDir;

pub fn new_library() -> (TempDir, Library) {
    let tmp = TempDir::new().unwrap();
    let lib = Library::create(&tmp.path().join("lib"))
        .unwrap()
        .with_unix_path_rules();
    (tmp, lib)
}

pub fn fixture_copy() -> Option<(TempDir, Library)> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/library/metadata.db");
    if !src.is_file() {
        eprintln!("fixture library not present, skipping");
        return None;
    }
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("lib");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(&src, dir.join("metadata.db")).unwrap();
    let lib = Library::open(&dir).unwrap().with_windows_path_rules();
    Some((tmp, lib))
}

pub fn png(alpha: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(12, 8, image::Rgba([200, 30, 30, alpha]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

pub fn write_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

pub fn meta(title: &str, authors: &[&str]) -> Metadata {
    Metadata {
        title: Some(title.into()),
        authors: authors.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

pub fn raw(lib: &Library) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(lib.dir().join("metadata.db")).unwrap();
    conn.create_scalar_function(
        "title_sort",
        -1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        |ctx| ctx.get::<String>(0),
    )
    .unwrap();
    conn
}
