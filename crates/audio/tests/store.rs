mod common;

use common::Dir;
use shelfmark_audio::{AudioStore, Error, Issue, NewRoot, RootPatch, ScannedBook, ScannedFile};
use shelfmark_model::{AudioIndex, AudioLayout, AudioState, LinkState, ReadingState};
use shelfmark_reading::ReadingStore;

fn root(store: &AudioStore, path: &str) -> i64 {
    store
        .add_root(&NewRoot {
            path: path.into(),
            ..Default::default()
        })
        .unwrap()
        .id
}

fn book(key: &str, title: &str, asin: Option<&str>) -> ScannedBook {
    ScannedBook {
        key: key.into(),
        title: title.into(),
        authors: vec!["Ann Author".into()],
        narrators: vec!["Ned Narrator".into()],
        asin: asin.map(str::to_string),
        duration_ms: 3_600_000,
        format: "M4B".into(),
        meta_source: "tags".into(),
        added: "2026-01-01T00:00:00Z".into(),
        modified: "2026-01-01T00:00:00Z".into(),
        files: vec![
            ScannedFile {
                rel_path: format!("{key}.m4b"),
                kind: "audio".into(),
                size: 100,
                mtime: 5,
                ..Default::default()
            },
            ScannedFile {
                rel_path: format!("{key}.jpg"),
                kind: "cover".into(),
                size: 7,
                mtime: 5,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

#[test]
fn opens_reopens_and_coexists_with_reading_store() {
    let dir = Dir::new();
    let reading = ReadingStore::open(dir.path()).unwrap();
    let audio = AudioStore::open(dir.path()).unwrap();
    let r = root(&audio, "/books");
    audio
        .upsert_books(r, 1, &[book("a/a", "A", Some("B000000001"))])
        .unwrap();
    reading
        .set_state(&ReadingState {
            book_id: 1,
            format: "EPUB".into(),
            progress: 0.5,
            ..Default::default()
        })
        .unwrap();
    drop(audio);
    drop(reading);
    let reading = ReadingStore::open(dir.path()).unwrap();
    let audio = AudioStore::open(dir.path()).unwrap();
    assert_eq!(reading.state(1).unwrap().unwrap().progress, 0.5);
    assert_eq!(audio.list().unwrap().len(), 1);
}

#[test]
fn concurrent_writers_do_not_fail() {
    let dir = Dir::new();
    let reading = std::sync::Arc::new(ReadingStore::open(dir.path()).unwrap());
    let audio = std::sync::Arc::new(AudioStore::open(dir.path()).unwrap());
    let r = root(&audio, "/books");
    let a = {
        let audio = audio.clone();
        std::thread::spawn(move || {
            for i in 0..40 {
                let key = format!("k{i}");
                audio.upsert_books(r, 1, &[book(&key, "T", None)]).unwrap();
            }
        })
    };
    let b = {
        let reading = reading.clone();
        std::thread::spawn(move || {
            for i in 0..40 {
                reading
                    .set_state(&ReadingState {
                        book_id: i,
                        format: "EPUB".into(),
                        progress: 0.1,
                        ..Default::default()
                    })
                    .unwrap();
            }
        })
    };
    a.join().unwrap();
    b.join().unwrap();
    assert_eq!(audio.list().unwrap().len(), 40);
    assert_eq!(reading.progress().unwrap().len(), 40);
}

#[test]
fn rejects_newer_schema() {
    let dir = Dir::new();
    drop(AudioStore::open(dir.path()).unwrap());
    {
        let c = rusqlite::Connection::open(dir.path().join("shelfmark.db")).unwrap();
        c.execute("UPDATE audio_meta SET value = '99' WHERE key = 'schema'", [])
            .unwrap();
    }
    assert!(matches!(
        AudioStore::open(dir.path()),
        Err(Error::SchemaVersion(99))
    ));
    assert!(ReadingStore::open(dir.path()).is_ok());
}

#[test]
fn roots_crud_and_resolution() {
    let dir = Dir::new();
    let store = AudioStore::open(dir.path()).unwrap();
    let ext = store
        .add_root(&NewRoot {
            path: "/mnt/audio".into(),
            ..Default::default()
        })
        .unwrap();
    let inside = store
        .add_root(&NewRoot {
            path: "Audiobooks".into(),
            in_library: true,
            managed: true,
            layout: AudioLayout::AuthorTitle,
            label: None,
        })
        .unwrap();
    assert_eq!(store.root_dir(&ext), std::path::PathBuf::from("/mnt/audio"));
    assert_eq!(store.root_dir(&inside), dir.path().join("Audiobooks"));
    assert!(inside.managed && inside.enabled);
    assert_eq!(inside.layout, AudioLayout::AuthorTitle);
    assert!(
        store
            .add_root(&NewRoot {
                path: "/mnt/audio".into(),
                ..Default::default()
            })
            .is_err()
    );
    assert!(store.add_root(&NewRoot::default()).is_err());

    let patched = store
        .update_root(
            ext.id,
            &RootPatch {
                path: Some("/mnt/other".into()),
                managed: Some(true),
                label: Some(Some("NAS".into())),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(patched.path, "/mnt/other");
    assert!(patched.managed);
    assert_eq!(patched.label.as_deref(), Some("NAS"));
    assert_eq!(store.roots().unwrap().len(), 2);
}

#[test]
fn upsert_is_stable_and_keeps_added() {
    let dir = Dir::new();
    let store = AudioStore::open(dir.path()).unwrap();
    let r = root(&store, "/books");
    let first = store
        .upsert_books(r, 1, &[book("a/a", "A", Some("B000000001"))])
        .unwrap();
    let mut changed = book("a/a", "A2", Some("B000000001"));
    changed.added = "2030-01-01T00:00:00Z".into();
    let second = store.upsert_books(r, 2, &[changed]).unwrap();
    assert_eq!(first, second);
    let rows = store.list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "A2");
    assert_eq!(rows[0].added, "2026-01-01T00:00:00Z");
    assert_eq!(rows[0].size, 100);
    assert_eq!(rows[0].file_count, 1);
    assert_eq!(rows[0].narrators, ["Ned Narrator"]);
    assert_eq!(store.indexed_files(r).unwrap().len(), 2);
    assert_eq!(store.root(r).unwrap().audiobooks, 1);
}

#[test]
fn missing_rows_are_kept_and_revived() {
    let dir = Dir::new();
    let store = AudioStore::open(dir.path()).unwrap();
    let r = root(&store, "/books");
    let ids = store
        .upsert_books(
            r,
            1,
            &[book("a", "A", None), book("b", "B", None)],
        )
        .unwrap();
    store.set_link(ids[1], 9, Some("uuid"), LinkState::Manual).unwrap();
    store.touch_books(2, &ids[..1]).unwrap();
    assert_eq!(store.finish_scan(r, 2, false).unwrap(), 0);
    assert!(store.list().unwrap().iter().all(|b| b.state == AudioState::Present));
    assert_eq!(store.root(r).unwrap().last_scan_ok, Some(false));

    assert_eq!(store.finish_scan(r, 2, true).unwrap(), 1);
    let rows = store.list().unwrap();
    assert_eq!(rows[1].state, AudioState::Missing);
    assert_eq!(store.all_links().unwrap().len(), 1);

    store.touch_books(3, &ids).unwrap();
    assert_eq!(store.finish_scan(r, 3, true).unwrap(), 0);
    assert!(store.list().unwrap().iter().all(|b| b.state == AudioState::Present));
}

#[test]
fn links_and_cascades() {
    let dir = Dir::new();
    let store = AudioStore::open(dir.path()).unwrap();
    let r = root(&store, "/books");
    let ids = store
        .upsert_books(r, 1, &[book("a", "A", None), book("b", "B", None)])
        .unwrap();
    store.set_link(ids[0], 1, None, LinkState::Manual).unwrap();
    store.set_link(ids[0], 1, None, LinkState::Rejected).unwrap();
    store.set_link(ids[1], 2, None, LinkState::Manual).unwrap();
    let links = store.all_links().unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].state, LinkState::Rejected);
    store.delete_book_links(2).unwrap();
    assert_eq!(store.all_links().unwrap().len(), 1);
    store.set_cover(ids[0], b"jpg", "embedded", 3).unwrap();
    assert_eq!(store.cover(ids[0]).unwrap().unwrap(), b"jpg");
    assert!(store.list().unwrap()[0].has_cover);
    store
        .set_issues(
            r,
            &[Issue {
                path: "x.m4b".into(),
                message: "bad".into(),
            }],
        )
        .unwrap();
    assert_eq!(store.issues().unwrap().len(), 1);

    store.remove_root(r).unwrap();
    assert!(store.list().unwrap().is_empty());
    assert!(store.all_links().unwrap().is_empty());
    assert!(store.cover(ids[0]).unwrap().is_none());
    assert!(store.issues().unwrap().is_empty());
    assert!(store.indexed_files(r).unwrap().is_empty());
}

#[test]
fn disabled_roots_are_hidden_and_generation_moves() {
    let dir = Dir::new();
    let store = AudioStore::open(dir.path()).unwrap();
    let r = root(&store, "/books");
    let g0 = store.generation();
    store.upsert_books(r, 1, &[book("a", "A", None)]).unwrap();
    let g1 = store.generation();
    assert!(g1 > g0);
    assert!(std::sync::Arc::ptr_eq(
        &store.audiobooks().unwrap(),
        &store.audiobooks().unwrap()
    ));
    store
        .update_root(
            r,
            &RootPatch {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn detail_lists_files_and_dir() {
    let dir = Dir::new();
    let store = AudioStore::open(dir.path()).unwrap();
    let r = root(&store, "/books");
    let ids = store
        .upsert_books(r, 1, &[book("Title [B000000001]/Title [B000000001]", "T", None)])
        .unwrap();
    let d = store.detail(ids[0]).unwrap();
    assert_eq!(d.dir.as_deref(), Some("Title [B000000001]"));
    assert_eq!(d.files.len(), 2);
    assert_eq!(d.files[0].kind, "audio");
    assert!(store.detail(999).is_err());
}

#[test]
fn store_is_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<AudioStore>();
}

#[test]
fn remote_titles_sync_and_hide_behind_downloaded_copies() {
    use shelfmark_audio::RemoteBook;
    let dir = Dir::new();
    let store = AudioStore::open(dir.path()).unwrap();
    let r = root(&store, "/books");
    store
        .upsert_books(r, 1, &[book("a", "Dune", Some("B000000001"))])
        .unwrap();
    let remote = |asin: &str, title: &str| RemoteBook {
        asin: asin.into(),
        title: title.into(),
        authors: vec!["Ann Author".into()],
        duration_ms: 60_000,
        added: Some("2020-01-01T00:00:00Z".into()),
        ..Default::default()
    };
    let s = store
        .sync_remote("acc", &[remote("B000000001", "Dune"), remote("B000000002", "Emma")])
        .unwrap();
    assert_eq!((s.total, s.added, s.removed, s.without_cover.len()), (2, 2, 0, 2));
    assert_eq!(store.remote_count().unwrap(), 2);
    let rows = store.list().unwrap();
    assert_eq!(rows.len(), 2);
    let emma = rows.iter().find(|b| b.title == "Emma").unwrap();
    assert_eq!((emma.state, emma.root_id), (AudioState::Remote, None));
    assert_eq!(emma.added, "2020-01-01T00:00:00Z");
    store.set_link(emma.id, 4, None, LinkState::Manual).unwrap();
    store.set_cover(emma.id, b"jpg", "audible", 0).unwrap();

    let s = store
        .sync_remote("acc", &[remote("B000000002", "Emma (new title)"), remote("B000000003", "Zed")])
        .unwrap();
    assert_eq!((s.added, s.removed, s.without_cover.len()), (1, 1, 1));
    let rows = store.list().unwrap();
    assert!(rows.iter().any(|b| b.title == "Emma (new title)" && b.id == emma.id && b.has_cover));
    assert_eq!(store.all_links().unwrap().len(), 1);

    store.sync_remote("other", &[remote("B000000009", "Other")]).unwrap();
    assert_eq!(store.remote_count().unwrap(), 3);
    store.sync_remote("acc", &[]).unwrap();
    assert_eq!(store.remote_count().unwrap(), 1);

    store.finish_scan(r, 5, true).unwrap();
    store.sync_remote("acc", &[remote("B000000001", "Dune")]).unwrap();
    assert!(store.list().unwrap().iter().any(|b| b.title == "Dune" && b.state == AudioState::Remote));
    assert_eq!(store.clear_remote().unwrap(), 2);
    assert_eq!(store.indexed_books(r).unwrap().len(), 1);
}
