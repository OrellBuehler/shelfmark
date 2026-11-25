mod common;

use common::Dir;
use common::audio_gen::{M4b, Mp3, jpeg};
use shelfmark_audio::tags::read;

#[test]
fn reads_libation_m4b_tags() {
    let dir = Dir::new();
    let bytes = M4b::new(3_723_000)
        .text(b"\xa9nam", "Dune")
        .text(b"\xa9alb", "Dune")
        .text(b"\xa9ART", "Frank Herbert, Brian Herbert")
        .text(b"\xa9wrt", "Scott Brick")
        .text(b"\xa9gen", "Science Fiction; Classics")
        .text(b"asin", "B002V1A0WE")
        .free("com.pilabor.tone", "AUDIBLE_ASIN", "B002V1A0WE")
        .free("com.pilabor.tone", "SERIES", "Dune Chronicles")
        .free("com.pilabor.tone", "PART", "1")
        .free("com.pilabor.tone", "SUBTITLE", "Book One")
        .free("com.pilabor.tone", "PUBLISHER", "Macmillan")
        .free("com.pilabor.tone", "LANGUAGE", "english")
        .cover(jpeg(40, 40))
        .build();
    let path = dir.write("Dune [B002V1A0WE].m4b", &bytes);
    let t = read(&path, true).unwrap();
    assert_eq!(t.title.as_deref(), Some("Dune"));
    assert_eq!(t.authors, ["Frank Herbert", "Brian Herbert"]);
    assert_eq!(t.narrators, ["Scott Brick"]);
    assert_eq!(t.genres, ["Science Fiction", "Classics"]);
    assert_eq!(t.asin.as_deref(), Some("B002V1A0WE"));
    assert_eq!(t.series.as_deref(), Some("Dune Chronicles"));
    assert_eq!(t.series_index, Some(1.0));
    assert_eq!(t.subtitle.as_deref(), Some("Book One"));
    assert_eq!(t.publisher.as_deref(), Some("Macmillan"));
    assert_eq!(t.language.as_deref(), Some("english"));
    assert_eq!(t.duration_ms, 3_723_000);
    assert!(t.cover.is_some());
    assert!(read(&path, false).unwrap().cover.is_none());
}

#[test]
fn freeform_asin_is_used_when_atom_is_absent() {
    let dir = Dir::new();
    let bytes = M4b::new(1000)
        .text(b"\xa9nam", "X")
        .free("com.pilabor.tone", "AUDIBLE_ASIN", "b002v1a0we")
        .track(7)
        .build();
    let t = read(&dir.write("x.m4b", &bytes), false).unwrap();
    assert_eq!(t.asin.as_deref(), Some("B002V1A0WE"));
    assert_eq!(t.track, Some(7));
}

#[test]
fn untagged_m4b_still_has_duration() {
    let dir = Dir::new();
    let t = read(&dir.write("x.m4b", &M4b::new(5000).build()), true).unwrap();
    assert_eq!(t.duration_ms, 5000);
    assert!(t.title.is_none() && t.asin.is_none() && t.authors.is_empty());
}

#[test]
fn reads_mp3_frames_and_user_text() {
    let dir = Dir::new();
    let bytes = Mp3::new(20)
        .text(b"TIT2", "Chapter 1")
        .text(b"TALB", "Dune")
        .text(b"TPE1", "Frank Herbert")
        .text(b"TCOM", "Scott Brick")
        .text(b"TRCK", "3/40")
        .user("AUDIBLE_ASIN", "B002V1A0WE")
        .user("SERIES", "Dune Chronicles")
        .user("PART", "1")
        .build();
    let t = read(&dir.write("x.mp3", &bytes), false).unwrap();
    assert_eq!(t.title.as_deref(), Some("Chapter 1"));
    assert_eq!(t.album.as_deref(), Some("Dune"));
    assert_eq!(t.authors, ["Frank Herbert"]);
    assert_eq!(t.narrators, ["Scott Brick"]);
    assert_eq!(t.track, Some(3));
    assert_eq!(t.asin.as_deref(), Some("B002V1A0WE"));
    assert_eq!(t.series.as_deref(), Some("Dune Chronicles"));
    assert_eq!(t.series_index, Some(1.0));
    assert!((19_000..=21_500).contains(&t.duration_ms), "{}", t.duration_ms);
}

#[test]
fn corrupt_files_are_errors() {
    let dir = Dir::new();
    assert!(read(&dir.write("bad.m4b", b"not an mp4 at all"), false).is_err());
    assert!(read(&dir.path().join("missing.m4b"), false).is_err());
}
