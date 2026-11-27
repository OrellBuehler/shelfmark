mod common;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use common::audio_gen::{M4b, Mp3, jpeg};
use common::{Dir, age};
use shelfmark_audio::{
    AudioStore, Error, FileTags, LoftyReader, NewRoot, ScanSummary, TagReader, scan,
};
use shelfmark_model::{AudioRoot, AudioState, LinkState};

struct Counting(AtomicUsize);

impl TagReader for Counting {
    fn read(&self, path: &Path, want_cover: bool) -> shelfmark_audio::Result<FileTags> {
        self.0.fetch_add(1, Ordering::SeqCst);
        LoftyReader.read(path, want_cover)
    }
}

struct Fixture {
    lib: Dir,
    audio: Dir,
    store: AudioStore,
    root: AudioRoot,
}

fn fixture() -> Fixture {
    let lib = Dir::new();
    let audio = Dir::new();
    let store = AudioStore::open(lib.path()).unwrap();
    let root = store
        .add_root(&NewRoot {
            path: audio.path().to_string_lossy().into_owned(),
            ..Default::default()
        })
        .unwrap();
    Fixture {
        lib,
        audio,
        store,
        root,
    }
}

fn run(f: &Fixture) -> ScanSummary {
    scan(&f.store, f.root.id, &LoftyReader, &AtomicBool::new(false), &|_| {}).unwrap()
}

fn dune() -> Vec<u8> {
    M4b::new(60_000)
        .text(b"\xa9nam", "Dune")
        .text(b"\xa9ART", "Frank Herbert")
        .text(b"\xa9wrt", "Scott Brick")
        .text(b"asin", "B002V1A0WE")
        .cover(jpeg(900, 900))
        .build()
}

#[test]
fn libation_default_layout() {
    let f = fixture();
    f.audio.old("Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b", &dune());
    f.audio.old("Dune [B002V1A0WE]/Dune [B002V1A0WE].pdf", b"%PDF-1.4");
    f.audio.old(
        "Dune [B002V1A0WE]/Dune [B002V1A0WE].metadata.json",
        br#"{"asin":"B002V1A0WE","series":[{"title":"Dune Chronicles","sequence":"1"}],
            "ChapterInfo":{"chapters":[{"title":"One","start_offset_ms":0}]}}"#,
    );
    f.audio.old("Untagged [B00AAAAAA1]/Untagged [B00AAAAAA1].m4b", &M4b::new(1000).build());
    f.audio.old("Untagged [B00AAAAAA1]/Untagged [B00AAAAAA1].jpg", &jpeg(30, 30));
    f.audio.old(".hidden/x.m4b", &dune());
    f.audio.old("@eaDir/x.m4b", &dune());
    f.audio.old("Dune [B002V1A0WE]/Dune [B002V1A0WE].aaxc", b"x");

    let s = run(&f);
    assert_eq!((s.audiobooks, s.added, s.missing, s.issues), (2, 2, 0, 0));
    assert!(s.complete);
    let rows = f.store.list().unwrap();
    let dune = rows.iter().find(|r| r.title == "Dune").unwrap();
    assert_eq!(dune.asin.as_deref(), Some("B002V1A0WE"));
    assert_eq!(dune.authors, ["Frank Herbert"]);
    assert_eq!(dune.narrators, ["Scott Brick"]);
    assert_eq!(dune.series.as_deref(), Some("Dune Chronicles"));
    assert_eq!(dune.duration_ms, 60_000);
    assert_eq!(dune.format, "M4B");
    assert!(dune.has_cover);
    let thumb = image::load_from_memory(&f.store.cover(dune.id).unwrap().unwrap()).unwrap();
    assert_eq!(thumb.width(), 500);
    let detail = f.store.detail(dune.id).unwrap();
    assert_eq!(detail.files.len(), 3);
    assert_eq!(detail.chapters.len(), 1);
    assert_eq!(detail.dir.as_deref(), Some("Dune [B002V1A0WE]"));

    let bare = rows.iter().find(|r| r.title == "Untagged").unwrap();
    assert_eq!(bare.asin.as_deref(), Some("B00AAAAAA1"));
    assert!(bare.has_cover);
    assert!(bare.authors.is_empty());
}

#[test]
fn chapter_split_folder_is_one_audiobook() {
    let f = fixture();
    for i in 1..=3 {
        let bytes = M4b::new(10_000)
            .text(b"\xa9nam", &format!("Chapter {i}"))
            .text(b"\xa9alb", "Dune")
            .text(b"\xa9ART", "Frank Herbert")
            .text(b"asin", "B002V1A0WE")
            .track(i)
            .build();
        f.audio.old(
            &format!("Dune [B002V1A0WE]/Dune [B002V1A0WE] - 0{i} - Chapter {i}.m4b"),
            &bytes,
        );
    }
    f.audio.old("Dune [B002V1A0WE]/Dune [B002V1A0WE].jpg", &jpeg(20, 20));
    let s = run(&f);
    assert_eq!(s.audiobooks, 1);
    let rows = f.store.list().unwrap();
    assert_eq!(rows[0].title, "Dune");
    assert_eq!(rows[0].file_count, 3);
    assert_eq!(rows[0].duration_ms, 30_000);
    let d = f.store.detail(rows[0].id).unwrap();
    assert_eq!(d.dir.as_deref(), Some("Dune [B002V1A0WE]"));
    assert_eq!(d.files[0].track, Some(1));
    assert_eq!(d.files[1].title.as_deref(), Some("Chapter 2"));
}

#[test]
fn author_folders_with_several_books_and_mp3_albums() {
    let f = fixture();
    f.audio.old("Frank Herbert/Dune.m4b", &M4b::new(1000).build());
    f.audio.old("Frank Herbert/Dune Messiah.m4b", &M4b::new(1000).build());
    for i in 1..=2 {
        let bytes = Mp3::new(5)
            .text(b"TALB", "The Hobbit")
            .text(b"TPE1", "J.R.R. Tolkien")
            .text(b"TIT2", &format!("Part {i}"))
            .build();
        f.audio.old(&format!("Tolkien/The Hobbit/0{i} - Part {i}.mp3"), &bytes);
    }
    f.audio.old("Converted/Book.m4b", &M4b::new(1000).text(b"\xa9nam", "Book").build());
    f.audio.old("Converted/Book.mp3", &Mp3::new(5).build());
    let s = run(&f);
    assert_eq!(s.audiobooks, 4);
    let rows = f.store.list().unwrap();
    let mut titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
    titles.sort();
    assert_eq!(titles, ["Book", "Dune", "Dune Messiah", "The Hobbit"]);
    let hobbit = rows.iter().find(|r| r.title == "The Hobbit").unwrap();
    assert_eq!(hobbit.format, "MP3");
    assert_eq!(hobbit.file_count, 2);
    let book = rows.iter().find(|r| r.title == "Book").unwrap();
    assert_eq!((book.format.as_str(), book.file_count), ("M4B", 1));
}

#[test]
fn rescan_is_incremental_and_picks_up_changes() {
    let f = fixture();
    let a = f.audio.old("A [B00AAAAAA1]/A [B00AAAAAA1].m4b", &dune());
    f.audio.old("B [B00AAAAAA2]/B [B00AAAAAA2].m4b", &dune());
    let reader = Counting(AtomicUsize::new(0));
    let cancel = AtomicBool::new(false);
    scan(&f.store, f.root.id, &reader, &cancel, &|_| {}).unwrap();
    assert_eq!(reader.0.swap(0, Ordering::SeqCst), 2);
    let ids: Vec<_> = f.store.list().unwrap().iter().map(|r| r.id).collect();

    let s = scan(&f.store, f.root.id, &reader, &cancel, &|_| {}).unwrap();
    assert_eq!(reader.0.swap(0, Ordering::SeqCst), 0);
    assert_eq!((s.unchanged, s.added, s.updated, s.missing), (2, 0, 0, 0));

    std::fs::write(&a, M4b::new(5000).text(b"\xa9nam", "Renamed").build()).unwrap();
    age(&a, 600);
    let s = scan(&f.store, f.root.id, &reader, &cancel, &|_| {}).unwrap();
    assert_eq!(reader.0.swap(0, Ordering::SeqCst), 1);
    assert_eq!((s.unchanged, s.added, s.updated), (1, 0, 1));
    let rows = f.store.list().unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), ids);
    assert!(rows.iter().any(|r| r.title == "Renamed"));
}

#[test]
fn moved_book_keeps_its_id_and_links() {
    let f = fixture();
    f.audio.old("Dune [B002V1A0WE]/Dune [B002V1A0WE].m4b", &dune());
    run(&f);
    let id = f.store.list().unwrap()[0].id;
    f.store.set_link(id, 7, None, LinkState::Manual).unwrap();
    std::fs::create_dir_all(f.audio.path().join("Frank Herbert")).unwrap();
    std::fs::rename(
        f.audio.path().join("Dune [B002V1A0WE]"),
        f.audio.path().join("Frank Herbert/Dune [B002V1A0WE]"),
    )
    .unwrap();
    let s = run(&f);
    assert_eq!((s.added, s.updated, s.missing), (0, 1, 0));
    let rows = f.store.list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert_eq!(f.store.all_links().unwrap()[0].audiobook_id, id);
    assert_eq!(
        f.store.detail(id).unwrap().dir.as_deref(),
        Some("Frank Herbert/Dune [B002V1A0WE]")
    );
}

#[test]
fn deleted_files_become_missing_but_an_empty_or_dead_root_changes_nothing() {
    let f = fixture();
    f.audio.old("A/A.m4b", &dune());
    let b = f.audio.old("B/B.m4b", &M4b::new(1).text(b"\xa9nam", "B").build());
    run(&f);
    std::fs::remove_file(&b).unwrap();
    let s = run(&f);
    assert_eq!(s.missing, 1);
    let rows = f.store.list().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.iter().filter(|r| r.state == AudioState::Missing).count(), 1);

    std::fs::remove_dir_all(f.audio.path().join("A")).unwrap();
    let s = run(&f);
    assert!(!s.complete);
    assert_eq!(s.missing, 0);
    assert_eq!(
        f.store.list().unwrap().iter().filter(|r| r.state == AudioState::Present).count(),
        1
    );

    std::fs::remove_dir_all(f.audio.path()).unwrap();
    let err = scan(&f.store, f.root.id, &LoftyReader, &AtomicBool::new(false), &|_| {});
    assert!(matches!(err, Err(Error::Invalid(_))));
    assert_eq!(f.store.root(f.root.id).unwrap().last_scan_ok, Some(false));
    assert_eq!(f.store.list().unwrap().len(), 2);
}

#[test]
fn unreadable_audio_is_reported_and_still_indexed_by_name() {
    let f = fixture();
    f.audio.old("Broken [B00AAAAAA1]/Broken [B00AAAAAA1].m4b", b"garbage");
    let s = run(&f);
    assert_eq!((s.audiobooks, s.issues), (1, 1));
    let rows = f.store.list().unwrap();
    assert_eq!(rows[0].title, "Broken");
    assert_eq!(rows[0].asin.as_deref(), Some("B00AAAAAA1"));
    assert_eq!(f.store.issues().unwrap().len(), 1);
}

#[test]
fn fresh_files_are_skipped_until_they_settle() {
    let f = fixture();
    f.audio.write("New/New.m4b", &dune());
    assert_eq!(run(&f).audiobooks, 0);
}

#[test]
fn cancel_leaves_a_consistent_index() {
    let f = fixture();
    for i in 0..30 {
        f.audio.old(&format!("B{i}/B{i}.m4b"), &M4b::new(1).build());
    }
    let cancel = AtomicBool::new(false);
    let res = scan(&f.store, f.root.id, &LoftyReader, &cancel, &|p| {
        if p.phase == "reading" && p.done >= 3 {
            cancel.store(true, Ordering::SeqCst);
        }
    });
    assert!(matches!(res, Err(Error::Cancelled)));
    assert!(f.store.list().unwrap().len() <= 30);
    assert!(f.store.list().unwrap().iter().all(|r| r.state == AudioState::Present));
    assert_eq!(run(&f).audiobooks, 30);
    assert_eq!(f.store.list().unwrap().len(), 30);
    let _ = &f.lib;
}

#[test]
fn numbered_series_folder_is_split_into_books() {
    let f = fixture();
    for (i, title) in ["Killing Floor", "Die Trying", "Tripwire"].iter().enumerate() {
        let bytes = M4b::new(3_600_000)
            .text(b"\xa9nam", title)
            .text(b"\xa9alb", title)
            .text(b"\xa9ART", "Lee Child")
            .cover(jpeg(20, 20))
            .build();
        f.audio.old(
            &format!("Jack Reacher - Lee Child/0{} {title}_ Jack Reacher.m4b", i + 1),
            &bytes,
        );
    }
    f.audio.old("Jack Reacher - Lee Child/folder.jpg", &jpeg(10, 10));
    let reader = Counting(AtomicUsize::new(0));
    let cancel = AtomicBool::new(false);
    let s = scan(&f.store, f.root.id, &reader, &cancel, &|_| {}).unwrap();
    assert_eq!((s.audiobooks, s.added), (3, 3));
    let rows = f.store.list().unwrap();
    let mut titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
    titles.sort();
    assert_eq!(titles, ["Die Trying", "Killing Floor", "Tripwire"]);
    assert!(rows.iter().all(|r| r.file_count == 1 && r.duration_ms == 3_600_000 && r.has_cover));

    reader.0.store(0, Ordering::SeqCst);
    let s = scan(&f.store, f.root.id, &reader, &cancel, &|_| {}).unwrap();
    assert_eq!(reader.0.load(Ordering::SeqCst), 0);
    assert_eq!((s.audiobooks, s.unchanged, s.added, s.updated), (3, 3, 0, 0));
}

#[test]
fn duplicate_copies_count_once_and_are_reported() {
    let f = fixture();
    let old = f.audio.old("Dune [B002V1A0WE]/Dune_ Book [B002V1A0WE].m4b", &dune());
    age(&old, 90_000);
    f.audio.old("Dune [B002V1A0WE]/Dune? Book [B002V1A0WE].m4b", &dune());
    f.audio.old("Dune [B002V1A0WE]/Dune_ Book [B002V1A0WE].jpg", &jpeg(10, 10));
    f.audio.old("Dune [B002V1A0WE]/Dune? Book [B002V1A0WE].jpg", &jpeg(10, 10));
    let s = run(&f);
    assert_eq!((s.audiobooks, s.issues), (1, 1));
    let rows = f.store.list().unwrap();
    assert_eq!((rows[0].file_count, rows[0].duration_ms), (1, 60_000));
    let d = f.store.detail(rows[0].id).unwrap();
    assert!(d.files[0].rel_path.contains("Dune? Book"));
    let s = run(&f);
    assert_eq!((s.unchanged, s.issues), (1, 1));
    assert!(f.store.issues().unwrap()[0].message.contains("duplicate"));
}

#[test]
fn issues_survive_an_unchanged_rescan() {
    let f = fixture();
    f.audio.old("Broken [B00AAAAAA1]/Broken [B00AAAAAA1].m4b", b"garbage");
    assert_eq!(run(&f).issues, 1);
    assert_eq!(run(&f).issues, 1);
}

#[test]
fn a_chosen_duplicate_copy_is_used_and_hidden_issues_stay_hidden() {
    use shelfmark_audio::duplicates;

    let f = fixture();
    let older = "Dune [B002V1A0WE]/Dune_ Book [B002V1A0WE].m4b";
    let newer = "Dune [B002V1A0WE]/Dune - Book [B002V1A0WE].m4b";
    age(&f.audio.old(older, &dune()), 90_000);
    f.audio.old(newer, &dune());
    run(&f);
    let id = f.store.list().unwrap()[0].id;
    let issue = f.store.issues().unwrap().remove(0);
    assert_eq!((issue.path.as_str(), issue.kind.as_str()), (older, "duplicate"));

    let copies = duplicates::copies(&f.store, f.root.id, older, &LoftyReader).unwrap();
    assert_eq!(copies.len(), 2);
    assert_eq!(copies.iter().filter(|c| c.in_library).count(), 1);
    assert!(copies.iter().all(|c| c.title.as_deref() == Some("Dune") && c.has_cover));
    assert!(duplicates::resolve(&f.store, f.root.id, "../x.m4b").is_err());

    let all = duplicates::siblings(&f.store, f.root.id, older).unwrap();
    f.store.set_pick(f.root.id, older, &all).unwrap();
    let s = run(&f);
    assert_eq!((s.audiobooks, s.added, s.missing), (1, 0, 0));
    let rows = f.store.list().unwrap();
    assert_eq!((rows.len(), rows[0].id), (1, id));
    assert_eq!(f.store.detail(id).unwrap().files[0].rel_path, older);
    assert_eq!(f.store.issues().unwrap()[0].path, newer);

    f.store.ignore_issues(f.root.id, &all).unwrap();
    assert!(f.store.issues().unwrap().is_empty());
    run(&f);
    assert!(f.store.issues().unwrap().is_empty());
}
