use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};

use shelfmark_devices::*;
use shelfmark_model::{DeviceKind, Metadata};
use tempfile::TempDir;

fn touch(p: &Path, content: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn kobo() -> TempDir {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join(".kobo/KoboReader.sqlite"), "");
    touch(
        &t.path().join(".kobo/version"),
        "N1234,4.1,4.38.1,4.38.1,4.38.1,00000000-0000-0000-0000-000000000391",
    );
    t
}

fn kindle() -> TempDir {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join("documents")).unwrap();
    fs::create_dir_all(t.path().join("system")).unwrap();
    t
}

fn meta(title: &str, author: &str) -> Metadata {
    Metadata {
        title: Some(title.into()),
        authors: vec![author.into()],
        uuid: Some("u-1".into()),
        ..Default::default()
    }
}

fn detect_one(root: &Path, label: Option<&str>) -> Option<shelfmark_model::Device> {
    detect_roots(&[(root.to_path_buf(), label.map(String::from))])
        .into_iter()
        .next()
}

#[test]
fn detects_kobo() {
    let t = kobo();
    let d = detect_one(t.path(), None).unwrap();
    assert_eq!(d.kind, DeviceKind::Kobo);
    assert_eq!(d.name, "Kobo Clara BW");
    assert_eq!(d.id, t.path().to_string_lossy());
    assert_eq!(d.books_dir, d.root);
    assert!(d.supported_formats.contains(&"KEPUB".to_string()));
    assert!(d.total_bytes > 0);
}

#[test]
fn kobo_needs_signature_files() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join(".kobo")).unwrap();
    assert!(detect_one(t.path(), None).is_none());
}

#[test]
fn detects_tolino_on_kobo_layout() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join(".kobo/version"), "N1,1,2,3,4,00000000-0000-0000-0000-000000000691");
    let d = detect_one(t.path(), None).unwrap();
    assert_eq!(d.kind, DeviceKind::Tolino);
    assert_eq!(d.supported_formats, ["EPUB", "PDF", "TXT"]);
}

#[test]
fn detects_old_tolino_by_label() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(detect_one(t.path(), Some("tolino")).unwrap().kind, DeviceKind::Tolino);
}

#[test]
fn detects_kindle() {
    let t = kindle();
    let d = detect_one(t.path(), None).unwrap();
    assert_eq!(d.kind, DeviceKind::Kindle);
    assert_eq!(d.books_dir, t.path().join("documents").to_string_lossy());
    assert!(d.supported_formats.contains(&"AZW3".to_string()));
}

#[test]
fn detects_pocketbook() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join("system/config")).unwrap();
    let d = detect_one(t.path(), None).unwrap();
    assert_eq!(d.kind, DeviceKind::PocketBook);
    assert_eq!(d.books_dir, t.path().join("Books").to_string_lossy());
}

#[test]
fn detects_generic_by_marker_only() {
    let t = tempfile::tempdir().unwrap();
    assert!(detect_one(t.path(), None).is_none());
    touch(&t.path().join("metadata.calibre"), "[]");
    assert_eq!(detect_one(t.path(), None).unwrap().kind, DeviceKind::Generic);

    let t2 = tempfile::tempdir().unwrap();
    touch(
        &t2.path().join(".shelfmark-device"),
        r#"{"name":"My Reader","booksDir":"Books/sub"}"#,
    );
    let d = detect_one(t2.path(), None).unwrap();
    assert_eq!(d.name, "My Reader");
    assert_eq!(d.books_dir, t2.path().join("Books/sub").to_string_lossy());

    let t3 = tempfile::tempdir().unwrap();
    touch(&t3.path().join(".shelfmark-device"), r#"{"booksDir":"../x"}"#);
    assert_eq!(detect_one(t3.path(), None).unwrap().books_dir, t3.path().to_string_lossy());
}

#[test]
fn detect_roots_dedups_and_never_panics() {
    let t = kobo();
    let list = detect_roots(&[
        (t.path().to_path_buf(), None),
        (t.path().to_path_buf(), None),
        (PathBuf::from("/definitely/not/here"), None),
    ]);
    assert_eq!(list.len(), 1);
    let _ = detect();
}

#[test]
fn folder_device_works() {
    let t = tempfile::tempdir().unwrap();
    let d = folder_device(t.path()).unwrap();
    assert_eq!(d.kind, DeviceKind::Folder);
    assert!(matches!(
        folder_device(&t.path().join("nope")),
        Err(Error::NotAFolder(_))
    ));
}

#[test]
fn lists_books_and_skips_hidden_dirs() {
    let t = kobo();
    touch(&t.path().join("Author/Book - Author.epub"), "aaa");
    touch(&t.path().join("Author/Other.kepub.epub"), "bb");
    touch(&t.path().join("loose.pdf"), "c");
    touch(&t.path().join("note.jpg"), "c");
    touch(&t.path().join(".kobo/kepub/hidden.epub"), "c");
    touch(&t.path().join(".Trashes/x.epub"), "c");
    touch(&t.path().join("system/x.epub"), "c");
    touch(&t.path().join("._mac.epub"), "c");
    let d = detect_one(t.path(), None).unwrap();
    let reader = |p: &Path| {
        p.to_string_lossy().contains("Book - Author").then(|| meta("The Book", "An Author"))
    };
    let books = list_books(&d, &reader).unwrap();
    let rels: Vec<_> = books.iter().map(|b| b.rel_path.as_str()).collect();
    assert_eq!(rels, ["Author/Book - Author.epub", "Author/Other.kepub.epub", "loose.pdf"]);
    assert_eq!(books[0].title, "The Book");
    assert_eq!(books[0].authors, ["An Author"]);
    assert_eq!(books[0].format, "EPUB");
    assert_eq!(books[0].size, 3);
    assert_eq!(books[0].uuid.as_deref(), Some("u-1"));
    assert!(books[0].modified.as_deref().unwrap().ends_with('Z'));
    assert_eq!(books[1].format, "KEPUB");
    assert_eq!(books[1].title, "Other");
    assert_eq!(books[2].title, "loose");
    assert!(books[2].authors.is_empty());
}

#[test]
fn kindle_listing_ignores_sdr() {
    let t = kindle();
    touch(&t.path().join("documents/a.azw3"), "x");
    touch(&t.path().join("documents/a.sdr/b.mobi"), "x");
    touch(&t.path().join("system/c.mobi"), "x");
    let d = detect_one(t.path(), None).unwrap();
    let books = list_books(&d, &|_| None).unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].rel_path, "documents/a.azw3");
}

#[test]
fn cache_avoids_rereads_and_invalidates_on_change() {
    let t = kobo();
    touch(&t.path().join("a.epub"), "123");
    let d = detect_one(t.path(), None).unwrap();
    let calls = Cell::new(0);
    let reader = |_: &Path| {
        calls.set(calls.get() + 1);
        Some(meta("T", "A"))
    };
    list_books(&d, &reader).unwrap();
    assert_eq!(calls.get(), 1);
    assert!(t.path().join(".shelfmark/cache.json").is_file());
    let again = list_books(&d, &reader).unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(again[0].title, "T");

    touch(&t.path().join("a.epub"), "123456");
    list_books(&d, &reader).unwrap();
    assert_eq!(calls.get(), 2);

    fs::remove_file(t.path().join("a.epub")).unwrap();
    list_books(&d, &reader).unwrap();
    let text = fs::read_to_string(t.path().join(".shelfmark/cache.json")).unwrap();
    assert!(!text.contains("a.epub"));
}

#[test]
fn corrupt_cache_is_ignored() {
    let t = kobo();
    touch(&t.path().join("a.epub"), "1");
    touch(&t.path().join(".shelfmark/cache.json"), "{not json");
    let d = detect_one(t.path(), None).unwrap();
    assert_eq!(list_books(&d, &|_| Some(meta("T", "A"))).unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn read_only_device_still_lists() {
    use std::os::unix::fs::PermissionsExt;
    let t = kobo();
    touch(&t.path().join("a.epub"), "1");
    let d = detect_one(t.path(), None).unwrap();
    fs::set_permissions(t.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let r = list_books(&d, &|_| None);
    fs::set_permissions(t.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(r.unwrap().len(), 1);
}

#[test]
fn send_sanitises_and_roundtrips() {
    let t = kobo();
    let d = detect_one(t.path(), None).unwrap();
    let src = tempfile::tempdir().unwrap();
    touch(&src.path().join("in.epub"), "content");
    let m = meta("What: A \"Title\"? / x.", "J. R. R. Tolkien.");
    let b = send_book(&d, &src.path().join("in.epub"), &m, "EPUB").unwrap();
    assert_eq!(b.rel_path, "J. R. R. Tolkien/What_ A _Title__ _ x - J. R. R. Tolkien.epub");
    assert_eq!(b.format, "EPUB");
    assert_eq!(b.size, 7);
    assert_eq!(fs::read_to_string(&b.path).unwrap(), "content");
    assert_eq!(b.uuid.as_deref(), Some("u-1"));

    let calls = Cell::new(0);
    let listed = list_books(&d, &|_| {
        calls.set(calls.get() + 1);
        None
    })
    .unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(listed[0].title, "What: A \"Title\"? / x.");

    touch(&src.path().join("in.epub"), "newer content");
    let b2 = send_book(&d, &src.path().join("in.epub"), &m, "epub").unwrap();
    assert_eq!(b2.path, b.path);
    assert_eq!(fs::read_to_string(&b2.path).unwrap(), "newer content");
    let leftovers: Vec<_> = fs::read_dir(Path::new(&b.path).parent().unwrap()).unwrap().collect();
    assert_eq!(leftovers.len(), 1);
}

#[test]
fn send_fallbacks_and_reserved_names() {
    let t = tempfile::tempdir().unwrap();
    let d = folder_device(t.path()).unwrap();
    let src = tempfile::tempdir().unwrap();
    touch(&src.path().join("Some Book.pdf"), "p");
    let b = send_book(&d, &src.path().join("Some Book.pdf"), &Metadata::default(), "PDF").unwrap();
    assert_eq!(b.rel_path, "Unknown/Some Book - Unknown.pdf");

    let m = Metadata {
        title: Some("CON".into()),
        authors: vec!["aux".into()],
        ..Default::default()
    };
    let b = send_book(&d, &src.path().join("Some Book.pdf"), &m, "PDF").unwrap();
    assert_eq!(b.rel_path, "_aux/_CON - _aux.pdf");

    let long = Metadata {
        title: Some("x".repeat(400)),
        authors: vec!["y".repeat(400)],
        ..Default::default()
    };
    let b = send_book(&d, &src.path().join("Some Book.pdf"), &long, "PDF").unwrap();
    assert!(b.rel_path.split('/').all(|c| c.len() <= 255));
}

#[test]
fn send_rejects_unsupported_format_and_handles_kepub() {
    let t = kindle();
    let d = detect_one(t.path(), None).unwrap();
    let src = tempfile::tempdir().unwrap();
    touch(&src.path().join("a.epub"), "x");
    let err = send_book(&d, &src.path().join("a.epub"), &meta("T", "A"), "EPUB").unwrap_err();
    assert!(matches!(err, Error::UnsupportedFormat { .. }));
    assert!(err.to_string().contains("EPUB"));

    let k = kobo();
    let kd = detect_one(k.path(), None).unwrap();
    let b = send_book(&kd, &src.path().join("a.epub"), &meta("T", "A"), "EPUB").unwrap();
    assert_eq!(b.rel_path, "A/T - A.epub");
    touch(&src.path().join("b.kepub.epub"), "x");
    let b = send_book(&kd, &src.path().join("b.kepub.epub"), &meta("T", "A"), "EPUB").unwrap();
    assert_eq!(b.rel_path, "A/T - A.kepub.epub");
    assert_eq!(b.format, "KEPUB");
    let listed = list_books(&kd, &|_| None).unwrap();
    assert_eq!(listed.len(), 2);
}

#[test]
fn kindle_send_goes_to_documents() {
    let t = kindle();
    let d = detect_one(t.path(), None).unwrap();
    let src = tempfile::tempdir().unwrap();
    touch(&src.path().join("a.azw3"), "x");
    let b = send_book(&d, &src.path().join("a.azw3"), &meta("T", "A"), "AZW3").unwrap();
    assert_eq!(b.rel_path, "documents/A/T - A.azw3");
}

#[test]
fn delete_removes_file_sidecar_and_empty_dirs() {
    let t = kindle();
    touch(&t.path().join("documents/A/T - A.azw3"), "x");
    touch(&t.path().join("documents/A/T - A.sdr/annot.txt"), "x");
    touch(&t.path().join("documents/A/T - A.apnx"), "x");
    touch(&t.path().join("documents/B/keep.azw3"), "x");
    touch(&t.path().join("documents/A/Other - A.azw3"), "x");
    let d = detect_one(t.path(), None).unwrap();
    delete_book(&d, &t.path().join("documents/A/T - A.azw3")).unwrap();
    assert!(!t.path().join("documents/A/T - A.azw3").exists());
    assert!(!t.path().join("documents/A/T - A.sdr").exists());
    assert!(!t.path().join("documents/A/T - A.apnx").exists());
    assert!(t.path().join("documents/A").exists());
    delete_book(&d, Path::new("documents/A/Other - A.azw3")).unwrap();
    assert!(!t.path().join("documents/A").exists());
    assert!(t.path().join("documents").exists());
    assert!(t.path().join("documents/B/keep.azw3").exists());
}

#[test]
fn delete_never_removes_root_or_books_dir_and_updates_cache() {
    let t = kobo();
    touch(&t.path().join("A/x.epub"), "1");
    let d = detect_one(t.path(), None).unwrap();
    list_books(&d, &|_| Some(meta("T", "A"))).unwrap();
    delete_book(&d, &t.path().join("A/x.epub")).unwrap();
    assert!(t.path().exists());
    assert!(!t.path().join("A").exists());
    let text = fs::read_to_string(t.path().join(".shelfmark/cache.json")).unwrap();
    assert!(!text.contains("x.epub"));
    assert!(matches!(
        delete_book(&d, &t.path().join("A/x.epub")),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn delete_refuses_outside_paths_and_non_books() {
    let t = kobo();
    let outside = tempfile::tempdir().unwrap();
    touch(&outside.path().join("victim.epub"), "x");
    touch(&t.path().join("a.epub"), "x");
    touch(&t.path().join("notes.txt.bak"), "x");
    let d = detect_one(t.path(), None).unwrap();

    let abs = delete_book(&d, &outside.path().join("victim.epub"));
    assert!(matches!(abs, Err(Error::OutsideDevice(_))));
    let trav = t.path().join("../").join(outside.path().file_name().unwrap()).join("victim.epub");
    assert!(delete_book(&d, &trav).is_err());
    assert!(matches!(
        delete_book(&d, Path::new("../x.epub")),
        Err(Error::OutsideDevice(_) | Error::NotFound(_))
    ));
    assert!(delete_book(&d, t.path()).is_err());
    assert!(delete_book(&d, &t.path().join(".kobo/KoboReader.sqlite")).is_err());
    assert!(delete_book(&d, &t.path().join("notes.txt.bak")).is_err());
    assert!(outside.path().join("victim.epub").exists());
    assert!(t.path().join(".kobo/KoboReader.sqlite").exists());
    assert!(t.path().join("a.epub").exists());
}

#[cfg(unix)]
#[test]
fn delete_refuses_symlink_escape() {
    let t = kobo();
    let outside = tempfile::tempdir().unwrap();
    touch(&outside.path().join("victim.epub"), "x");
    std::os::unix::fs::symlink(outside.path(), t.path().join("link")).unwrap();
    let d = detect_one(t.path(), None).unwrap();
    let r = delete_book(&d, &t.path().join("link/victim.epub"));
    assert!(matches!(r, Err(Error::OutsideDevice(_))));
    assert!(outside.path().join("victim.epub").exists());
}
