use futures_util::future::{join, join_all};
use serde_json::Value;
use shelfmark_model::{IdentifyQuery, Metadata, SourceResult};

use super::{clean_isbn, get_json};
use crate::lang;
use crate::util::{clean, normalize_date, parse_series, push_unique};

const BASE: &str = "https://openlibrary.org";
const FIELDS: &str = "key,title,subtitle,author_name,first_publish_year,publisher,isbn,language,subject,cover_i,cover_edition_key,edition_key";

pub(super) async fn identify(client: &reqwest::Client, query: &IdentifyQuery) -> Vec<SourceResult> {
    if let Some(isbn) = query
        .isbn
        .as_deref()
        .map(clean_isbn)
        .filter(|i| !i.is_empty())
    {
        if let Some(r) = by_isbn(client, &isbn).await {
            return vec![r];
        }
        let found = search(client, &[("q", &format!("isbn:{isbn}"))]).await;
        if !found.is_empty() {
            return found;
        }
    }
    let Some(title) = query.title.as_deref().and_then(clean) else {
        return Vec::new();
    };
    let mut params = vec![("title", title)];
    if let Some(a) = query.authors.first().and_then(|a| clean(a)) {
        params.push(("author", a));
    }
    let params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
    search(client, &params).await
}

async fn search(client: &reqwest::Client, params: &[(&str, &str)]) -> Vec<SourceResult> {
    let mut p = params.to_vec();
    p.push(("limit", "10"));
    p.push(("fields", FIELDS));
    get_json(client, &format!("{BASE}/search.json"), &p)
        .await
        .map(|j| parse_search(&j))
        .unwrap_or_default()
}

async fn by_isbn(client: &reqwest::Client, isbn: &str) -> Option<SourceResult> {
    let edition = get_json(client, &format!("{BASE}/isbn/{isbn}.json"), &[]).await?;
    let author_keys: Vec<String> = edition
        .get("authors")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("key").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let work_key = edition
        .get("works")
        .and_then(Value::as_array)
        .and_then(|w| w.first())
        .and_then(|w| w.get("key"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let authors_fut = join_all(
        author_keys
            .iter()
            .map(|k| async move { get_json(client, &format!("{BASE}{k}.json"), &[]).await }),
    );
    let work_fut = async {
        match &work_key {
            Some(k) => get_json(client, &format!("{BASE}{k}.json"), &[]).await,
            None => None,
        }
    };
    let (authors, work) = join(authors_fut, work_fut).await;
    let names: Vec<String> = authors
        .into_iter()
        .flatten()
        .filter_map(|a| a.get("name").and_then(Value::as_str).and_then(clean))
        .collect();
    parse_edition(&edition, &names, work.as_ref(), isbn)
}

fn description(v: &Value) -> Option<String> {
    let d = v.get("description")?;
    let s = d
        .as_str()
        .or_else(|| d.get("value").and_then(Value::as_str))?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter_map(clean)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn parse_edition(
    edition: &Value,
    authors: &[String],
    work: Option<&Value>,
    isbn: &str,
) -> Option<SourceResult> {
    let title = edition
        .get("title")
        .and_then(Value::as_str)
        .and_then(clean)?;
    let mut m = Metadata {
        title: Some(
            match edition
                .get("subtitle")
                .and_then(Value::as_str)
                .and_then(clean)
            {
                Some(s) => format!("{title}: {s}"),
                None => title,
            },
        ),
        ..Default::default()
    };
    m.authors = authors.to_vec();
    m.publisher = strings(edition, "publishers").into_iter().next();
    m.pubdate = edition
        .get("publish_date")
        .and_then(Value::as_str)
        .and_then(normalize_date);
    m.comments = description(edition).or_else(|| work.and_then(description));
    let subjects = work
        .map(|w| strings(w, "subjects"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| strings(edition, "subjects"));
    for s in subjects.into_iter().take(15) {
        push_unique(&mut m.tags, s);
    }
    let langs: Vec<String> = edition
        .get("languages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|l| l.get("key").and_then(Value::as_str))
                .filter_map(|k| k.rsplit('/').next())
                .filter_map(lang::normalize)
                .collect()
        })
        .unwrap_or_default();
    for l in langs {
        push_unique(&mut m.languages, l);
    }
    let isbn13 = strings(edition, "isbn_13").into_iter().next();
    let isbn10 = strings(edition, "isbn_10").into_iter().next();
    m.identifiers.insert(
        "isbn".into(),
        clean_isbn(&isbn13.or(isbn10).unwrap_or_else(|| isbn.to_string())),
    );
    let key = edition.get("key").and_then(Value::as_str);
    if let Some(olid) = key.and_then(|k| k.rsplit('/').next()) {
        m.identifiers.insert("openlibrary".into(), olid.to_string());
    }
    if let Some((name, idx)) = strings(edition, "series")
        .first()
        .and_then(|s| parse_series(s))
    {
        m.series = Some(name);
        m.series_index = idx;
    }
    let cover = edition
        .get("covers")
        .and_then(Value::as_array)
        .and_then(|c| c.iter().filter_map(Value::as_i64).find(|id| *id > 0))
        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"))
        .or_else(|| {
            Some(format!(
                "https://covers.openlibrary.org/b/isbn/{}-L.jpg?default=false",
                clean_isbn(isbn)
            ))
        });
    Some(SourceResult {
        source: "Open Library".into(),
        metadata: m,
        cover_url: cover,
        url: key.map(|k| format!("{BASE}{k}")),
    })
}

pub(crate) fn parse_search(json: &Value) -> Vec<SourceResult> {
    let Some(docs) = json.get("docs").and_then(Value::as_array) else {
        return Vec::new();
    };
    docs.iter().filter_map(parse_doc).collect()
}

fn parse_doc(doc: &Value) -> Option<SourceResult> {
    let title = doc.get("title").and_then(Value::as_str).and_then(clean)?;
    let mut m = Metadata {
        title: Some(
            match doc.get("subtitle").and_then(Value::as_str).and_then(clean) {
                Some(s) => format!("{title}: {s}"),
                None => title,
            },
        ),
        ..Default::default()
    };
    m.authors = strings(doc, "author_name");
    m.publisher = strings(doc, "publisher").into_iter().next();
    m.pubdate = doc
        .get("first_publish_year")
        .and_then(Value::as_i64)
        .and_then(|y| normalize_date(&y.to_string()));
    for s in strings(doc, "subject").into_iter().take(15) {
        push_unique(&mut m.tags, s);
    }
    for l in strings(doc, "language") {
        if let Some(l) = lang::normalize(&l) {
            push_unique(&mut m.languages, l);
        }
    }
    let isbns = strings(doc, "isbn");
    if let Some(i) = isbns.iter().find(|i| i.len() == 13).or(isbns.first()) {
        m.identifiers.insert("isbn".into(), clean_isbn(i));
    }
    let key = doc.get("key").and_then(Value::as_str);
    if let Some(olid) = key.and_then(|k| k.rsplit('/').next()) {
        m.identifiers.insert("openlibrary".into(), olid.to_string());
    }
    let cover = doc
        .get("cover_i")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"));
    Some(SourceResult {
        source: "Open Library".into(),
        metadata: m,
        cover_url: cover,
        url: key.map(|k| format!("{BASE}{k}")),
    })
}
