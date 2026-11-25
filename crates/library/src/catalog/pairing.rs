use std::collections::{HashMap, HashSet};

use shelfmark_model::{AudioLink, AudiobookId, AudiobookRow, BookId, LinkState};

use crate::record::Record;
use crate::text;

pub const AUTO_THRESHOLD: f32 = 0.85;
const SUGGEST_THRESHOLD: f32 = 0.5;

const NOISE: &[&str] = &[
    "unabridged",
    "abridged",
    "ungekurzt",
    "ungekuerzt",
    "gekurzt",
    "horbuch",
    "hoerbuch",
    "audiobook",
    "audiolibro",
];
const ARTICLES: &[&str] = &[
    "the", "a", "an", "der", "die", "das", "ein", "eine", "le", "la", "les", "un", "une", "el",
    "los", "las", "il", "lo",
];
const SUFFIXES: &[&str] = &["jr", "sr", "ii", "iii", "iv", "phd", "md", "dr", "esq"];
const NOBODY: &[&str] = &["unknown", "unbekannt", "various", "anonymous"];

fn words(s: &str) -> Vec<String> {
    text::primary(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn strip_bracketed(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn key(s: &str) -> String {
    let mut w: Vec<String> = words(&strip_bracketed(s))
        .into_iter()
        .filter(|w| !NOISE.contains(&w.as_str()))
        .collect();
    if w.len() > 1 && ARTICLES.contains(&w[0].as_str()) {
        w.remove(0);
    }
    w.concat()
}

fn split_subtitle(title: &str) -> Option<(&str, &str)> {
    let t = title.trim();
    [": ", " - ", " \u{2013} ", " \u{2014} "]
        .iter()
        .filter_map(|sep| t.find(sep).map(|i| (i, sep.len())))
        .min()
        .map(|(i, len)| (t[..i].trim(), t[i + len..].trim()))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TitleKeys {
    pub full: String,
    pub base: Option<String>,
    pub tail: Option<String>,
}

pub fn title_keys(title: &str) -> TitleKeys {
    let clean = strip_bracketed(title);
    let parts = split_subtitle(&clean);
    let part = |s: &str| Some(key(s)).filter(|k| k.chars().count() >= 3);
    TitleKeys {
        full: key(&clean),
        base: parts.and_then(|(b, _)| part(b)),
        tail: parts.and_then(|(_, t)| part(t)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorKey {
    surname: String,
    initial: Option<char>,
}

pub fn author_key(name: &str) -> Option<AuthorKey> {
    let reordered = match name.split_once(',') {
        Some((last, first)) if !SUFFIXES.contains(&key(first).as_str()) => {
            format!("{first} {last}")
        }
        _ => name.to_string(),
    };
    let mut tokens: Vec<String> = words(&reordered)
        .into_iter()
        .filter(|w| !SUFFIXES.contains(&w.as_str()))
        .collect();
    let surname = tokens.pop()?;
    if tokens.is_empty() && NOBODY.contains(&surname.as_str()) {
        return None;
    }
    Some(AuthorKey {
        surname,
        initial: tokens.first().and_then(|t| t.chars().next()),
    })
}

fn author_keys(names: &[String]) -> Vec<AuthorKey> {
    names.iter().filter_map(|n| author_key(n)).collect()
}

fn authors_overlap(a: &[AuthorKey], b: &[AuthorKey]) -> bool {
    a.iter().any(|x| {
        b.iter().any(|y| {
            x.surname == y.surname
                && match (x.initial, y.initial) {
                    (Some(p), Some(q)) => p == q,
                    _ => true,
                }
        })
    })
}

struct Ebook<'a> {
    rec: &'a Record,
    keys: TitleKeys,
    authors: Vec<AuthorKey>,
}

pub struct Index<'a> {
    books: Vec<Ebook<'a>>,
    by_key: HashMap<String, Vec<usize>>,
    by_ident: HashMap<String, Vec<usize>>,
    by_id: HashMap<BookId, usize>,
    by_uuid: HashMap<&'a str, usize>,
}

fn ident_keys(r: &Record) -> impl Iterator<Item = String> + '_ {
    r.identifiers.iter().filter_map(|(k, v)| {
        let v = v.trim();
        if v.is_empty() {
            return None;
        }
        match k.as_str() {
            "amazon" | "asin" | "mobi-asin" | "audible" => Some(format!("asin:{}", v.to_uppercase())),
            "isbn" => Some(format!("isbn:{}", v.replace('-', ""))),
            _ => None,
        }
    })
}

impl<'a> Index<'a> {
    pub fn new(recs: &'a [Record]) -> Self {
        let mut index = Index {
            books: Vec::with_capacity(recs.len()),
            by_key: HashMap::new(),
            by_ident: HashMap::new(),
            by_id: HashMap::new(),
            by_uuid: HashMap::new(),
        };
        for (i, rec) in recs.iter().enumerate() {
            let keys = title_keys(&rec.title);
            for k in [Some(&keys.full), keys.base.as_ref(), keys.tail.as_ref()]
                .into_iter()
                .flatten()
            {
                if !k.is_empty() {
                    index.by_key.entry(k.clone()).or_default().push(i);
                }
            }
            for k in ident_keys(rec) {
                index.by_ident.entry(k).or_default().push(i);
            }
            index.by_id.insert(rec.id, i);
            if !rec.uuid.is_empty() {
                index.by_uuid.insert(rec.uuid.as_str(), i);
            }
            index.books.push(Ebook {
                rec,
                keys,
                authors: author_keys(&rec.authors),
            });
        }
        index
    }

    fn resolve(&self, link: &AudioLink) -> Option<BookId> {
        let by_id = self.by_id.get(&link.book_id).map(|&i| self.books[i].rec);
        match (by_id, link.book_uuid.as_deref()) {
            (Some(r), Some(uuid)) if !r.uuid.is_empty() && r.uuid != uuid => self
                .by_uuid
                .get(uuid)
                .map(|&i| self.books[i].rec.id)
                .or(Some(r.id)),
            (Some(r), _) => Some(r.id),
            (None, Some(uuid)) => self.by_uuid.get(uuid).map(|&i| self.books[i].rec.id),
            (None, None) => None,
        }
    }

    pub fn candidates(&self, a: &AudiobookRow) -> Vec<(BookId, f32)> {
        let mut scores: HashMap<usize, f32> = HashMap::new();
        let mut bump = |i: usize, s: f32| {
            let e = scores.entry(i).or_insert(0.0);
            if s > *e {
                *e = s;
            }
        };
        let idents = a
            .asin
            .iter()
            .map(|v| format!("asin:{}", v.to_uppercase()))
            .chain(a.isbn.iter().map(|v| format!("isbn:{}", v.replace('-', ""))));
        for k in idents {
            for &i in self.by_ident.get(&k).into_iter().flatten() {
                bump(i, 1.0);
            }
        }
        let keys = title_keys(&a.title);
        let authors = author_keys(&a.authors);
        let mine = [Some(&keys.full), keys.base.as_ref(), keys.tail.as_ref()];
        for k in mine.into_iter().flatten().filter(|k| !k.is_empty()) {
            for &i in self.by_key.get(k).into_iter().flatten() {
                let e = &self.books[i];
                let both_full = keys.full == e.keys.full;
                let one_full = both_full
                    || *k == e.keys.full
                    || e.keys.base.as_deref() == Some(keys.full.as_str())
                    || e.keys.tail.as_deref() == Some(keys.full.as_str());
                if !one_full {
                    continue;
                }
                let known = !authors.is_empty() && !e.authors.is_empty();
                let overlap = authors_overlap(&authors, &e.authors);
                if known && !overlap {
                    continue;
                }
                if !both_full
                    && let (Some(ai), Some(_), Some(_)) =
                        (a.series_index, a.series.as_ref(), e.rec.series.as_ref())
                    && (ai - e.rec.series_index).abs() > f64::EPSILON
                {
                    continue;
                }
                bump(
                    i,
                    match (both_full, overlap) {
                        (true, true) => 1.0,
                        (false, true) => 0.9,
                        (true, false) => 0.8,
                        (false, false) => 0.6,
                    },
                );
            }
        }
        let mut out: Vec<(usize, f32)> = scores
            .into_iter()
            .filter(|(_, s)| *s >= SUGGEST_THRESHOLD)
            .collect();
        out.sort_by(|x, y| {
            y.1.total_cmp(&x.1)
                .then_with(|| self.books[x.0].rec.id.cmp(&self.books[y.0].rec.id))
        });
        out.into_iter()
            .map(|(i, s)| (self.books[i].rec.id, s))
            .collect()
    }

    fn same_work(&self, a: BookId, b: BookId) -> bool {
        match (self.by_id.get(&a), self.by_id.get(&b)) {
            (Some(&x), Some(&y)) => self.books[x].keys.full == self.books[y].keys.full,
            _ => false,
        }
    }

    pub fn pair(
        &self,
        audio: &[AudiobookRow],
        links: &[AudioLink],
    ) -> HashMap<AudiobookId, (BookId, bool)> {
        let mut out: HashMap<AudiobookId, (BookId, bool)> = HashMap::new();
        let mut rejected: HashSet<(AudiobookId, BookId)> = HashSet::new();
        for l in links {
            let Some(book) = self.resolve(l) else { continue };
            match l.state {
                LinkState::Manual => {
                    out.insert(l.audiobook_id, (book, true));
                }
                LinkState::Rejected => {
                    rejected.insert((l.audiobook_id, book));
                }
            }
        }
        for a in audio {
            if out.contains_key(&a.id) {
                continue;
            }
            let cands: Vec<(BookId, f32)> = self
                .candidates(a)
                .into_iter()
                .filter(|(b, _)| !rejected.contains(&(a.id, *b)))
                .collect();
            let Some(&(best, score)) = cands.first() else {
                continue;
            };
            if score < AUTO_THRESHOLD {
                continue;
            }
            let rival = cands[1..]
                .iter()
                .any(|(b, s)| *s >= score && !self.same_work(best, *b));
            if !rival {
                out.insert(a.id, (best, false));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: i64, title: &str, authors: &[&str]) -> Record {
        Record {
            id,
            title: title.into(),
            authors: authors.iter().map(|s| s.to_string()).collect(),
            uuid: format!("uuid-{id}"),
            series_index: 1.0,
            ..Default::default()
        }
    }

    fn audio(id: i64, title: &str, authors: &[&str]) -> AudiobookRow {
        AudiobookRow {
            id,
            title: title.into(),
            authors: authors.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn paired(recs: &[Record], a: &[AudiobookRow], links: &[AudioLink]) -> Vec<(i64, i64, bool)> {
        let mut v: Vec<_> = Index::new(recs)
            .pair(a, links)
            .into_iter()
            .map(|(a, (b, m))| (a, b, m))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn keys_ignore_noise_articles_and_brackets() {
        assert_eq!(key("The Hobbit (Unabridged)"), "hobbit");
        assert_eq!(key("Der Hobbit [Ungekürzt]"), "hobbit");
        assert_eq!(key("Can’t Hurt Me"), key("Can't Hurt Me"));
        let k = title_keys("Killing Floor: Jack Reacher, Book 1");
        assert_eq!(k.base.as_deref(), Some("killingfloor"));
        assert_eq!(k.tail.as_deref(), Some("jackreacherbook1"));
        assert_eq!(title_keys("Catch-22").base, None);
    }

    #[test]
    fn author_forms_match() {
        let k = |s: &str| author_keys(&[s.to_string()]);
        assert!(authors_overlap(&k("J.K. Rowling"), &k("Rowling, J.K.")));
        assert!(authors_overlap(&k("J. R. R. Tolkien"), &k("John Ronald Reuel Tolkien")));
        assert!(authors_overlap(&k("Martin Luther King, Jr."), &k("Martin Luther King")));
        assert!(!authors_overlap(&k("Frank Herbert"), &k("Brian Herbert")));
        assert!(k("Unknown").is_empty());
    }

    #[test]
    fn pairs_by_title_author_and_subtitle() {
        let recs = [
            rec(1, "Dune", &["Frank Herbert"]),
            rec(2, "Killing Floor", &["Lee Child"]),
            rec(3, "Harry Potter und der Stein der Weisen", &["Rowling, J.K."]),
            rec(4, "The Lady of Caladan", &["Brian Herbert", "Kevin J. Anderson"]),
        ];
        let a = [
            audio(10, "Dune (Unabridged)", &["Frank Herbert"]),
            audio(11, "Killing Floor: Jack Reacher, Book 1", &["Lee Child"]),
            audio(12, "Harry Potter und der Stein der Weisen", &["J.K. Rowling"]),
            audio(13, "Dune: The Lady of Caladan", &["Brian Herbert", "Kevin J. Anderson"]),
            audio(14, "Dune Messiah", &["Frank Herbert"]),
        ];
        assert_eq!(
            paired(&recs, &a, &[]),
            [(10, 1, false), (11, 2, false), (12, 3, false), (13, 4, false)]
        );
    }

    #[test]
    fn different_authors_or_editions_do_not_auto_pair() {
        let recs = [
            rec(1, "Dune", &["Frank Herbert"]),
            rec(2, "Harry Potter and the Sorcerer's Stone", &["J.K. Rowling"]),
            rec(3, "Mistborn: The Final Empire", &["Brandon Sanderson"]),
        ];
        let a = [
            audio(10, "Dune", &["Someone Else"]),
            audio(11, "Harry Potter and the Philosopher's Stone", &["J.K. Rowling"]),
            audio(12, "Mistborn: The Well of Ascension", &["Brandon Sanderson"]),
            audio(13, "Dune", &[]),
        ];
        assert!(paired(&recs, &a, &[]).is_empty());
        let index = Index::new(&recs);
        assert_eq!(index.candidates(&a[3]), [(1, 0.8)]);
    }

    #[test]
    fn identifiers_win_and_ambiguity_blocks() {
        let mut with_asin = rec(1, "Completely Different", &["X Y"]);
        with_asin.identifiers.push(("mobi-asin".into(), "b002v1a0we".into()));
        let recs = [
            with_asin,
            rec(2, "Twins", &["A Author"]),
            rec(3, "Twins", &["A Author"]),
            rec(4, "Night", &["A Author"]),
            rec(5, "Night: A Memoir", &["A Author"]),
        ];
        let mut tagged = audio(10, "Dune", &["Frank Herbert"]);
        tagged.asin = Some("B002V1A0WE".into());
        let a = [
            tagged,
            audio(11, "Twins", &["A Author"]),
            audio(12, "Night", &["A Author"]),
        ];
        assert_eq!(
            paired(&recs, &a, &[]),
            [(10, 1, false), (11, 2, false), (12, 4, false)]
        );
    }

    #[test]
    fn manual_and_rejected_links_are_honoured() {
        let recs = [rec(1, "Dune", &["Frank Herbert"]), rec(2, "Other", &["X Y"])];
        let a = [audio(10, "Dune", &["Frank Herbert"]), audio(11, "Zzz", &[])];
        let link = |audiobook_id, book_id, uuid: Option<&str>, state| AudioLink {
            audiobook_id,
            book_id,
            book_uuid: uuid.map(str::to_string),
            state,
        };
        assert_eq!(
            paired(&recs, &a, &[link(10, 1, None, LinkState::Rejected)]),
            []
        );
        assert_eq!(
            paired(&recs, &a, &[link(11, 2, None, LinkState::Manual)]),
            [(10, 1, false), (11, 2, true)]
        );
        assert_eq!(
            paired(&recs, &a, &[link(11, 99, Some("uuid-2"), LinkState::Manual)]),
            [(10, 1, false), (11, 2, true)]
        );
        assert_eq!(
            paired(&recs, &a, &[link(11, 99, None, LinkState::Manual)]),
            [(10, 1, false)]
        );
    }
}
