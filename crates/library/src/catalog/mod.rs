pub mod pairing;

use std::collections::HashMap;

use shelfmark_model::{AudioLink, AudioState, AudiobookId, AudiobookRow, BookId};

use crate::dates;
use crate::langs;
use crate::record::{AudioFacet, Record};
use crate::text;

#[derive(Default)]
pub struct Built {
    pub overlay: HashMap<BookId, Record>,
    pub audio: Vec<Record>,
    pub unpaired: Vec<usize>,
    pub pairs: HashMap<AudiobookId, (BookId, bool)>,
}

fn facet(rows: &[&AudiobookRow]) -> AudioFacet {
    let first = rows[0];
    let mut f = AudioFacet {
        ids: rows.iter().map(|r| r.id).collect(),
        duration_ms: first.duration_ms,
        size: first.size,
        file_count: first.file_count,
        asin: first.asin.clone(),
        state: if rows.iter().any(|r| r.state == AudioState::Present) {
            AudioState::Present
        } else {
            first.state
        },
        ..Default::default()
    };
    for r in rows {
        for n in &r.narrators {
            if !f.narrators.contains(n) {
                f.narrators.push(n.clone());
            }
        }
        if !f.formats.contains(&r.format) {
            f.formats.push(r.format.clone());
        }
    }
    f
}

fn audio_record(row: &AudiobookRow, paired: Option<BookId>) -> Record {
    let languages: Vec<String> = row
        .language
        .as_deref()
        .and_then(|l| langs::canonicalize(&l.to_lowercase()))
        .into_iter()
        .collect();
    let mut identifiers = Vec::new();
    if let Some(asin) = &row.asin {
        identifiers.push(("audible".to_string(), asin.clone()));
    }
    if let Some(isbn) = &row.isbn {
        identifiers.push(("isbn".to_string(), isbn.clone()));
    }
    let author_sort = row
        .authors
        .iter()
        .map(|a| text::author_to_author_sort(a))
        .collect::<Vec<_>>()
        .join(" & ");
    let timestamp = dates::parse(&row.added);
    let last_modified = dates::parse(&row.modified);
    Record {
        id: -row.id,
        sort: text::title_sort(&row.title, languages.first().map(String::as_str)),
        title: row.title.clone(),
        authors: row.authors.clone(),
        author_sort,
        series: row.series.clone(),
        series_index: row.series_index.unwrap_or(1.0),
        tags: row.genres.clone(),
        publisher: row.publisher.clone(),
        languages,
        identifiers,
        has_cover: row.has_cover,
        size: row.size,
        timestamp,
        pubdate: row.release_date.as_deref().and_then(dates::parse),
        last_modified,
        timestamp_raw: row.added.clone(),
        last_modified_raw: row.modified.clone(),
        comments: row.description.clone(),
        audio: Some(facet(&[row])),
        ebook_paired: paired,
        ..Default::default()
    }
}

pub fn build(recs: &[Record], rows: &[AudiobookRow], links: &[AudioLink]) -> Built {
    let index = pairing::Index::new(recs);
    let pairs = index.pair(rows, links);
    let by_id: HashMap<BookId, &Record> = recs.iter().map(|r| (r.id, r)).collect();
    let mut grouped: HashMap<BookId, Vec<&AudiobookRow>> = HashMap::new();
    let mut audio = Vec::with_capacity(rows.len());
    let mut unpaired = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let paired = pairs.get(&row.id).map(|(b, _)| *b);
        match paired {
            Some(book) => grouped.entry(book).or_default().push(row),
            None => unpaired.push(i),
        }
        audio.push(audio_record(row, paired));
    }
    let overlay = grouped
        .into_iter()
        .filter_map(|(book, mut rows)| {
            rows.sort_by_key(|r| (r.state != AudioState::Present, r.id));
            let mut rec = (*by_id.get(&book)?).clone();
            rec.audio = Some(facet(&rows));
            Some((book, rec))
        })
        .collect();
    Built {
        overlay,
        audio,
        unpaired,
        pairs,
    }
}
