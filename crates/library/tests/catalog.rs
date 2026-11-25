mod common;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use common::{meta, new_library};
use shelfmark_library::Library;
use shelfmark_model::{
    AudioIndex, AudioLink, AudiobookRow, BookSummary, EntryKind, LinkState, ListQuery, Metadata,
    Scope, SortField,
};

#[derive(Default)]
struct Fake {
    generation: AtomicU64,
    rows: Mutex<Vec<AudiobookRow>>,
    links: Mutex<Vec<AudioLink>>,
}

impl Fake {
    fn set(&self, rows: Vec<AudiobookRow>, links: Vec<AudioLink>) {
        *self.rows.lock().unwrap() = rows;
        *self.links.lock().unwrap() = links;
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

impl AudioIndex for Fake {
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    fn audiobooks(&self) -> Result<Arc<Vec<AudiobookRow>>, String> {
        Ok(Arc::new(self.rows.lock().unwrap().clone()))
    }
    fn links(&self) -> Result<Vec<AudioLink>, String> {
        Ok(self.links.lock().unwrap().clone())
    }
}

fn audio(id: i64, title: &str, author: &str, narrator: &str, hours: u64) -> AudiobookRow {
    AudiobookRow {
        id,
        title: title.into(),
        authors: vec![author.into()],
        narrators: vec![narrator.into()],
        genres: vec!["Audio Genre".into()],
        duration_ms: hours * 3_600_000,
        size: hours * 1000,
        format: "M4B".into(),
        file_count: 1,
        asin: Some(format!("B00000000{id}")),
        added: "2026-03-01T00:00:00Z".into(),
        modified: "2026-03-01T00:00:00Z".into(),
        description: Some("a spoken tale".into()),
        ..Default::default()
    }
}

fn setup() -> (tempfile::TempDir, Library, Fake, [i64; 3]) {
    let (tmp, lib) = new_library();
    let dune = lib
        .add_book(
            &Metadata {
                tags: vec!["scifi".into()],
                comments: Some("desert planet".into()),
                ..meta("Dune", &["Frank Herbert"])
            },
            None,
            &[],
        )
        .unwrap();
    let emma = lib.add_book(&meta("Emma", &["Jane Austen"]), None, &[]).unwrap();
    let hobbit = lib
        .add_book(&meta("The Hobbit", &["J. R. R. Tolkien"]), None, &[])
        .unwrap();
    let fake = Fake::default();
    fake.set(
        vec![
            audio(1, "Dune (Unabridged)", "Frank Herbert", "Scott Brick", 21),
            audio(2, "Project Hail Mary", "Andy Weir", "Ray Porter", 16),
            audio(3, "The Hobbit", "Tolkien, J.R.R.", "Andy Serkis", 11),
        ],
        vec![],
    );
    (tmp, lib, fake, [dune, emma, hobbit])
}

fn list(lib: &Library, fake: &Fake, scope: Scope, search: &str) -> Vec<BookSummary> {
    lib.list_entries(
        &ListQuery {
            search: (!search.is_empty()).then(|| search.to_string()),
            sort_by: SortField::Title,
            descending: false,
            scope,
        },
        fake,
    )
    .unwrap()
}

fn titles(v: &[BookSummary]) -> Vec<&str> {
    v.iter().map(|b| b.title.as_str()).collect()
}

#[test]
fn scopes_merge_paired_titles_once() {
    let (_t, lib, fake, [dune, emma, hobbit]) = setup();
    let all = list(&lib, &fake, Scope::All, "");
    assert_eq!(titles(&all), ["Dune", "Emma", "The Hobbit", "Project Hail Mary"]);
    let by = |t: &str| all.iter().find(|b| b.title == t).unwrap();
    assert_eq!((by("Dune").id, by("Dune").kind), (dune, EntryKind::Both));
    assert_eq!(by("Dune").audiobook_id, Some(1));
    assert_eq!(by("Dune").audio.as_ref().unwrap().narrators, ["Scott Brick"]);
    assert_eq!((by("Emma").id, by("Emma").kind), (emma, EntryKind::Ebook));
    assert!(by("Emma").audio.is_none());
    assert_eq!(by("The Hobbit").id, hobbit);
    let phm = by("Project Hail Mary");
    assert_eq!((phm.id, phm.kind, phm.audiobook_id), (-2, EntryKind::Audiobook, Some(2)));
    assert!(phm.formats.is_empty());
    assert_eq!(phm.identifiers["audible"], "B000000002");

    let ebooks = list(&lib, &fake, Scope::Ebooks, "");
    assert_eq!(titles(&ebooks), ["Dune", "Emma", "The Hobbit"]);
    assert_eq!(ebooks[0].kind, EntryKind::Both);

    let audiobooks = list(&lib, &fake, Scope::Audiobooks, "");
    assert_eq!(titles(&audiobooks), ["Dune (Unabridged)", "The Hobbit", "Project Hail Mary"]);
    assert_eq!(audiobooks[0].id, -1);
    assert_eq!(audiobooks[0].kind, EntryKind::Both);
    assert_eq!(audiobooks[0].paired_book_id, Some(dune));
    assert_eq!(audiobooks[2].paired_book_id, None);
}

#[test]
fn search_uses_one_merged_record_per_entry() {
    let (_t, lib, fake, _) = setup();
    let q = |s: &str| {
        let v = list(&lib, &fake, Scope::All, s);
        v.iter().map(|b| b.title.clone()).collect::<Vec<_>>()
    };
    assert_eq!(q("audio:true"), ["Dune", "The Hobbit", "Project Hail Mary"]);
    assert_eq!(q("audio:false"), ["Emma"]);
    assert_eq!(q("ebook:false"), ["Project Hail Mary"]);
    assert_eq!(q("audio:true and ebook:true"), ["Dune", "The Hobbit"]);
    assert_eq!(q("not tags:scifi"), ["Emma", "The Hobbit", "Project Hail Mary"]);
    assert_eq!(q("narrators:serkis"), ["The Hobbit"]);
    assert_eq!(q("porter"), ["Project Hail Mary"]);
    assert_eq!(q("duration:>15"), ["Dune", "Project Hail Mary"]);
    assert_eq!(q("duration:<12h"), ["The Hobbit"]);
    assert_eq!(q("formats:m4b"), ["Dune", "The Hobbit", "Project Hail Mary"]);
    assert_eq!(q("comments:spoken"), ["Project Hail Mary"]);
    assert_eq!(q("comments:desert"), ["Dune"]);
    assert_eq!(q("authors:weir"), ["Project Hail Mary"]);
    assert!(lib.list_entries(
        &ListQuery { search: Some("duration:abc".into()), scope: Scope::All, ..Default::default() },
        &fake
    ).is_err());
}

#[test]
fn sorting_mixes_both_kinds() {
    let (_t, lib, fake, _) = setup();
    let sorted = |by, descending| {
        let v = lib
            .list_entries(
                &ListQuery { search: None, sort_by: by, descending, scope: Scope::All },
                &fake,
            )
            .unwrap();
        v.iter().map(|b| b.title.clone()).collect::<Vec<_>>()
    };
    assert_eq!(
        sorted(SortField::Duration, true),
        ["Dune", "Project Hail Mary", "The Hobbit", "Emma"]
    );
    assert_eq!(
        sorted(SortField::Author, false),
        ["Emma", "Dune", "The Hobbit", "Project Hail Mary"]
    );
}

#[test]
fn links_change_pairing_and_cache_follows_both_sides() {
    let (_t, lib, fake, [dune, emma, _]) = setup();
    assert_eq!(lib.audio_pairs(&fake).unwrap()[&1], (dune, false));

    let rows = fake.audiobooks().unwrap().as_ref().clone();
    let link = |audiobook_id, book_id, state| AudioLink {
        audiobook_id,
        book_id,
        book_uuid: None,
        state,
    };
    fake.set(
        rows.clone(),
        vec![link(1, dune, LinkState::Rejected), link(2, emma, LinkState::Manual)],
    );
    let all = list(&lib, &fake, Scope::All, "");
    assert_eq!(titles(&all), ["Dune", "Dune (Unabridged)", "Emma", "The Hobbit"]);
    assert_eq!(all[0].kind, EntryKind::Ebook);
    assert_eq!(all[1].kind, EntryKind::Audiobook);
    assert_eq!(all[2].kind, EntryKind::Both);
    assert_eq!(lib.audio_pairs(&fake).unwrap()[&2], (emma, true));

    fake.set(rows, vec![]);
    lib.delete_books(&[dune]).unwrap();
    let all = list(&lib, &fake, Scope::All, "");
    assert_eq!(titles(&all), ["Dune (Unabridged)", "Emma", "The Hobbit", "Project Hail Mary"]);

    let again = lib.add_book(&meta("Project Hail Mary", &["Andy Weir"]), None, &[]).unwrap();
    let all = list(&lib, &fake, Scope::All, "");
    let phm = all.iter().find(|b| b.title == "Project Hail Mary").unwrap();
    assert_eq!((phm.id, phm.kind), (again, EntryKind::Both));
}

#[test]
fn several_audiobooks_can_share_one_ebook() {
    let (_t, lib, fake, [dune, ..]) = setup();
    let mut rows = fake.audiobooks().unwrap().as_ref().clone();
    rows.push(audio(4, "Dune", "Frank Herbert", "Simon Vance", 20));
    fake.set(rows, vec![]);
    let all = list(&lib, &fake, Scope::All, "");
    let d = all.iter().find(|b| b.id == dune).unwrap();
    let brief = d.audio.as_ref().unwrap();
    assert_eq!(brief.count, 2);
    assert_eq!(brief.narrators, ["Scott Brick", "Simon Vance"]);
    assert_eq!(list(&lib, &fake, Scope::Audiobooks, "").len(), 4);
}

#[test]
fn candidates_suggest_weaker_matches() {
    let (_t, lib, fake, [_, emma, _]) = setup();
    let mut rows = fake.audiobooks().unwrap().as_ref().clone();
    let mut anon = audio(5, "Emma", "", "Someone", 9);
    anon.authors.clear();
    rows.push(anon);
    fake.set(rows, vec![]);
    assert!(!lib.audio_pairs(&fake).unwrap().contains_key(&5));
    let c = lib.audio_candidates(5, &fake).unwrap();
    assert_eq!((c[0].0.id, c[0].1), (emma, 0.8));
    assert!(lib.audio_candidates(999, &fake).unwrap().is_empty());
}

#[test]
fn in_memory_categories_match_sql_for_ebooks_and_add_audio_groups() {
    let (_t, lib, fake, _) = setup();
    lib.add_book(
        &Metadata {
            series: Some("The Chronicles".into()),
            tags: vec!["scifi".into(), "classic".into()],
            publisher: Some("Ace".into()),
            languages: vec!["eng".into()],
            rating: Some(8),
            identifiers: [("isbn".to_string(), "9780441013593".to_string())].into(),
            ..meta("Zeta", &["Ursula K. Le Guin", "Frank Herbert"])
        },
        None,
        &[],
    )
    .unwrap();
    let empty = Fake::default();
    let sql = lib.categories(None).unwrap();
    let mem = lib.categories_entries(Scope::Ebooks, None, &empty).unwrap();
    for cat in &sql {
        let other = mem.iter().find(|c| c.key == cat.key).unwrap();
        let strip = |items: &[shelfmark_model::CategoryItem]| {
            items
                .iter()
                .map(|i| (i.name.clone(), i.count, i.search.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(strip(&cat.items), strip(&other.items), "category {}", cat.key);
    }

    let cats = lib.categories_entries(Scope::All, None, &fake).unwrap();
    let get = |key: &str| &cats.iter().find(|c| c.key == key).unwrap().items;
    let audio: Vec<_> = get("audio").iter().map(|i| (i.name.as_str(), i.count)).collect();
    assert_eq!(
        audio,
        [
            ("Has audiobook", 3),
            ("Has ebook", 4),
            ("Both formats", 2),
            ("No audiobook", 2),
            ("No ebook", 1)
        ]
    );
    assert_eq!(get("narrators").len(), 3);
    assert_eq!(get("narrators")[0].search, "narrators:\"=Scott Brick\"");
    assert!(get("formats").iter().any(|i| i.name == "M4B" && i.count == 3));

    let filtered = lib
        .categories_entries(Scope::All, Some("narrators:\"=Ray Porter\""), &fake)
        .unwrap();
    let authors = &filtered.iter().find(|c| c.key == "authors").unwrap().items;
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Andy Weir");
}

#[test]
fn remote_audiobooks_are_listed_and_filterable() {
    let (_t, lib, fake, [dune, ..]) = setup();
    let mut rows = fake.audiobooks().unwrap().as_ref().clone();
    rows[0].state = shelfmark_model::AudioState::Remote;
    rows[1].state = shelfmark_model::AudioState::Remote;
    fake.set(rows, vec![]);
    let q = |s: &str| {
        let v = list(&lib, &fake, Scope::All, s);
        v.iter().map(|b| b.title.clone()).collect::<Vec<_>>()
    };
    assert_eq!(q("downloaded:false and audio:true"), ["Dune", "Project Hail Mary"]);
    assert_eq!(q("downloaded:true"), ["The Hobbit"]);
    let all = list(&lib, &fake, Scope::All, "");
    let d = all.iter().find(|b| b.id == dune).unwrap();
    assert_eq!(d.audio.as_ref().unwrap().state, shelfmark_model::AudioState::Remote);
    let cats = lib.categories_entries(Scope::All, None, &fake).unwrap();
    let audio = &cats.iter().find(|c| c.key == "audio").unwrap().items;
    let item = audio.iter().find(|i| i.name == "Not downloaded").unwrap();
    assert_eq!((item.count, item.search.as_str()), (2, "audio:true and downloaded:false"));
}
