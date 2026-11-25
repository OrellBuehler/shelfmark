mod common;

use std::sync::atomic::{AtomicBool, Ordering};

use common::Dir;
use common::audio_gen::{M4b, jpeg};
use shelfmark_audio::transfer::{Dest, Mode, Verify, plan, run};
use shelfmark_audio::{AudioStore, LoftyReader, NewRoot, scan};
use shelfmark_model::{AudioLayout, AudioState, LinkState};

struct Fixture {
    lib: Dir,
    ext: Dir,
    store: AudioStore,
    ext_root: i64,
    lib_root: i64,
}

fn book(title: &str, asin: &str) -> Vec<u8> {
    let mut bytes = M4b::new(60_000)
        .text(b"\xa9nam", title)
        .text(b"\xa9ART", "Frank Herbert")
        .text(b"asin", asin)
        .build();
    bytes.extend(std::iter::repeat_n(7u8, 3 * 1024 * 1024));
    bytes
}

fn fixture() -> Fixture {
    fixture_with(Dir::new())
}

fn fixture_with(ext: Dir) -> Fixture {
    let lib = Dir::new();
    let store = AudioStore::open(lib.path()).unwrap();
    let ext_root = store
        .add_root(&NewRoot {
            path: ext.path().to_string_lossy().into_owned(),
            ..Default::default()
        })
        .unwrap()
        .id;
    std::fs::create_dir_all(lib.path().join("Audiobooks")).unwrap();
    let lib_root = store
        .add_root(&NewRoot {
            path: "Audiobooks".into(),
            in_library: true,
            managed: true,
            layout: AudioLayout::AuthorTitle,
            label: None,
        })
        .unwrap()
        .id;
    ext.old("Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b", &book("Dune", "B002V1A0WE"));
    ext.old("Dune [B002V1A0WE]/Dune [B002V1A0WE].jpg", &jpeg(20, 20));
    ext.old("Emma [B00AAAAAA1]/Emma [B00AAAAAA1].m4b", &book("Emma", "B00AAAAAA1"));
    let f = Fixture {
        lib,
        ext,
        store,
        ext_root,
        lib_root,
    };
    rescan(&f, f.ext_root);
    f
}

fn rescan(f: &Fixture, root: i64) -> shelfmark_audio::ScanSummary {
    scan(&f.store, root, &LoftyReader, &AtomicBool::new(false), &|_| {}).unwrap()
}

fn id_of(f: &Fixture, title: &str) -> i64 {
    f.store.list().unwrap().iter().find(|r| r.title == title).unwrap().id
}

#[test]
fn move_into_library_keeps_identity_and_removes_sources() {
    let f = fixture();
    let dune = id_of(&f, "Dune");
    f.store.set_link(dune, 5, None, LinkState::Manual).unwrap();
    let p = plan(&f.store, &[dune], &Dest::Root(f.lib_root)).unwrap();
    assert_eq!(p.items[0].dest_dir, "Frank Herbert/Dune [B002V1A0WE]");
    assert!(p.items[0].skip.is_none() && p.enough_space);
    let s = run(&f.store, &p, Mode::Move, Verify::Full, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!((s.done, s.failed.len(), s.skipped.len()), (1, 0, 0));

    let dest = f.lib.path().join("Audiobooks/Frank Herbert/Dune [B002V1A0WE]");
    assert!(dest.join("Dune [B002V1A0WE].m4b").is_file());
    assert!(dest.join("Dune [B002V1A0WE].jpg").is_file());
    assert!(!f.ext.path().join("Dune [B002V1A0WE]").exists());
    assert!(f.ext.path().join("Emma [B00AAAAAA1]").exists());
    let row = f.store.list().unwrap().iter().find(|r| r.id == dune).cloned().unwrap();
    assert_eq!(row.root_id, Some(f.lib_root));
    assert_eq!(f.store.all_links().unwrap()[0].audiobook_id, dune);

    let s = rescan(&f, f.lib_root);
    assert_eq!((s.unchanged, s.added, s.updated), (1, 0, 0));
    let s = rescan(&f, f.ext_root);
    assert_eq!((s.audiobooks, s.missing), (1, 0));
    assert_eq!(f.store.list().unwrap().len(), 2);
    assert!(f.store.list().unwrap().iter().all(|r| r.state == AudioState::Present));
}

#[test]
fn copy_leaves_sources_and_index_alone() {
    let f = fixture();
    let dune = id_of(&f, "Dune");
    let p = plan(&f.store, &[dune], &Dest::Root(f.lib_root)).unwrap();
    let s = run(&f.store, &p, Mode::Copy, Verify::Quick, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(s.done, 1);
    assert!(f.ext.path().join("Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b").is_file());
    assert_eq!(f.store.detail(dune).unwrap().row.root_id, Some(f.ext_root));
    let src = std::fs::read(f.ext.path().join("Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b")).unwrap();
    let dst = std::fs::read(
        f.lib.path().join("Audiobooks/Frank Herbert/Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b"),
    )
    .unwrap();
    assert_eq!(src, dst);

    let again = plan(&f.store, &[dune], &Dest::Root(f.lib_root)).unwrap();
    assert!(again.items[0].skip.as_deref().unwrap().contains("already exists"));
    let s = run(&f.store, &again, Mode::Copy, Verify::Quick, &AtomicBool::new(false), &|_| {})
        .unwrap();
    assert_eq!((s.done, s.skipped.len()), (0, 1));
}

#[test]
fn export_to_a_plain_folder_copies_only() {
    let f = fixture();
    let out = Dir::new();
    let ids = [id_of(&f, "Dune"), id_of(&f, "Emma")];
    let p = plan(&f.store, &ids, &Dest::Folder(out.path().join("Audiobooks"))).unwrap();
    assert!(run(&f.store, &p, Mode::Move, Verify::Quick, &AtomicBool::new(false), &|_| {}).is_err());
    let s = run(&f.store, &p, Mode::Copy, Verify::Full, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(s.done, 2);
    assert!(out.path().join("Audiobooks/Frank Herbert/Emma [B00AAAAAA1]/Emma [B00AAAAAA1].m4b").is_file());
    assert_eq!(f.store.list().unwrap().len(), 2);
}

#[test]
fn cancel_mid_copy_leaves_no_partial_folder() {
    let f = fixture();
    let ids = [id_of(&f, "Dune"), id_of(&f, "Emma")];
    let p = plan(&f.store, &ids, &Dest::Root(f.lib_root)).unwrap();
    let cancel = AtomicBool::new(false);
    let s = run(&f.store, &p, Mode::Copy, Verify::Full, &cancel, &|pr| {
        if pr.item == 2 {
            cancel.store(true, Ordering::SeqCst);
        }
    })
    .unwrap();
    assert!(s.cancelled);
    assert_eq!(s.done, 1);
    let author = f.lib.path().join("Audiobooks/Frank Herbert");
    let names: Vec<String> = std::fs::read_dir(&author)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["Dune [B002V1A0WE]"]);
    assert!(f.ext.path().join("Emma [B00AAAAAA1]/Emma [B00AAAAAA1].m4b").is_file());
}

#[test]
fn unmanaged_destinations_and_unavailable_books_are_refused() {
    let f = fixture();
    let dune = id_of(&f, "Dune");
    assert!(plan(&f.store, &[dune], &Dest::Root(f.ext_root)).is_err());

    std::fs::remove_dir_all(f.ext.path().join("Dune [B002V1A0WE]")).unwrap();
    rescan(&f, f.ext_root);
    let p = plan(&f.store, &[dune], &Dest::Root(f.lib_root)).unwrap();
    assert!(p.items[0].skip.as_deref().unwrap().contains("not available"));
    assert_eq!(p.total_bytes, 0);
}

#[test]
fn a_source_that_vanishes_fails_that_item_only() {
    let f = fixture();
    let ids = [id_of(&f, "Dune"), id_of(&f, "Emma")];
    let p = plan(&f.store, &ids, &Dest::Root(f.lib_root)).unwrap();
    std::fs::remove_file(f.ext.path().join("Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b")).unwrap();
    let s = run(&f.store, &p, Mode::Move, Verify::Full, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!((s.done, s.failed.len()), (1, 1));
    assert_eq!(s.failed[0].title, "Dune");
    assert!(f.ext.path().join("Dune [B002V1A0WE]/Dune [B002V1A0WE].jpg").is_file());
    assert!(!f.lib.path().join("Audiobooks/Frank Herbert/Dune [B002V1A0WE]").exists());
    assert_eq!(f.store.detail(ids[0]).unwrap().row.root_id, Some(f.ext_root));
    assert_eq!(f.store.detail(ids[1]).unwrap().row.root_id, Some(f.lib_root));
}

#[test]
fn move_across_filesystems_copies_verifies_then_deletes() {
    let shm = std::path::Path::new("/dev/shm");
    if !shm.is_dir() {
        eprintln!("no second filesystem available, skipping");
        return;
    }
    let f = fixture_with(Dir::under(shm));
    let dune = id_of(&f, "Dune");
    let p = plan(&f.store, &[dune], &Dest::Root(f.lib_root)).unwrap();
    let s = run(&f.store, &p, Mode::Move, Verify::Full, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!((s.done, s.failed.len()), (1, 0));
    assert!(!f.ext.path().join("Dune [B002V1A0WE]").exists());
    assert!(
        f.lib
            .path()
            .join("Audiobooks/Frank Herbert/Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b")
            .is_file()
    );
    let s = rescan(&f, f.lib_root);
    assert_eq!((s.unchanged, s.updated), (1, 0));
}
