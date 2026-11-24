mod google;
mod openlibrary;

use std::time::Duration;

use futures_util::future::join;
use shelfmark_model::{IdentifyQuery, Metadata, SourceResult};

use crate::error::{Result, invalid};
use crate::util::sniff_image;

const USER_AGENT: &str = "shelfmark/0.1";
const MAX_COVER_BYTES: usize = 25 << 20;

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(10))
        .build()?)
}

pub async fn identify(query: &IdentifyQuery) -> Vec<SourceResult> {
    let Ok(client) = client() else {
        return Vec::new();
    };
    let (g, o) = join(
        google::identify(&client, query),
        openlibrary::identify(&client, query),
    )
    .await;
    let mut all: Vec<(f64, SourceResult)> = g
        .into_iter()
        .chain(o)
        .map(|r| (score(query, &r.metadata), r))
        .collect();
    all.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    all.into_iter().map(|(_, r)| r).collect()
}

pub async fn fetch_cover(url: &str) -> Result<Vec<u8>> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(invalid("cover", "unsupported url"));
    }
    let resp = client()?.get(url).send().await?.error_for_status()?;
    let bytes = resp.bytes().await?;
    if bytes.len() > MAX_COVER_BYTES || sniff_image(&bytes).is_none() || bytes.len() < 200 {
        return Err(invalid("cover", "response is not a usable image"));
    }
    Ok(bytes.to_vec())
}

pub(crate) async fn get_json(
    client: &reqwest::Client,
    url: &str,
    params: &[(&str, &str)],
) -> Option<serde_json::Value> {
    let resp = client
        .get(url)
        .query(params)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    resp.json().await.ok()
}

fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn overlap(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let common = a.iter().filter(|t| b.contains(t)).count() as f64;
    common / (a.len() + b.len()) as f64 * 2.0
}

pub(crate) fn score(q: &IdentifyQuery, m: &Metadata) -> f64 {
    let mut s = 0.0;
    if let Some(isbn) = q.isbn.as_deref().map(clean_isbn).filter(|i| !i.is_empty())
        && m.identifiers
            .get("isbn")
            .is_some_and(|i| clean_isbn(i) == isbn)
    {
        s += 3.0;
    }
    if let (Some(qt), Some(mt)) = (&q.title, &m.title) {
        let (a, b) = (tokens(qt), tokens(mt));
        s += overlap(&a, &b) * 2.0;
        if a == b {
            s += 0.5;
        }
    }
    if !q.authors.is_empty() && !m.authors.is_empty() {
        let a = tokens(&q.authors.join(" "));
        let b = tokens(&m.authors.join(" "));
        s += overlap(&a, &b);
    }
    if m.comments.is_some() {
        s += 0.05;
    }
    if m.publisher.is_some() {
        s += 0.02;
    }
    s
}

pub(crate) fn clean_isbn(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
pub(crate) use google::parse_volumes as parse_google;
#[cfg(test)]
pub(crate) use openlibrary::{parse_edition as parse_ol_edition, parse_search as parse_ol_search};
