use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub type BookId = i64;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Metadata {
    pub title: Option<String>,
    pub title_sort: Option<String>,
    pub authors: Vec<String>,
    pub author_sort: Option<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub tags: Vec<String>,
    pub publisher: Option<String>,
    pub pubdate: Option<String>,
    pub languages: Vec<String>,
    pub identifiers: BTreeMap<String, String>,
    pub rating: Option<u8>,
    pub comments: Option<String>,
    pub uuid: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookSummary {
    pub id: BookId,
    pub title: String,
    pub sort: String,
    pub authors: Vec<String>,
    pub author_sort: String,
    pub series: Option<String>,
    pub series_index: f64,
    pub tags: Vec<String>,
    pub publisher: Option<String>,
    pub languages: Vec<String>,
    pub rating: Option<u8>,
    pub formats: Vec<String>,
    pub identifiers: BTreeMap<String, String>,
    pub has_cover: bool,
    pub size: u64,
    pub timestamp: String,
    pub pubdate: Option<String>,
    pub last_modified: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FormatFile {
    pub format: String,
    pub file_name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    #[serde(flatten)]
    pub summary: BookSummary,
    pub comments: Option<String>,
    pub uuid: String,
    pub path: String,
    pub files: Vec<FormatFile>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    Title,
    Author,
    Series,
    #[default]
    Added,
    Published,
    Modified,
    Rating,
    Size,
    Publisher,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ListQuery {
    pub search: Option<String>,
    pub sort_by: SortField,
    pub descending: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct BookPatch {
    pub title: Option<String>,
    pub title_sort: Option<String>,
    pub authors: Option<Vec<String>>,
    pub author_sort: Option<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub tags: Option<Vec<String>>,
    pub publisher: Option<String>,
    pub pubdate: Option<String>,
    pub languages: Option<Vec<String>>,
    pub identifiers: Option<BTreeMap<String, String>>,
    pub rating: Option<u8>,
    pub comments: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct BulkPatch {
    pub authors: Option<Vec<String>>,
    pub series: Option<String>,
    pub series_start: Option<f64>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub clear_tags: bool,
    pub publisher: Option<String>,
    pub pubdate: Option<String>,
    pub languages: Option<Vec<String>>,
    pub rating: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryItem {
    pub id: Option<i64>,
    pub name: String,
    pub count: u32,
    pub search: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub key: String,
    pub items: Vec<CategoryItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub name: String,
    pub query: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct IdentifyQuery {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub isbn: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceResult {
    pub source: String,
    pub metadata: Metadata,
    pub cover_url: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Kobo,
    Kindle,
    PocketBook,
    Tolino,
    #[default]
    Generic,
    Folder,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub root: String,
    pub books_dir: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub supported_formats: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceBook {
    pub path: String,
    pub rel_path: String,
    pub title: String,
    pub authors: Vec<String>,
    pub format: String,
    pub size: u64,
    pub modified: Option<String>,
    pub uuid: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadingState {
    pub book_id: BookId,
    pub format: String,
    pub position: Option<String>,
    pub progress: f64,
    pub updated: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationKind {
    #[default]
    Highlight,
    Bookmark,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub book_id: BookId,
    pub kind: AnnotationKind,
    pub position: String,
    pub text: Option<String>,
    pub note: Option<String>,
    pub color: Option<String>,
    pub chapter: Option<String>,
    pub created: String,
    pub updated: String,
}
