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

fn confident(q: &IdentifyQuery, m: &Metadata) -> bool {
    if let Some(isbn) = q.isbn.as_deref().map(clean_isbn).filter(|i| !i.is_empty())
        && m.identifiers
            .get("isbn")
            .is_some_and(|i| clean_isbn(i) == isbn)
    {
        return true;
    }
    let (Some(qt), Some(mt)) = (&q.title, &m.title) else {
        return false;
    };
    let (a, b) = (tokens(qt), tokens(mt));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let qa = tokens(&q.authors.join(" "));
    let known = !qa.is_empty() && qa != ["unknown"];
    let authors = known && overlap(&qa, &tokens(&m.authors.join(" "))) >= 0.5;
    let subset = a.iter().all(|t| b.contains(t)) || b.iter().all(|t| a.contains(t));
    if known {
        authors && (subset || overlap(&a, &b) >= 0.8)
    } else {
        a == b
    }
}

pub fn pick_best(query: &IdentifyQuery, results: Vec<SourceResult>) -> Option<SourceResult> {
    let mut matches = results
        .into_iter()
        .filter(|r| confident(query, &r.metadata));
    let mut best = matches.next()?;
    for other in matches {
        let (m, o) = (&mut best.metadata, other.metadata);
        m.comments = m.comments.take().or(o.comments);
        m.publisher = m.publisher.take().or(o.publisher);
        m.pubdate = m.pubdate.take().or(o.pubdate);
        if m.series.is_none() && o.series.is_some() {
            m.series = o.series;
            m.series_index = o.series_index;
        }
        if m.tags.is_empty() {
            m.tags = o.tags;
        }
        if m.languages.is_empty() {
            m.languages = o.languages;
        }
        for (k, v) in o.identifiers {
            m.identifiers.entry(k).or_insert(v);
        }
        best.cover_url = best.cover_url.take().or(other.cover_url);
    }
    Some(best)
}

pub async fn best_match(query: &IdentifyQuery) -> Option<SourceResult> {
    pick_best(query, identify(query).await)
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

#[cfg(test)]
mod best_tests {
    use super::*;

    fn result(title: &str, author: &str) -> SourceResult {
        SourceResult {
            source: "test".into(),
            metadata: Metadata {
                title: Some(title.into()),
                authors: vec![author.into()],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn query(title: &str, authors: &[&str]) -> IdentifyQuery {
        IdentifyQuery {
            title: Some(title.into()),
            authors: authors.iter().map(|a| a.to_string()).collect(),
            isbn: None,
        }
    }

    #[test]
    fn rejects_other_books() {
        let q = query("Dune", &["Frank Herbert"]);
        assert!(pick_best(&q, vec![result("Dune", "Someone Else")]).is_none());
        assert!(pick_best(&q, vec![result("Emma", "Frank Herbert")]).is_none());
        let q = query("Dune", &["Unknown"]);
        assert!(pick_best(&q, vec![result("Dune Messiah", "Frank Herbert")]).is_none());
    }

    #[test]
    fn accepts_subtitle_and_name_order() {
        let q = query("Dune", &["Herbert, Frank"]);
        assert!(pick_best(&q, vec![result("Dune: A Novel", "Frank Herbert")]).is_some());
    }

    #[test]
    fn accepts_isbn_and_fills_gaps() {
        let mut q = query("Wrong title", &[]);
        q.isbn = Some("978-0-441-17271-9".into());
        let mut a = result("Dune", "Frank Herbert");
        a.metadata
            .identifiers
            .insert("isbn".into(), "9780441172719".into());
        let mut b = a.clone();
        b.metadata.comments = Some("Desert planet.".into());
        b.metadata.identifiers.insert("google".into(), "x".into());
        b.cover_url = Some("https://example.org/c.jpg".into());
        let best = pick_best(&q, vec![a, result("Other", "Nobody"), b]).unwrap();
        assert_eq!(best.metadata.comments.as_deref(), Some("Desert planet."));
        assert_eq!(best.metadata.identifiers.len(), 2);
        assert!(best.cover_url.is_some());
    }
}
