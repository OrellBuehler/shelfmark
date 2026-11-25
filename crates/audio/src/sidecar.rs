use serde_json::Value;
use shelfmark_model::AudioChapter;

use crate::tags::{FileTags, valid_asin};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sidecar {
    pub tags: FileTags,
    pub chapters: Vec<AudioChapter>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn names(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|p| text(p, "name"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn chapters(list: &Value, prefix: Option<&str>, out: &mut Vec<AudioChapter>) {
    let Some(items) = list.as_array() else { return };
    for c in items {
        let title = text(c, "title").unwrap_or_default();
        let full = match prefix {
            Some(p) if !p.is_empty() => format!("{p}: {title}"),
            _ => title,
        };
        if let Some(start) = c.get("start_offset_ms").and_then(Value::as_u64) {
            out.push(AudioChapter {
                title: full.clone(),
                start_ms: start,
            });
        }
        if let Some(nested) = c.get("chapters") {
            chapters(nested, Some(&full), out);
        }
    }
}

pub fn parse(bytes: &[u8]) -> Option<Sidecar> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if !v.is_object() {
        return None;
    }
    let mut tags = FileTags {
        title: text(&v, "title"),
        subtitle: text(&v, "subtitle"),
        authors: names(&v, "authors"),
        narrators: names(&v, "narrators"),
        publisher: text(&v, "publisher_name"),
        language: text(&v, "language"),
        description: text(&v, "publisher_summary").or_else(|| text(&v, "merchandising_summary")),
        asin: text(&v, "asin").as_deref().and_then(valid_asin),
        date: text(&v, "release_date").or_else(|| text(&v, "issue_date")),
        ..Default::default()
    };
    if let Some(first) = v
        .get("series")
        .and_then(Value::as_array)
        .and_then(|s| s.first())
    {
        tags.series = text(first, "title");
        tags.series_index = text(first, "sequence").and_then(|s| s.parse().ok());
    }
    if let Some(min) = v.get("runtime_length_min").and_then(Value::as_u64) {
        tags.duration_ms = min * 60_000;
    }
    let mut list = Vec::new();
    let info = v.get("ChapterInfo").or_else(|| v.get("chapter_info"));
    if let Some(info) = info {
        if let Some(ms) = info.get("runtime_length_ms").and_then(Value::as_u64) {
            tags.duration_ms = ms;
        }
        if let Some(c) = info.get("chapters") {
            chapters(c, None, &mut list);
        }
    }
    list.sort_by_key(|c| c.start_ms);
    Some(Sidecar {
        tags,
        chapters: list,
    })
}

pub fn parse_cue(text: &str) -> Vec<AudioChapter> {
    let mut out = Vec::new();
    let mut title: Option<String> = None;
    let mut in_track = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("TRACK ") {
            in_track = true;
            title = None;
        } else if in_track && let Some(rest) = line.strip_prefix("TITLE ") {
            title = Some(rest.trim().trim_matches('"').to_string());
        } else if in_track && let Some(rest) = line.strip_prefix("INDEX 01 ") {
            let parts: Vec<u64> = rest
                .trim()
                .split(':')
                .filter_map(|p| p.parse().ok())
                .collect();
            if let [m, s, f] = parts[..] {
                out.push(AudioChapter {
                    title: title.take().unwrap_or_else(|| format!("Chapter {}", out.len() + 1)),
                    start_ms: m * 60_000 + s * 1000 + f * 1000 / 75,
                });
            }
            in_track = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_catalog_product() {
        let json = br#"{
          "asin": "B002V1A0WE", "title": "Dune", "subtitle": "Book One",
          "authors": [{"asin": "x", "name": "Frank Herbert"}],
          "narrators": [{"name": "Scott Brick"}, {"name": "Orlagh Cassidy"}],
          "series": [{"asin": "s", "sequence": "1", "title": "Dune Chronicles"}],
          "publisher_name": "Macmillan", "release_date": "2007-01-01",
          "runtime_length_min": 1263, "language": "english",
          "ChapterInfo": {"runtime_length_ms": 75780000, "chapters": [
            {"title": "Book One", "start_offset_ms": 0, "length_ms": 10, "chapters": [
              {"title": "Chapter 1", "start_offset_ms": 5000, "length_ms": 5}
            ]},
            {"title": "Credits", "start_offset_ms": 9000, "length_ms": 5}
          ]}
        }"#;
        let s = parse(json).unwrap();
        assert_eq!(s.tags.asin.as_deref(), Some("B002V1A0WE"));
        assert_eq!(s.tags.narrators, ["Scott Brick", "Orlagh Cassidy"]);
        assert_eq!(s.tags.series.as_deref(), Some("Dune Chronicles"));
        assert_eq!(s.tags.series_index, Some(1.0));
        assert_eq!(s.tags.duration_ms, 75_780_000);
        let titles: Vec<_> = s.chapters.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["Book One", "Book One: Chapter 1", "Credits"]);
    }

    #[test]
    fn parses_cue_sheets() {
        let cue = "FILE \"Dune [B002V1A0WE].m4b\" MP4\n  TRACK 1 AUDIO\n    TITLE \"Opening Credits\"\n    INDEX 01 00:00:00\n  TRACK 2 AUDIO\n    TITLE \"Chapter 1\"\n    INDEX 01 01:30:37\n  TRACK 3 AUDIO\n    INDEX 01 75:00:00\n";
        let c = parse_cue(cue);
        assert_eq!(c.len(), 3);
        assert_eq!((c[0].title.as_str(), c[0].start_ms), ("Opening Credits", 0));
        assert_eq!((c[1].title.as_str(), c[1].start_ms), ("Chapter 1", 90_493));
        assert_eq!((c[2].title.as_str(), c[2].start_ms), ("Chapter 3", 4_500_000));
        assert!(parse_cue("garbage").is_empty());
    }

    #[test]
    fn tolerates_sparse_and_garbage() {
        let s = parse(br#"{"asin": "B002V1A0WE"}"#).unwrap();
        assert_eq!(s.tags.asin.as_deref(), Some("B002V1A0WE"));
        assert!(s.tags.title.is_none() && s.chapters.is_empty());
        assert!(parse(b"not json").is_none());
        assert!(parse(b"[1,2]").is_none());
    }
}
