mod common;

use std::path::Path;

use common::*;
use shelfmark_library::{Error, Library};
use shelfmark_model::*;

fn list(lib: &Library, search: &str) -> Vec<BookSummary> {
    lib.list_books(&ListQuery {
        search: Some(search.into()),
        sort_by: SortField::Title,
        descending: false,
    })
    .unwrap()
}

fn ids(lib: &Library, search: &str) -> Vec<i64> {
    let mut v: Vec<i64> = list(lib, search).iter().map(|b| b.id).collect();
    v.sort();
    v
}

fn count(conn: &rusqlite::Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn library_is_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<Library>();
}

#[test]
fn create_open_roundtrip() {
    let (tmp, lib) = new_library();
    assert_eq!(lib.book_count().unwrap(), 0);
    assert!(lib.dir().join("metadata.db").is_file());
    assert!(matches!(
        Library::create(lib.dir()),
        Err(Error::AlreadyExists(_))
    ));
    drop(lib);
    let lib = Library::open(&tmp.path().join("lib")).unwrap();
    assert_eq!(lib.book_count().unwrap(), 0);
    let conn = raw(&lib);
    assert_eq!(count(&conn, "PRAGMA user_version"), 28);
    assert_eq!(count(&conn, "PRAGMA application_id"), 0x63616c69);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM library_id"), 1);
    let ic: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ic, "ok");
    assert!(matches!(
        Library::open(&tmp.path().join("nothing")),
        Err(Error::NotALibrary(_))
    ));
}

#[test]
fn schema_matches_real_calibre_library() {
    let Some((_tmp, fixture)) = fixture_copy() else {
        return;
    };
    let (_tmp2, lib) = new_library();
    let names = |lib: &Library| -> std::collections::BTreeSet<(String, String)> {
        let conn = raw(lib);
        let mut stmt = conn
            .prepare("SELECT type, name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    let a = names(&fixture);
    let b = names(&lib);
    let missing: Vec<_> = a.difference(&b).collect();
    assert!(
        missing.is_empty(),
        "objects in a real library missing from a fresh one: {missing:?}"
    );
    let extra: Vec<_> = b.difference(&a).collect();
    assert!(
        extra.iter().all(|(_, n)| n.contains("book_storage")),
        "unexpected extra objects: {extra:?}"
    );
}

#[test]
fn add_book_with_files_and_cover() {
    let (tmp, lib) = new_library();
    let src = tmp.path().join("src");
    let epub = write_file(&src, "whatever.epub", b"epub-bytes");
    let pdf = write_file(&src, "other.PDF", b"pdf-bytes-longer");
    let meta = Metadata {
        title: Some("The Hobbit: There and Back".into()),
        authors: vec!["J. R. R. Tolkien".into(), "Jane Doe".into()],
        series: Some("Middle Earth".into()),
        tags: vec!["Fantasy".into(), "fantasy".into(), "Classic, Old".into()],
        publisher: Some("Allen & Unwin".into()),
        languages: vec!["en".into()],
        identifiers: [("ISBN".to_string(), "9780261102217".to_string())].into(),
        rating: Some(8),
        comments: Some("<p>A hobbit.</p>".into()),
        pubdate: Some("1937-09-21".into()),
        ..Default::default()
    };
    let id = lib.add_book(&meta, Some(&png(255)), &[epub, pdf]).unwrap();
    assert_eq!(id, 1);

    let book = lib.get_book(id).unwrap();
    assert_eq!(book.summary.title, "The Hobbit: There and Back");
    assert_eq!(book.summary.sort, "Hobbit: There and Back, The");
    assert_eq!(book.summary.authors, vec!["J. R. R. Tolkien", "Jane Doe"]);
    assert_eq!(book.summary.author_sort, "Tolkien, J. R. R. & Doe, Jane");
    assert_eq!(book.summary.series.as_deref(), Some("Middle Earth"));
    assert_eq!(book.summary.series_index, 1.0);
    assert_eq!(book.summary.tags, vec!["Fantasy", "Classic; Old"]);
    assert_eq!(book.summary.publisher.as_deref(), Some("Allen & Unwin"));
    assert_eq!(book.summary.languages, vec!["eng"]);
    assert_eq!(book.summary.rating, Some(8));
    assert_eq!(
        book.summary.identifiers.get("isbn").map(String::as_str),
        Some("9780261102217")
    );
    assert_eq!(book.summary.formats, vec!["EPUB", "PDF"]);
    assert_eq!(book.summary.size, 16);
    assert!(book.summary.has_cover);
    assert!(
        book.summary
            .pubdate
            .as_deref()
            .unwrap()
            .starts_with("1937-09-21T00:00:00")
    );
    assert_eq!(book.comments.as_deref(), Some("<p>A hobbit.</p>"));
    assert_eq!(book.uuid.len(), 36);
    assert_eq!(book.path, "J. R. R. Tolkien/The Hobbit_ There and Back (1)");
    assert_eq!(book.files.len(), 2);
    assert_eq!(
        book.files[0].file_name,
        "The Hobbit_ There and Back - J. R. R. Tolkien.epub"
    );

    let dir = lib.book_dir(id).unwrap();
    assert!(
        dir.join("The Hobbit_ There and Back - J. R. R. Tolkien.epub")
            .is_file()
    );
    assert!(
        dir.join("The Hobbit_ There and Back - J. R. R. Tolkien.pdf")
            .is_file()
    );
    let cover = lib.cover_path(id).unwrap().unwrap();
    assert_eq!(cover, dir.join("cover.jpg"));
    let bytes = std::fs::read(&cover).unwrap();
    assert_eq!(&bytes[..3], &[0xff, 0xd8, 0xff]);
    assert_eq!(
        lib.format_path(id, "epub").unwrap(),
        dir.join("The Hobbit_ There and Back - J. R. R. Tolkien.epub")
    );
    assert!(matches!(
        lib.format_path(id, "mobi"),
        Err(Error::FormatNotFound { .. })
    ));

    let conn = raw(&lib);
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM metadata_dirtied WHERE book=1"),
        1
    );
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM books_pages_link WHERE book=1"),
        1
    );
    let m = lib.metadata(id).unwrap();
    assert_eq!(m.title_sort.as_deref(), Some("Hobbit: There and Back, The"));
    assert_eq!(m.uuid.as_deref(), Some(book.uuid.as_str()));
}

#[test]
fn add_book_minimal_and_missing_file() {
    let (tmp, lib) = new_library();
    let id = lib.add_book(&Metadata::default(), None, &[]).unwrap();
    let b = lib.get_book(id).unwrap();
    assert_eq!(b.summary.title, "Unknown");
    assert_eq!(b.summary.authors, vec!["Unknown"]);
    assert_eq!(b.summary.pubdate, None);
    assert_eq!(b.summary.rating, None);
    assert_eq!(b.path, "Unknown/Unknown (1)");
    assert!(!lib.dir().join("Unknown").exists());
    assert_eq!(lib.cover_path(id).unwrap(), None);
    let missing = tmp.path().join("nope.epub");
    let err = lib
        .add_book(&meta("X", &["Y"]), None, &[missing])
        .unwrap_err();
    assert!(matches!(err, Error::FileMissing(_)));
    assert_eq!(lib.book_count().unwrap(), 1);
    assert!(matches!(
        lib.add_book(&meta("X", &["Y"]), Some(b"not an image"), &[]),
        Err(Error::Image(_))
    ));
    assert_eq!(lib.book_count().unwrap(), 1);
}

#[test]
fn update_renames_folder_and_files() {
    let (tmp, lib) = new_library();
    let epub = write_file(&tmp.path().join("src"), "a.epub", b"1234");
    let id = lib
        .add_book(
            &meta("Old Title", &["Old Author"]),
            Some(&png(255)),
            &[epub],
        )
        .unwrap();
    let old_dir = lib.book_dir(id).unwrap();
    assert!(old_dir.join("Old Title - Old Author.epub").is_file());
    std::fs::write(old_dir.join("extra.txt"), b"keep me").unwrap();

    let patch = BookPatch {
        title: Some("Neue Überschrift".into()),
        authors: Some(vec!["Jörg Müller".into()]),
        ..Default::default()
    };
    let book = lib.update_book(id, &patch).unwrap();
    assert_eq!(book.summary.title, "Neue Überschrift");
    assert_eq!(book.summary.authors, vec!["Jörg Müller"]);
    assert_eq!(book.summary.author_sort, "Müller, Jörg");
    assert_eq!(book.path, "Jorg Muller/Neue Uberschrift (1)");
    assert_eq!(
        book.files[0].file_name,
        "Neue Uberschrift - Jorg Muller.epub"
    );
    let new_dir = lib.book_dir(id).unwrap();
    assert!(!old_dir.exists());
    assert!(
        !lib.dir().join("Old Author").exists(),
        "empty author dir removed"
    );
    assert!(
        new_dir
            .join("Neue Uberschrift - Jorg Muller.epub")
            .is_file()
    );
    assert!(new_dir.join("cover.jpg").is_file());
    assert_eq!(
        std::fs::read(new_dir.join("extra.txt")).unwrap(),
        b"keep me"
    );
    assert_eq!(
        lib.format_path(id, "EPUB").unwrap(),
        new_dir.join("Neue Uberschrift - Jorg Muller.epub")
    );

    let conn = raw(&lib);
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM authors"),
        1,
        "orphaned author removed"
    );
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM metadata_dirtied WHERE book=1"),
        1
    );
    let sort: String = conn
        .query_row("SELECT sort FROM books WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sort, "Neue Überschrift");
    let name: String = conn
        .query_row("SELECT name FROM data WHERE book=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "Neue Uberschrift - Jorg Muller");
}

#[test]
fn update_only_title_keeps_author_dir() {
    let (tmp, lib) = new_library();
    let epub = write_file(&tmp.path().join("src"), "a.epub", b"1234");
    let id = lib
        .add_book(&meta("One", &["Same Author"]), None, &[epub])
        .unwrap();
    lib.update_book(
        id,
        &BookPatch {
            title: Some("Two".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        lib.dir()
            .join("Same Author/Two (1)/Two - Same Author.epub")
            .is_file()
    );
    assert!(!lib.dir().join("Same Author/One (1)").exists());
    lib.update_book(
        id,
        &BookPatch {
            title: Some("Two".into()),
            tags: Some(vec!["x".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        lib.dir()
            .join("Same Author/Two (1)/Two - Same Author.epub")
            .is_file()
    );
}

#[test]
fn update_tolerates_missing_folders() {
    let (_tmp, lib) = new_library();
    let id = lib
        .add_book(&meta("Ghost", &["Some One"]), None, &[])
        .unwrap();
    let b = lib
        .update_book(
            id,
            &BookPatch {
                title: Some("Phantom".into()),
                authors: Some(vec!["Other Person".into()]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(b.path, "Other Person/Phantom (1)");
    assert!(!lib.dir().join("Other Person").exists());
}

#[test]
fn update_fields_and_clearing() {
    let (_tmp, lib) = new_library();
    let id = lib
        .add_book(&meta("Book", &["Ann Author"]), None, &[])
        .unwrap();
    let patch = BookPatch {
        series: Some("Saga".into()),
        series_index: Some(2.5),
        tags: Some(vec!["a".into(), "B".into()]),
        publisher: Some("Pub".into()),
        pubdate: Some("2020-02-03T10:00:00+00:00".into()),
        languages: Some(vec!["German".into(), "fr".into(), "xx".into()]),
        identifiers: Some(
            [
                ("ISBN".to_string(), "1,2".to_string()),
                ("empty".into(), " ".into()),
            ]
            .into(),
        ),
        rating: Some(6),
        comments: Some("  hello  ".into()),
        title_sort: Some("Custom Sort".into()),
        author_sort: Some("Custom, Author".into()),
        ..Default::default()
    };
    let b = lib.update_book(id, &patch).unwrap().summary;
    assert_eq!(b.series.as_deref(), Some("Saga"));
    assert_eq!(b.series_index, 2.5);
    assert_eq!(b.tags, vec!["a", "B"]);
    assert_eq!(b.publisher.as_deref(), Some("Pub"));
    assert!(
        b.pubdate
            .as_deref()
            .unwrap()
            .starts_with("2020-02-03T10:00:00")
    );
    assert_eq!(b.languages, vec!["deu", "fra"]);
    assert_eq!(b.identifiers.len(), 1);
    assert_eq!(b.identifiers["isbn"], "1|2");
    assert_eq!(b.rating, Some(6));
    assert_eq!(b.sort, "Custom Sort");
    assert_eq!(b.author_sort, "Custom, Author");
    assert_eq!(lib.get_book(id).unwrap().comments.as_deref(), Some("hello"));

    let clear = BookPatch {
        series: Some("".into()),
        tags: Some(vec![]),
        publisher: Some("".into()),
        pubdate: Some("".into()),
        languages: Some(vec![]),
        identifiers: Some(Default::default()),
        rating: Some(0),
        comments: Some("".into()),
        title_sort: Some("".into()),
        author_sort: Some("".into()),
        ..Default::default()
    };
    let b = lib.update_book(id, &clear).unwrap();
    let s = b.summary;
    assert_eq!(
        (s.series, s.publisher, s.pubdate, s.rating),
        (None, None, None, None)
    );
    assert!(s.tags.is_empty() && s.languages.is_empty() && s.identifiers.is_empty());
    assert_eq!(b.comments, None);
    assert_eq!(s.sort, "Book");
    assert_eq!(s.author_sort, "Author, Ann");

    let conn = raw(&lib);
    for t in ["series", "tags", "publishers", "ratings", "languages"] {
        assert_eq!(
            count(&conn, &format!("SELECT COUNT(*) FROM {t}")),
            0,
            "{t} orphans removed"
        );
    }
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM comments"), 0);
    let pubdate: String = conn
        .query_row("SELECT pubdate FROM books", [], |r| r.get(0))
        .unwrap();
    assert_eq!(pubdate, "0101-01-01 00:00:00+00:00");

    assert!(matches!(
        lib.update_book(
            id,
            &BookPatch {
                pubdate: Some("not a date".into()),
                ..Default::default()
            }
        ),
        Err(Error::Invalid { .. })
    ));
    assert!(matches!(
        lib.update_book(99, &BookPatch::default()),
        Err(Error::BookNotFound(99))
    ));
}

#[test]
fn shared_items_and_case_changes() {
    let (_tmp, lib) = new_library();
    let a = lib
        .add_book(
            &Metadata {
                tags: vec!["sci-fi".into()],
                ..meta("A", &["Zed Writer"])
            },
            None,
            &[],
        )
        .unwrap();
    let b = lib
        .add_book(
            &Metadata {
                tags: vec!["Sci-Fi".into()],
                ..meta("B", &["zed writer"])
            },
            None,
            &[],
        )
        .unwrap();
    let conn = raw(&lib);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM tags"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM authors"), 1);
    assert_eq!(lib.get_book(b).unwrap().summary.tags, vec!["sci-fi"]);

    lib.update_book(
        a,
        &BookPatch {
            tags: Some(vec!["SCI-FI".into()]),
            authors: Some(vec!["ZED WRITER".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(lib.get_book(b).unwrap().summary.tags, vec!["SCI-FI"]);
    assert_eq!(lib.get_book(b).unwrap().summary.authors, vec!["ZED WRITER"]);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM tags"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM metadata_dirtied"), 2);

    lib.update_book(
        a,
        &BookPatch {
            tags: Some(vec![]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM tags"),
        1,
        "tag still used by the other book"
    );
    lib.delete_books(&[b]).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM tags"), 0);
}

#[test]
fn bulk_update_series_numbering_and_tags() {
    let (_tmp, lib) = new_library();
    let mut ids = Vec::new();
    for t in ["C", "A", "B"] {
        let m = Metadata {
            tags: vec!["keep".into(), "drop".into()],
            ..meta(t, &["Au Thor"])
        };
        ids.push(lib.add_book(&m, None, &[]).unwrap());
    }
    let patch = BulkPatch {
        series: Some("Numbered".into()),
        series_start: Some(3.0),
        add_tags: vec!["new".into(), "KEEP".into()],
        remove_tags: vec!["Drop".into()],
        publisher: Some("Same Pub".into()),
        rating: Some(10),
        ..Default::default()
    };
    lib.bulk_update(&[ids[1], ids[2], ids[0]], &patch).unwrap();
    let get = |id| lib.get_book(id).unwrap().summary;
    assert_eq!(
        (
            get(ids[1]).series_index,
            get(ids[2]).series_index,
            get(ids[0]).series_index
        ),
        (3.0, 4.0, 5.0)
    );
    assert_eq!(get(ids[0]).series.as_deref(), Some("Numbered"));
    assert_eq!(get(ids[0]).tags, vec!["keep", "new"]);
    assert_eq!(get(ids[0]).publisher.as_deref(), Some("Same Pub"));
    assert_eq!(get(ids[0]).rating, Some(10));

    lib.bulk_update(
        &ids,
        &BulkPatch {
            clear_tags: true,
            add_tags: vec!["only".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(get(ids[2]).tags, vec!["only"]);
    let conn = raw(&lib);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM tags"), 1);

    lib.bulk_update(
        &ids,
        &BulkPatch {
            authors: Some(vec!["New One".into(), "New Two".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(get(ids[1]).authors, vec!["New One", "New Two"]);
    assert_eq!(
        lib.get_book(ids[1]).unwrap().path,
        format!("New One/A ({})", ids[1])
    );

    lib.bulk_update(
        &ids,
        &BulkPatch {
            series: Some("".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(get(ids[0]).series, None);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM series"), 0);
}

#[test]
fn covers() {
    let (tmp, lib) = new_library();
    let id = lib.add_book(&meta("Cov", &["Er"]), None, &[]).unwrap();
    assert!(!lib.get_book(id).unwrap().summary.has_cover);
    lib.set_cover(id, &png(0)).unwrap();
    let p = lib.cover_path(id).unwrap().unwrap();
    let img = image::open(&p).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (12, 8));
    assert_eq!(
        img.get_pixel(0, 0).0,
        [255, 255, 255],
        "transparent pixels flattened onto white"
    );
    assert!(lib.get_book(id).unwrap().summary.has_cover);

    let jpeg = std::fs::read(&p).unwrap();
    lib.set_cover(id, &jpeg).unwrap();
    assert_eq!(
        std::fs::read(&p).unwrap(),
        jpeg,
        "jpeg input is stored unchanged"
    );
    assert!(lib.set_cover(id, b"garbage").is_err());

    lib.update_book(
        id,
        &BookPatch {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        lib.cover_path(id)
            .unwrap()
            .unwrap()
            .ends_with("Er/Renamed (1)/cover.jpg")
    );

    lib.remove_cover(id).unwrap();
    assert_eq!(lib.cover_path(id).unwrap(), None);
    assert!(!lib.get_book(id).unwrap().summary.has_cover);
    lib.remove_cover(id).unwrap();
    let _ = tmp;
}

#[test]
fn add_and_remove_formats() {
    let (tmp, lib) = new_library();
    let src = tmp.path().join("src");
    let id = lib.add_book(&meta("Fmt", &["Au"]), None, &[]).unwrap();
    lib.add_format(id, &write_file(&src, "x.mobi", b"mobi"))
        .unwrap();
    lib.add_format(id, &write_file(&src, "y.EPUB", b"epubepub"))
        .unwrap();
    let b = lib.get_book(id).unwrap();
    assert_eq!(b.summary.formats, vec!["MOBI", "EPUB"]);
    assert_eq!(b.summary.size, 8);
    lib.add_format(id, &write_file(&src, "z.epub", b"new"))
        .unwrap();
    assert_eq!(
        std::fs::read(lib.format_path(id, "epub").unwrap()).unwrap(),
        b"new"
    );
    assert_eq!(lib.get_book(id).unwrap().files.len(), 2);

    let epub_path = lib.format_path(id, "EPUB").unwrap();
    lib.remove_format(id, "epub").unwrap();
    assert!(!epub_path.exists());
    assert_eq!(lib.get_book(id).unwrap().summary.formats, vec!["MOBI"]);
    let trashed = lib.dir().join(".caltrash/f/1");
    assert_eq!(std::fs::read(trashed.join("epub")).unwrap(), b"new");
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(trashed.join("metadata.json")).unwrap()).unwrap();
    assert_eq!(meta["title"], "Fmt");
    assert_eq!(meta["authors"][0], "Au");
    assert!(matches!(
        lib.remove_format(id, "epub"),
        Err(Error::FormatNotFound { .. })
    ));
    assert!(matches!(
        lib.add_format(id, Path::new("/nonexistent/file.epub")),
        Err(Error::FileMissing(_))
    ));
}

#[test]
fn delete_books_uses_calibre_trash() {
    let (tmp, lib) = new_library();
    let src = tmp.path().join("src");
    let meta = Metadata {
        series: Some("S".into()),
        tags: vec!["t".into()],
        rating: Some(4),
        comments: Some("c".into()),
        identifiers: [("isbn".to_string(), "1".to_string())].into(),
        languages: vec!["eng".into()],
        ..meta("Doomed & <Co>", &["Du Mmy"])
    };
    let id = lib
        .add_book(
            &meta,
            Some(&png(255)),
            &[write_file(&src, "a.epub", b"data")],
        )
        .unwrap();
    let keep = lib
        .add_book(&self::meta("Keeper", &["Du Mmy"]), None, &[])
        .unwrap();
    let dir = lib.book_dir(id).unwrap();
    assert!(dir.is_dir());
    lib.delete_books(&[id, 12345]).unwrap();

    assert_eq!(lib.book_count().unwrap(), 1);
    assert!(lib.get_book(keep).is_ok());
    assert!(matches!(lib.get_book(id), Err(Error::BookNotFound(_))));
    assert!(!dir.exists());
    let trash = lib.dir().join(".caltrash/b").join(id.to_string());
    assert!(
        trash.join("Doomed & _Co_ - Du Mmy.epub").is_file(),
        "{:?}",
        std::fs::read_dir(&trash).map(|d| d.collect::<Vec<_>>())
    );
    assert!(trash.join("cover.jpg").is_file());
    assert!(lib.dir().join(".caltrash/f").is_dir());
    let opf = std::fs::read_to_string(trash.join("metadata.opf")).unwrap();
    assert!(opf.contains("<dc:title>Doomed &amp; &lt;Co&gt;</dc:title>"));
    assert!(opf.contains("opf:file-as=\"Mmy, Du\""));
    assert!(opf.contains("<dc:subject>t</dc:subject>"));
    assert!(opf.contains("calibre:series"));
    assert!(opf.contains("opf:scheme=\"ISBN\""));
    assert!(opf.contains("<dc:language>eng</dc:language>"));
    assert!(opf.contains("calibre:rating\" content=\"4.0\""));

    let conn = raw(&lib);
    for t in [
        "books_authors_link",
        "books_tags_link",
        "books_series_link",
        "books_ratings_link",
        "books_languages_link",
        "data",
        "identifiers",
        "comments",
        "metadata_dirtied",
    ] {
        let n = count(&conn, &format!("SELECT COUNT(*) FROM {t} WHERE book={id}"));
        assert_eq!(n, 0, "{t}");
    }
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM tags"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM series"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM books_pages_link"), 1);

    let mtime = std::fs::metadata(&trash).unwrap().modified().unwrap();
    assert!(
        mtime.elapsed().unwrap().as_secs() < 60,
        "trash entry must be fresh so calibre does not expire it at once"
    );
}

#[test]
fn saved_searches_roundtrip_and_use() {
    let (_tmp, lib) = new_library();
    assert!(lib.saved_searches().unwrap().is_empty());
    lib.add_book(
        &Metadata {
            tags: vec!["unread".into()],
            ..meta("Alpha", &["A B"])
        },
        None,
        &[],
    )
    .unwrap();
    lib.add_book(
        &Metadata {
            tags: vec!["read".into()],
            ..meta("Beta", &["A B"])
        },
        None,
        &[],
    )
    .unwrap();
    let saved = vec![
        SavedSearch {
            name: "To Read".into(),
            query: "tags:=unread".into(),
        },
        SavedSearch {
            name: "Ümlaut".into(),
            query: "title:beta".into(),
        },
        SavedSearch {
            name: "Combined".into(),
            query: "search:\"to read\" or search:=ümlaut".into(),
        },
    ];
    lib.set_saved_searches(&saved).unwrap();
    let got = lib.saved_searches().unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].name, "Combined");
    let conn = raw(&lib);
    let raw_val: String = conn
        .query_row(
            "SELECT val FROM preferences WHERE key='saved_searches'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        raw_val,
        "{\n  \"Combined\": \"search:\\\"to read\\\" or search:=ümlaut\",\n  \"To Read\": \"tags:=unread\",\n  \"Ümlaut\": \"title:beta\"\n}"
            .replace('ü', "\\u00fc")
            .replace('Ü', "\\u00dc")
    );
    assert_eq!(ids(&lib, "search:\"To Read\""), vec![1]);
    assert_eq!(ids(&lib, "search:combined"), vec![1, 2]);
    assert_eq!(ids(&lib, "not search:\"to read\""), vec![2]);
    assert!(
        lib.list_books(&ListQuery {
            search: Some("search:nope".into()),
            ..Default::default()
        })
        .is_err()
    );
    lib.set_saved_searches(&[SavedSearch {
        name: "R".into(),
        query: "search:r".into(),
    }])
    .unwrap();
    assert!(
        lib.list_books(&ListQuery {
            search: Some("search:r".into()),
            ..Default::default()
        })
        .is_err(),
        "recursion is an error"
    );
    lib.set_saved_searches(&[]).unwrap();
    assert!(lib.saved_searches().unwrap().is_empty());
    let raw_val: String = conn
        .query_row(
            "SELECT val FROM preferences WHERE key='saved_searches'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw_val, "{}");
}

#[test]
fn completions() {
    let (_tmp, lib) = new_library();
    lib.add_book(
        &Metadata {
            tags: vec!["b".into(), "A".into()],
            publisher: Some("Pub".into()),
            series: Some("Ser".into()),
            languages: vec!["de".into()],
            ..meta("T", &["Last, First"])
        },
        None,
        &[],
    )
    .unwrap();
    assert_eq!(lib.completions("tags").unwrap(), vec!["A", "b"]);
    assert_eq!(lib.completions("authors").unwrap(), vec!["Last, First"]);
    assert_eq!(lib.completions("publisher").unwrap(), vec!["Pub"]);
    assert_eq!(lib.completions("series").unwrap(), vec!["Ser"]);
    assert_eq!(lib.completions("languages").unwrap(), vec!["deu"]);
    assert!(lib.completions("bogus").is_err());
}

fn sample(lib: &Library, tmp: &Path) -> Vec<i64> {
    let src = tmp.join("sample-src");
    let mk = |title: &str, authors: &[&str], f: &dyn Fn(&mut Metadata), files: &[&str]| {
        let mut m = meta(title, authors);
        f(&mut m);
        let paths: Vec<_> = files
            .iter()
            .map(|e| write_file(&src, &format!("f.{e}"), b"0123456789"))
            .collect();
        lib.add_book(&m, None, &paths).unwrap()
    };
    vec![
        mk(
            "Dune",
            &["Frank Herbert"],
            &|m| {
                m.series = Some("Dune Chronicles".into());
                m.series_index = Some(1.0);
                m.tags = vec!["Science Fiction".into(), "Classic".into()];
                m.rating = Some(10);
                m.publisher = Some("Chilton".into());
                m.languages = vec!["eng".into()];
                m.pubdate = Some("1965-08-01".into());
                m.identifiers = [("isbn".to_string(), "9780441172719".to_string())].into();
                m.comments = Some("<p>Desert planet and spice.</p>".into());
            },
            &["epub", "mobi"],
        ),
        mk(
            "Dune Messiah",
            &["Frank Herbert"],
            &|m| {
                m.series = Some("Dune Chronicles".into());
                m.series_index = Some(2.0);
                m.tags = vec!["Science Fiction".into()];
                m.rating = Some(6);
                m.languages = vec!["eng".into()];
                m.pubdate = Some("1969-10-01".into());
            },
            &["epub"],
        ),
        mk(
            "Émile et les détectives",
            &["Erich Kästner"],
            &|m| {
                m.tags = vec!["Kids".into()];
                m.languages = vec!["deu".into()];
                m.rating = Some(3);
                m.pubdate = Some("2021-03-04".into());
                m.identifiers = [("goodreads".to_string(), "42".to_string())].into();
            },
            &["pdf"],
        ),
        mk(
            "The Rust Programming Language",
            &["Steve Klabnik", "Carol Nichols"],
            &|m| {
                m.tags = vec!["Programming".into()];
                m.publisher = Some("No Starch".into());
                m.languages = vec!["eng".into()];
                m.comments = Some("Learn <b>Rust</b>, a systems language. 100% safe!".into());
            },
            &["epub", "pdf"],
        ),
        mk("Untagged", &["Nobody"], &|_| {}, &[]),
    ]
}

#[test]
fn search_semantics() {
    let (tmp, lib) = new_library();
    let id = sample(&lib, tmp.path());
    let (dune, messiah, emile, rust, plain) = (id[0], id[1], id[2], id[3], id[4]);
    let s = |q: &str| ids(&lib, q);

    assert_eq!(s(""), id);
    assert_eq!(s("   "), id);
    assert_eq!(s("dune"), vec![dune, messiah]);
    assert_eq!(s("DUNE messiah"), vec![messiah]);
    assert_eq!(s("title:=dune"), vec![dune]);
    assert_eq!(s("title:\"=Dune Messiah\""), vec![messiah]);
    assert_eq!(s("herbert"), vec![dune, messiah]);
    assert_eq!(s("author:herbert"), vec![dune, messiah]);
    assert_eq!(s("authors:\"=Frank Herbert\""), vec![dune, messiah]);
    assert_eq!(s("authors:\"=herbert\""), Vec::<i64>::new());
    assert_eq!(s("emile"), vec![emile]);
    assert_eq!(s("kastner"), vec![emile]);
    assert_eq!(s("title:detectives"), vec![emile]);
    assert_eq!(s("title:^emile"), vec![emile]);
    assert_eq!(s("title:\"~^du.e$\""), vec![dune]);
    assert_eq!(s("title:~messiah$"), vec![messiah]);
    assert_eq!(s("tag:\"=science fiction\""), vec![dune, messiah]);
    assert_eq!(
        s("tags:science-fiction"),
        vec![dune, messiah],
        "punctuation and spaces are ignored by contains"
    );
    assert_eq!(s("tags:fiction and not tags:classic"), vec![messiah]);
    assert_eq!(
        s("(tags:kids or tags:programming) and not title:rust"),
        vec![emile]
    );
    assert_eq!(s("tags:kids or tags:programming"), vec![emile, rust]);
    assert_eq!(s("not tags:true"), vec![plain]);
    assert_eq!(s("tags:false"), vec![plain]);
    assert_eq!(s("series:true"), vec![dune, messiah]);
    assert_eq!(s("series:false"), vec![emile, rust, plain]);
    assert_eq!(s("series:\"=dune chronicles\""), vec![dune, messiah]);
    assert_eq!(s("publisher:chilton"), vec![dune]);
    assert_eq!(s("publisher:false"), vec![messiah, emile, plain]);
    assert_eq!(s("comments:spice"), vec![dune]);
    assert_eq!(s("comments:\"100%\""), vec![rust]);
    assert_eq!(s("spice"), vec![dune]);
    assert_eq!(s("comments:true"), vec![dune, rust]);
    assert_eq!(s("formats:epub"), vec![dune, messiah, rust]);
    assert_eq!(s("formats:=PDF"), vec![emile, rust]);
    assert_eq!(s("format:mobi"), vec![dune]);
    assert_eq!(s("formats:false"), vec![plain]);
    assert_eq!(s("formats:#>1"), vec![dune, rust]);
    assert_eq!(s("authors:#>1"), vec![rust]);
    assert_eq!(s("languages:eng"), vec![dune, messiah, rust]);
    assert_eq!(s("languages:german"), vec![emile]);
    assert_eq!(s("language:\"=de\""), vec![emile]);
    assert_eq!(s("languages:false"), vec![plain]);
    assert_eq!(s("identifiers:isbn:9780441172719"), vec![dune]);
    assert_eq!(s("isbn:9780441172719"), vec![dune]);
    assert_eq!(s("isbn:978"), vec![dune]);
    assert_eq!(s("identifiers:\"=isbn:\""), vec![dune]);
    assert_eq!(s("identifiers:=goodreads:42"), vec![emile]);
    assert_eq!(s("identifiers:true"), vec![dune, emile]);
    assert_eq!(
        s("identifiers:isbn:false"),
        vec![messiah, emile, rust, plain]
    );
    assert_eq!(s("9780441172719"), vec![dune]);
}

#[test]
fn search_numbers_dates_and_sizes() {
    let (tmp, lib) = new_library();
    let id = sample(&lib, tmp.path());
    let (dune, messiah, emile, rust, plain) = (id[0], id[1], id[2], id[3], id[4]);
    let s = |q: &str| ids(&lib, q);

    assert_eq!(s("rating:5"), vec![dune]);
    assert_eq!(s("rating:=3"), vec![messiah]);
    assert_eq!(s("rating:>3"), vec![dune]);
    assert_eq!(s("rating:>=3"), vec![dune, messiah]);
    assert_eq!(s("rating:<3"), vec![emile]);
    assert_eq!(s("rating:1"), vec![emile], "calibre compares whole stars");
    assert_eq!(
        s("rating:!=5"),
        vec![messiah, emile],
        "unrated books never match a comparison"
    );
    assert_eq!(s("rating:true"), vec![dune, messiah, emile]);
    assert_eq!(s("rating:false"), vec![rust, plain]);
    assert_eq!(s("rating:1.5"), vec![emile]);
    assert!(
        lib.list_books(&ListQuery {
            search: Some("rating:abc".into()),
            ..Default::default()
        })
        .is_err()
    );
    assert_eq!(s("id:2"), vec![messiah]);
    assert_eq!(s("id:>3"), vec![rust, plain]);
    assert_eq!(s("series_index:2"), vec![messiah]);
    assert_eq!(s("size:>0"), vec![dune, messiah, emile, rust]);
    assert_eq!(s("size:10"), vec![dune, messiah, emile, rust]);
    assert_eq!(s("size:>1k"), Vec::<i64>::new());
    assert_eq!(s("size:false"), vec![plain]);
    assert_eq!(s("cover:true"), Vec::<i64>::new());
    assert_eq!(s("cover:false"), id);
    assert_eq!(s("pubdate:>1966"), vec![messiah, emile]);
    assert_eq!(s("pubdate:1965"), vec![dune]);
    assert_eq!(s("pubdate:<=1965-08"), vec![dune]);
    assert_eq!(s("pubdate:=2021-03-04"), vec![emile]);
    assert_eq!(s("pubdate:false"), vec![rust, plain]);
    assert_eq!(s("pubdate:true"), vec![dune, messiah, emile]);
    assert_eq!(s("date:today"), id);
    assert_eq!(s("date:thisyear"), id);
    assert_eq!(s("date:>1daysago"), id);
    assert_eq!(s("date:<30daysago"), Vec::<i64>::new());
    assert_eq!(s("date:>=2020-01"), id);
    assert_eq!(s("last_modified:thismonth"), id);
    assert!(
        lib.list_books(&ListQuery {
            search: Some("date:not-a-date".into()),
            ..Default::default()
        })
        .is_err()
    );
    let conn = raw(&lib);
    conn.execute(
        "UPDATE books SET timestamp='2015-05-05 12:00:00.000000+00:00' WHERE id=?1",
        [dune],
    )
    .unwrap();
    assert_eq!(s("date:2015"), vec![dune]);
    assert_eq!(s("date:<2016"), vec![dune]);
    assert_eq!(s("date:>2016 and rating:>3"), Vec::<i64>::new());
}

#[test]
fn search_errors_and_edge_cases() {
    let (tmp, lib) = new_library();
    let id = sample(&lib, tmp.path());
    let bad = |q: &str| {
        lib.list_books(&ListQuery {
            search: Some(q.into()),
            ..Default::default()
        })
        .is_err()
    };
    assert!(bad("(dune"));
    assert!(bad("dune or"));
    assert!(bad("title:~(unclosed"));
    assert!(bad("title:\"~[\""));
    assert_eq!(ids(&lib, "title:\"\""), Vec::<i64>::new());
    assert_eq!(ids(&lib, "nosuchfield:thing"), Vec::<i64>::new());
    assert_eq!(ids(&lib, "DUNE OR rust"), vec![id[0], id[1], id[3]]);
    assert_eq!(ids(&lib, "dune AND NOT messiah"), vec![id[0]]);
    assert_eq!(ids(&lib, "\"dune messiah\""), vec![id[1]]);
    assert_eq!(ids(&lib, "title:\"\"\"dune\"\"\""), vec![id[0], id[1]]);
}

#[test]
fn list_sorting() {
    let (tmp, lib) = new_library();
    let id = sample(&lib, tmp.path());
    let titles = |by: SortField, desc: bool| -> Vec<String> {
        lib.list_books(&ListQuery {
            search: None,
            sort_by: by,
            descending: desc,
        })
        .unwrap()
        .into_iter()
        .map(|b| b.title)
        .collect()
    };
    assert_eq!(
        titles(SortField::Title, false),
        vec![
            "Dune",
            "Dune Messiah",
            "Émile et les détectives",
            "The Rust Programming Language",
            "Untagged"
        ]
    );
    assert_eq!(titles(SortField::Title, true)[0], "Untagged");
    assert_eq!(
        titles(SortField::Author, false)[0],
        "Dune",
        "Herbert, Frank"
    );
    assert_eq!(
        titles(SortField::Series, false)[..2],
        ["Dune", "Dune Messiah"]
    );
    assert_eq!(
        titles(SortField::Series, true)[..2],
        ["Dune Messiah", "Dune"]
    );
    assert_eq!(
        titles(SortField::Rating, true)[..3],
        ["Dune", "Dune Messiah", "Émile et les détectives"]
    );
    assert_eq!(
        titles(SortField::Published, false)[..3],
        ["Dune", "Dune Messiah", "Émile et les détectives"]
    );
    assert_eq!(
        titles(SortField::Published, true)[0],
        "Émile et les détectives"
    );
    assert_eq!(
        titles(SortField::Publisher, false)[..2],
        ["Dune", "The Rust Programming Language"]
    );
    assert_eq!(titles(SortField::Size, true).last().unwrap(), "Untagged");
    assert_eq!(titles(SortField::Added, false).len(), 5);
    assert_eq!(titles(SortField::Modified, true).len(), 5);
    assert_eq!(id.len(), 5);
}

#[test]
fn categories_are_consistent_with_search() {
    let (tmp, lib) = new_library();
    sample(&lib, tmp.path());
    let cats = lib.categories(None).unwrap();
    let keys: Vec<_> = cats.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "authors",
            "series",
            "tags",
            "publisher",
            "languages",
            "formats",
            "rating",
            "identifiers"
        ]
    );
    let items = |cats: &[Category], k: &str| -> Vec<(String, u32)> {
        cats.iter()
            .find(|c| c.key == k)
            .unwrap()
            .items
            .iter()
            .map(|i| (i.name.clone(), i.count))
            .collect()
    };
    let p = |n: &str, c: u32| (n.to_string(), c);
    assert_eq!(
        items(&cats, "authors"),
        vec![
            p("Frank Herbert", 2),
            p("Erich Kästner", 1),
            p("Steve Klabnik", 1),
            p("Carol Nichols", 1),
            p("Nobody", 1)
        ]
    );
    assert_eq!(items(&cats, "series"), vec![p("Dune Chronicles", 2)]);
    assert_eq!(
        items(&cats, "tags"),
        vec![
            p("Classic", 1),
            p("Kids", 1),
            p("Programming", 1),
            p("Science Fiction", 2)
        ]
    );
    assert_eq!(
        items(&cats, "publisher"),
        vec![p("Chilton", 1), p("No Starch", 1)]
    );
    assert_eq!(
        items(&cats, "languages"),
        vec![p("English", 3), p("German", 1)]
    );
    assert_eq!(
        items(&cats, "formats"),
        vec![p("EPUB", 3), p("MOBI", 1), p("PDF", 2)]
    );
    assert_eq!(
        items(&cats, "rating"),
        vec![p("5", 1), p("3", 1), p("1", 1)]
    );
    assert_eq!(
        items(&cats, "identifiers"),
        vec![p("goodreads", 1), p("isbn", 1)]
    );

    let all: Vec<String> = cats
        .iter()
        .flat_map(|c| c.items.iter().map(|i| i.search.clone()))
        .collect();
    assert!(all.contains(&"tags:\"=Science Fiction\"".to_string()));
    assert!(all.contains(&"authors:\"=Frank Herbert\"".to_string()));
    assert!(all.contains(&"rating:5".to_string()));
    assert!(all.contains(&"formats:\"=EPUB\"".to_string()));
    assert!(all.contains(&"identifiers:\"=isbn:\"".to_string()));
    assert!(all.contains(&"languages:\"=eng\"".to_string()));
    for c in &cats {
        for item in &c.items {
            assert_eq!(
                list(&lib, &item.search).len() as u32,
                item.count,
                "{} {}",
                c.key,
                item.search
            );
        }
    }

    let restricted = lib.categories(Some("tags:\"=Science Fiction\"")).unwrap();
    assert_eq!(items(&restricted, "authors"), vec![p("Frank Herbert", 2)]);
    assert_eq!(
        items(&restricted, "tags"),
        vec![p("Classic", 1), p("Science Fiction", 2)]
    );
    assert_eq!(
        items(&restricted, "formats"),
        vec![p("EPUB", 2), p("MOBI", 1)]
    );
    for c in &restricted {
        for item in &c.items {
            let q = format!("({}) and {}", "tags:\"=Science Fiction\"", item.search);
            assert_eq!(list(&lib, &q).len() as u32, item.count, "{q}");
        }
    }
    assert!(lib.categories(Some("(")).is_err());
}

#[test]
fn category_search_quotes_awkward_names() {
    let (_tmp, lib) = new_library();
    let names = [
        "Say \"Hi\"",
        ".hidden",
        "back\\slash",
        "(paren)",
        "=eq",
        "~tilde",
        "a.b",
    ];
    let m = Metadata {
        tags: names.iter().map(|s| s.to_string()).collect(),
        ..meta("Quoted", &["Au Thor"])
    };
    lib.add_book(&m, None, &[]).unwrap();
    lib.add_book(&meta("Other", &["Au Thor"]), None, &[])
        .unwrap();
    let cats = lib.categories(None).unwrap();
    let tags = cats.iter().find(|c| c.key == "tags").unwrap();
    assert_eq!(tags.items.len(), names.len());
    for item in &tags.items {
        assert_eq!(ids(&lib, &item.search), vec![1], "{}", item.search);
    }
}

#[test]
fn trigger_driven_fields_and_functions() {
    let (_tmp, lib) = new_library();
    let id = lib
        .add_book(
            &meta("A Study in Scarlet", &["Arthur Conan Doyle"]),
            None,
            &[],
        )
        .unwrap();
    let conn = raw(&lib);
    let sort: String = conn
        .query_row("SELECT sort FROM books WHERE id=?1", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(sort, "Study in Scarlet, A");
    let asort: String = conn
        .query_row("SELECT sort FROM authors", [], |r| r.get(0))
        .unwrap();
    assert_eq!(asort, "Doyle, Arthur Conan");
    lib.update_book(
        id,
        &BookPatch {
            series: Some("The Series".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let ssort: String = conn
        .query_row("SELECT sort FROM series", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ssort, "Series, The");
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM books WHERE uuid IS NOT NULL AND length(uuid)=36"
        ),
        1
    );
}

#[test]
fn survives_calibre_style_concurrent_access() {
    let (_tmp, lib) = new_library();
    let lib = std::sync::Arc::new(lib);
    let mut handles = Vec::new();
    for t in 0..4 {
        let lib = lib.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..10 {
                lib.add_book(&meta(&format!("Book {t}-{i}"), &["Au Thor"]), None, &[])
                    .unwrap();
                lib.list_books(&ListQuery::default()).unwrap();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(lib.book_count().unwrap(), 40);
    let other = rusqlite::Connection::open(lib.dir().join("metadata.db")).unwrap();
    other.execute_batch("BEGIN IMMEDIATE").unwrap();
    other.execute_batch("ROLLBACK").unwrap();
    lib.add_book(&meta("After", &["Au Thor"]), None, &[])
        .unwrap();
}

#[test]
fn fixture_read_invariants() {
    let Some((_tmp, lib)) = fixture_copy() else {
        return;
    };
    let conn = raw(&lib);
    let n = count(&conn, "SELECT COUNT(*) FROM books") as u64;
    assert_eq!(lib.book_count().unwrap(), n);
    let all = lib.list_books(&ListQuery::default()).unwrap();
    assert_eq!(all.len() as u64, n);

    for b in &all {
        assert!(!b.title.is_empty() && !b.authors.is_empty());
        assert!(b.timestamp.contains('T'));
        if let Some(p) = &b.pubdate {
            assert!(!p.starts_with("0101-"));
        }
        assert!(b.rating.is_none_or(|r| (1..=10).contains(&r)));
        let full = lib.get_book(b.id).unwrap();
        assert_eq!(full.files.len(), b.formats.len());
        assert_eq!(full.uuid.len(), 36);
    }

    let cats = lib.categories(None).unwrap();
    for c in &cats {
        for item in &c.items {
            let got = lib
                .list_books(&ListQuery {
                    search: Some(item.search.clone()),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(got.len() as u32, item.count, "{}: {}", c.key, item.search);
        }
    }
    let with_series = list(&lib, "series:true").len() as i64;
    assert_eq!(
        with_series,
        count(&conn, "SELECT COUNT(DISTINCT book) FROM books_series_link")
    );
    let tagged = list(&lib, "tags:true").len() as i64;
    assert_eq!(
        tagged,
        count(&conn, "SELECT COUNT(DISTINCT book) FROM books_tags_link")
    );
    assert_eq!(list(&lib, "tags:false").len() as i64 + tagged, n as i64);
    let rated = list(&lib, "rating:true").len() as i64;
    assert_eq!(
        rated,
        count(&conn, "SELECT COUNT(DISTINCT book) FROM books_ratings_link")
    );
    assert_eq!(list(&lib, "id:>0").len() as u64, n);
    let has_pub = list(&lib, "pubdate:true").len() as i64;
    assert_eq!(
        has_pub,
        count(
            &conn,
            "SELECT COUNT(*) FROM books WHERE pubdate NOT LIKE '0101-%'"
        )
    );
    assert!(lib.saved_searches().unwrap().is_empty());
}

#[test]
fn fixture_write_invariants() {
    let Some((tmp, lib)) = fixture_copy() else {
        return;
    };
    let before = lib.book_count().unwrap();
    let first = lib
        .list_books(&ListQuery {
            sort_by: SortField::Title,
            ..Default::default()
        })
        .unwrap();
    let target = &first[0];
    let old_path = lib.get_book(target.id).unwrap().path;
    let patch = BookPatch {
        title: Some("A Brand New Title".into()),
        authors: Some(vec!["New Author".into()]),
        tags: Some(vec!["shelfmark-test".into()]),
        rating: Some(8),
        ..Default::default()
    };
    let b = lib.update_book(target.id, &patch).unwrap();
    assert_eq!(b.summary.sort, "Brand New Title, A");
    assert_eq!(
        b.path,
        format!("New Author/A Brand New Title ({})", target.id)
    );
    assert_ne!(b.path, old_path);
    assert!(
        !tmp.path().join("lib").join("New Author").exists(),
        "no folders fabricated for books that are not on disk"
    );
    assert_eq!(list(&lib, "tags:=shelfmark-test").len(), 1);

    let id = lib
        .add_book(&meta("Added To Real Library", &["Some Writer"]), None, &[])
        .unwrap();
    assert!(id > before as i64);
    lib.delete_books(&[id]).unwrap();
    assert_eq!(lib.book_count().unwrap(), before);

    lib.bulk_update(
        &first.iter().take(5).map(|b| b.id).collect::<Vec<_>>(),
        &BulkPatch {
            add_tags: vec!["bulk-test".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(list(&lib, "tags:=bulk-test").len(), 5);

    let conn = raw(&lib);
    let ic: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ic, "ok");
    let fk: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(fk, 0);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM books_authors_link WHERE author NOT IN (SELECT id FROM authors)"
        ),
        0
    );
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM authors WHERE id NOT IN (SELECT author FROM books_authors_link)"
        ),
        0
    );
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM tags WHERE id NOT IN (SELECT tag FROM books_tags_link)"
        ),
        0
    );
    assert!(count(&conn, "SELECT COUNT(*) FROM metadata_dirtied") >= 5);
    let last_mod: String = conn
        .query_row(
            "SELECT last_modified FROM books WHERE id=?1",
            [target.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(last_mod.ends_with("+00:00") && last_mod.starts_with("20"));
    assert_eq!(
        count(&conn, "PRAGMA user_version"),
        27,
        "schema version of the user's library is left alone"
    );
}

#[test]
fn delete_cascades_through_calibre_triggers_and_fts() {
    let (_tmp, lib) = new_library();
    let id = lib
        .add_book(&meta("With Notes", &["Au Thor"]), None, &[])
        .unwrap();
    let other = lib
        .add_book(&meta("Other", &["Au Thor"]), None, &[])
        .unwrap();
    let conn = raw(&lib);
    for b in [id, other] {
        conn.execute(
            "INSERT INTO annotations(book, format, user_type, user, timestamp, annot_id, annot_type, annot_data, searchable_text) VALUES (?1, 'EPUB', 'local', 'viewer', 1.0, 'a', 'highlight', '{}', 'findable words')",
            [b],
        )
        .unwrap();
        conn.execute("INSERT INTO book_storage(book, format, user_type, user, timestamp) VALUES (?1, 'EPUB', 'local', 'viewer', 1.0)", [b]).unwrap();
        conn.execute("INSERT INTO last_read_positions(book, format, user, device, cfi, epoch) VALUES (?1, 'EPUB', 'u', 'd', 'c', 1.0)", [b]).unwrap();
        conn.execute(
            "INSERT INTO conversion_options(format, book, data) VALUES ('EPUB', ?1, x'00')",
            [b],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO books_plugin_data(book, name, val) VALUES (?1, 'n', 'v')",
            [b],
        )
        .unwrap();
    }
    drop(conn);
    lib.delete_books(&[id]).unwrap();
    let conn = raw(&lib);
    for t in [
        "annotations",
        "book_storage",
        "last_read_positions",
        "conversion_options",
        "books_plugin_data",
        "books_pages_link",
    ] {
        assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM {t}")), 1, "{t}");
        assert_eq!(
            count(&conn, &format!("SELECT COUNT(*) FROM {t} WHERE book={id}")),
            0,
            "{t}"
        );
    }
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM annotations_fts WHERE annotations_fts MATCH 'findable'"
        ),
        1
    );
    let ic: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ic, "ok");
    conn.execute(
        "INSERT INTO annotations_fts(annotations_fts) VALUES('integrity-check')",
        [],
    )
    .unwrap();
}

#[test]
fn custom_columns_are_cleaned_on_delete() {
    let (_tmp, lib) = new_library();
    let id = lib.add_book(&meta("Cc", &["Au Thor"]), None, &[]).unwrap();
    let conn = raw(&lib);
    conn.execute_batch(
        "INSERT INTO custom_columns(label, name, datatype, normalized) VALUES ('plain', 'Plain', 'text', 0), ('norm', 'Norm', 'text', 1);
         CREATE TABLE custom_column_1(id INTEGER PRIMARY KEY, book INTEGER, value TEXT);
         CREATE TABLE custom_column_2(id INTEGER PRIMARY KEY, value TEXT);
         CREATE TABLE books_custom_column_2_link(id INTEGER PRIMARY KEY, book INTEGER, value INTEGER);
         INSERT INTO custom_column_1(book, value) VALUES (1, 'x');
         INSERT INTO custom_column_2(value) VALUES ('y');
         INSERT INTO books_custom_column_2_link(book, value) VALUES (1, 1);",
    )
    .unwrap();
    drop(conn);
    lib.delete_books(&[id]).unwrap();
    let conn = raw(&lib);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM custom_column_1"), 0);
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM books_custom_column_2_link"),
        0
    );
}

#[test]
fn too_old_schema_is_rejected() {
    let (tmp, lib) = new_library();
    let dir = lib.dir().to_path_buf();
    raw(&lib).execute_batch("PRAGMA user_version=20").unwrap();
    drop(lib);
    assert!(matches!(
        Library::open(&dir),
        Err(Error::UnsupportedSchema(20))
    ));
    let _ = tmp;
}

#[test]
fn changes_made_by_another_process_are_seen() {
    let (_tmp, lib) = new_library();
    lib.add_book(&meta("First", &["Au Thor"]), None, &[])
        .unwrap();
    assert_eq!(list(&lib, "").len(), 1);
    let conn = raw(&lib);
    conn.create_scalar_function(
        "uuid4",
        0,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        |_| Ok("u".to_string()),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO books(title, author_sort) VALUES ('Added by calibre', 'x')",
        [],
    )
    .unwrap();
    assert_eq!(list(&lib, "").len(), 2);
    assert_eq!(lib.book_count().unwrap(), 2);
    conn.execute("UPDATE books SET title='Renamed elsewhere' WHERE id=1", [])
        .unwrap();
    assert_eq!(list(&lib, "elsewhere").len(), 1);
}

#[test]
fn fixture_books_with_files_are_renamed_like_calibre() {
    let Some((tmp, lib)) = fixture_copy() else {
        return;
    };
    let root = tmp.path().join("lib");
    let books = lib.list_books(&ListQuery::default()).unwrap();
    let picked: Vec<_> = books
        .iter()
        .filter(|b| !b.formats.is_empty())
        .take(3)
        .collect();
    assert_eq!(picked.len(), 3);
    for b in &picked {
        let full = lib.get_book(b.id).unwrap();
        let dir = root.join(&full.path);
        std::fs::create_dir_all(&dir).unwrap();
        for f in &full.files {
            std::fs::write(dir.join(&f.file_name), b"x").unwrap();
        }
        std::fs::write(dir.join("cover.jpg"), b"jpg").unwrap();
        std::fs::write(dir.join("metadata.opf"), b"opf").unwrap();
    }
    let ids: Vec<i64> = picked.iter().map(|b| b.id).collect();
    let before: Vec<_> = ids.iter().map(|&i| lib.get_book(i).unwrap()).collect();
    let updated = lib
        .update_book(
            ids[0],
            &BookPatch {
                title: Some("Completely Different Title".into()),
                authors: Some(vec!["Zora Test".into()]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        updated.path,
        format!("Zora Test/Completely Different Title ({})", ids[0])
    );
    let dir = root.join(&updated.path);
    for f in &updated.files {
        assert_eq!(
            f.file_name,
            format!(
                "Completely Different Title - Zora Test.{}",
                f.format.to_lowercase()
            )
        );
        assert!(dir.join(&f.file_name).is_file(), "{}", f.file_name);
    }
    assert!(dir.join("cover.jpg").is_file() && dir.join("metadata.opf").is_file());
    assert!(!root.join(&before[0].path).exists());

    lib.delete_books(&ids[1..]).unwrap();
    for b in &before[1..] {
        assert!(!root.join(&b.path).exists());
        assert!(
            root.join(".caltrash/b")
                .join(b.summary.id.to_string())
                .join("cover.jpg")
                .is_file()
        );
    }
    assert_eq!(lib.book_count().unwrap(), books.len() as u64 - 2);
}
