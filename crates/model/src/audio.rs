use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::BookId;

pub type AudiobookId = i64;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    All,
    #[default]
    Ebooks,
    Audiobooks,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    #[default]
    Ebook,
    Audiobook,
    Both,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioState {
    #[default]
    Present,
    Missing,
    Remote,
}

impl AudioState {
    pub fn as_str(self) -> &'static str {
        match self {
            AudioState::Present => "present",
            AudioState::Missing => "missing",
            AudioState::Remote => "remote",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "missing" => AudioState::Missing,
            "remote" => AudioState::Remote,
            _ => AudioState::Present,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AudioLayout {
    #[default]
    Libation,
    AuthorTitle,
}

impl AudioLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            AudioLayout::Libation => "libation",
            AudioLayout::AuthorTitle => "author-title",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "author-title" => AudioLayout::AuthorTitle,
            _ => AudioLayout::Libation,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioRoot {
    pub id: i64,
    pub path: String,
    pub in_library: bool,
    pub managed: bool,
    pub layout: AudioLayout,
    pub label: Option<String>,
    pub enabled: bool,
    pub last_scan: Option<String>,
    pub last_scan_ok: Option<bool>,
    #[serde(default)]
    pub audiobooks: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookRow {
    pub id: AudiobookId,
    pub root_id: Option<i64>,
    pub state: AudioState,
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub genres: Vec<String>,
    pub description: Option<String>,
    pub asin: Option<String>,
    pub isbn: Option<String>,
    pub release_date: Option<String>,
    pub duration_ms: u64,
    pub size: u64,
    pub format: String,
    pub file_count: u32,
    pub has_cover: bool,
    pub added: String,
    pub modified: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioBrief {
    pub narrators: Vec<String>,
    pub duration_ms: u64,
    pub format: String,
    pub file_count: u32,
    pub size: u64,
    pub asin: Option<String>,
    pub state: AudioState,
    pub count: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LinkState {
    Manual,
    Rejected,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioLink {
    pub audiobook_id: AudiobookId,
    pub book_id: BookId,
    pub book_uuid: Option<String>,
    pub state: LinkState,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioFile {
    pub rel_path: String,
    pub kind: String,
    pub size: u64,
    pub duration_ms: Option<u64>,
    pub track: Option<u32>,
    pub title: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioChapter {
    pub title: String,
    pub start_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookDetail {
    #[serde(flatten)]
    pub row: AudiobookRow,
    pub dir: Option<String>,
    pub files: Vec<AudioFile>,
    pub chapters: Vec<AudioChapter>,
}

pub trait AudioIndex: Send + Sync {
    fn generation(&self) -> u64;
    fn audiobooks(&self) -> Result<Arc<Vec<AudiobookRow>>, String>;
    fn links(&self) -> Result<Vec<AudioLink>, String>;
}

#[cfg(test)]
mod tests {
    use crate::{BookSummary, ListQuery};

    use super::*;

    #[test]
    fn list_query_defaults_to_ebooks() {
        let q: ListQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.scope, Scope::Ebooks);
        let q: ListQuery = serde_json::from_str(r#"{"scope":"audiobooks"}"#).unwrap();
        assert_eq!(q.scope, Scope::Audiobooks);
    }

    #[test]
    fn summary_audio_fields_are_camel_case() {
        let s = BookSummary {
            kind: EntryKind::Both,
            audiobook_id: Some(3),
            audio: Some(AudioBrief {
                duration_ms: 5,
                ..Default::default()
            }),
            ..Default::default()
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"], "both");
        assert_eq!(v["audiobookId"], 3);
        assert_eq!(v["audio"]["durationMs"], 5);
        assert_eq!(v["pairedBookId"], serde_json::Value::Null);
    }

    #[test]
    fn layout_round_trips() {
        for l in [AudioLayout::Libation, AudioLayout::AuthorTitle] {
            assert_eq!(AudioLayout::parse(l.as_str()), l);
        }
        assert_eq!(
            serde_json::to_string(&AudioLayout::AuthorTitle).unwrap(),
            "\"author-title\""
        );
    }
}
