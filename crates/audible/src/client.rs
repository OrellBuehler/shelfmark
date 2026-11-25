use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;

use crate::identity::Account;
use crate::sign::{Signer, signature_headers};
use crate::{Error, Result};

const USER_AGENT: &str = "Audible/671 CFNetwork/1240.0.4 Darwin/20.6.0";
const PAGE_SIZE: usize = 200;
const MAX_PAGES: usize = 50;
const PAGE_PAUSE: Duration = Duration::from_millis(400);
const MAX_COVER: usize = 5 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteBook {
    pub asin: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub release_date: Option<String>,
    pub duration_ms: u64,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub purchase_date: Option<String>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v[key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn names(v: &Value, key: &str) -> Vec<String> {
    v[key]
        .as_array()
        .map(|l| l.iter().filter_map(|p| text(p, "name")).collect())
        .unwrap_or_default()
}

pub fn parse_library(body: &Value) -> Vec<RemoteBook> {
    let Some(items) = body["items"].as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|it| {
            matches!(
                it["content_delivery_type"].as_str(),
                Some("SinglePartBook" | "MultiPartBook")
            )
        })
        .filter_map(|it| {
            let series = it["series"].as_array().and_then(|s| s.first());
            Some(RemoteBook {
                asin: text(it, "asin")?,
                title: text(it, "title")?,
                subtitle: text(it, "subtitle"),
                authors: names(it, "authors"),
                narrators: names(it, "narrators"),
                series: series.and_then(|s| text(s, "title")),
                series_index: series
                    .and_then(|s| text(s, "sequence"))
                    .and_then(|s| s.parse().ok()),
                publisher: text(it, "publisher_name"),
                language: text(it, "language"),
                release_date: text(it, "release_date").or_else(|| text(it, "issue_date")),
                duration_ms: it["runtime_length_min"].as_u64().unwrap_or(0) * 60_000,
                description: text(it, "publisher_summary")
                    .or_else(|| text(it, "merchandising_summary")),
                cover_url: text(&it["product_images"], "500"),
                purchase_date: text(it, "purchase_date"),
            })
        })
        .collect()
}

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(60))
        .build()?)
}

pub async fn fetch_library(
    account: &Account,
    cancel: &AtomicBool,
    on_page: &(dyn Fn(usize) + Sync),
) -> Result<Vec<RemoteBook>> {
    if let Some(problem) = &account.problem {
        return Err(Error::Account(problem.clone()));
    }
    let domain = account
        .domain
        .ok_or_else(|| Error::Account("unknown marketplace".into()))?;
    let signer = Signer::new(&account.private_key, &account.adp_token)?;
    let client = http()?;
    let mut out = Vec::new();
    for page in 1..=MAX_PAGES {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let path = format!(
            "/1.0/library?num_results={PAGE_SIZE}&page={page}&response_groups=product_desc,contributors,series,product_attrs,media,product_extended_attrs&image_sizes=500&sort_by=-PurchaseDate"
        );
        let mut request = client
            .get(format!("https://api.audible.{domain}{path}"))
            .header("Accept", "application/json");
        for (name, value) in signature_headers(&signer, "GET", &path, "")? {
            request = request.header(name, value);
        }
        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let message: String = response.text().await.unwrap_or_default().chars().take(200).collect();
            return Err(Error::Api {
                status: status.as_u16(),
                message,
            });
        }
        let body: Value = response.json().await?;
        let count = body["items"].as_array().map_or(0, Vec::len);
        out.extend(parse_library(&body));
        on_page(out.len());
        if count < PAGE_SIZE {
            break;
        }
        std::thread::sleep(PAGE_PAUSE);
    }
    Ok(out)
}

pub async fn fetch_cover(url: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    let response = http().ok()?.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= MAX_COVER).then(|| bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_books_and_skips_podcasts() {
        let body: Value = serde_json::from_str(
            r#"{"items":[
              {"asin":"B01GSIZ5AC","title":"Ego Is the Enemy","subtitle":"Sub","content_delivery_type":"SinglePartBook",
               "authors":[{"asin":"x","name":"Ryan Holiday"}],"narrators":[{"name":"Ryan Holiday"}],
               "series":[{"title":"Stoic","sequence":"2"}],"publisher_name":"Penguin","language":"english",
               "release_date":"2016-06-14","runtime_length_min":416,"publisher_summary":"<p>Text</p>",
               "product_images":{"500":"https://m.media-amazon.com/images/I/x._SL500_.jpg"},"purchase_date":"2020-01-01T00:00:00Z"},
              {"asin":"B0POD","title":"A Podcast","content_delivery_type":"PodcastParent"},
              {"asin":"B0NOTITLE","content_delivery_type":"MultiPartBook"},
              {"asin":"B0752ZQR33","title":"Why We Sleep","content_delivery_type":"MultiPartBook","series":[]}
            ]}"#,
        )
        .unwrap();
        let books = parse_library(&body);
        assert_eq!(books.len(), 2);
        let b = &books[0];
        assert_eq!(b.asin, "B01GSIZ5AC");
        assert_eq!(b.authors, ["Ryan Holiday"]);
        assert_eq!((b.series.as_deref(), b.series_index), (Some("Stoic"), Some(2.0)));
        assert_eq!(b.duration_ms, 416 * 60_000);
        assert!(b.cover_url.as_deref().unwrap().starts_with("https://"));
        assert_eq!(books[1].series, None);
        assert!(parse_library(&serde_json::json!({})).is_empty());
    }
}
