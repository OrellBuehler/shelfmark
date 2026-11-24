use serde_json::Value;
use shelfmark_model::{IdentifyQuery, Metadata, SourceResult};

use super::{clean_isbn, get_json};
use crate::lang;
use crate::util::{clean, normalize_date, push_unique};

const API: &str = "https://www.googleapis.com/books/v1/volumes";

pub(super) async fn identify(client: &reqwest::Client, query: &IdentifyQuery) -> Vec<SourceResult> {
    let isbn = query
        .isbn
        .as_deref()
        .map(clean_isbn)
        .filter(|i| !i.is_empty());
    if let Some(isbn) = isbn {
        let q = format!("isbn:{isbn}");
        let found = fetch(client, &q).await;
        if !found.is_empty() {
            return found;
        }
    }
    let Some(title) = query.title.as_deref().and_then(clean) else {
        return Vec::new();
    };
    let mut q = format!("intitle:{title}");
    if let Some(a) = query.authors.first().and_then(|a| clean(a)) {
        q.push_str(&format!(" inauthor:{a}"));
    }
    fetch(client, &q).await
}

async fn fetch(client: &reqwest::Client, q: &str) -> Vec<SourceResult> {
    let key = std::env::var("SHELFMARK_GOOGLE_BOOKS_KEY")
        .ok()
        .filter(|k| !k.is_empty());
    let mut params = vec![("q", q), ("maxResults", "10"), ("printType", "books")];
    if let Some(k) = &key {
        params.push(("key", k.as_str()));
    }
    let json = get_json(client, API, &params).await;
    json.map(|j| parse_volumes(&j)).unwrap_or_default()
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).and_then(clean)
}

fn cover_url(links: &Value, id: Option<&str>) -> Option<String> {
    for key in ["extraLarge", "large", "medium", "small"] {
        if let Some(u) = links.get(key).and_then(Value::as_str) {
            return Some(sanitize(u));
        }
    }
    for key in ["thumbnail", "smallThumbnail"] {
        if let Some(u) = links.get(key).and_then(Value::as_str) {
            let u = sanitize(u);
            return Some(if u.contains("zoom=") {
                replace_zoom(&u, 3)
            } else {
                u
            });
        }
    }
    id.map(|id| format!("https://books.google.com/books/content?id={id}&printsec=frontcover&img=1&zoom=3&source=gbs_api"))
}

fn sanitize(u: &str) -> String {
    let u = u
        .replacen("http://", "https://", 1)
        .replace("&edge=curl", "")
        .replace("&amp;", "&");
    u.replace("edge=curl&", "")
}

fn replace_zoom(u: &str, zoom: u8) -> String {
    match u.find("zoom=") {
        Some(i) => {
            let rest = &u[i + 5..];
            let end = rest.find('&').unwrap_or(rest.len());
            format!("{}zoom={zoom}{}", &u[..i], &rest[end..])
        }
        None => u.to_string(),
    }
}

pub(crate) fn parse_volumes(json: &Value) -> Vec<SourceResult> {
    let Some(items) = json.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };
    items.iter().filter_map(parse_volume).collect()
}

fn parse_volume(item: &Value) -> Option<SourceResult> {
    let info = item.get("volumeInfo")?;
    let id = item.get("id").and_then(Value::as_str);
    let mut m = Metadata::default();
    let title = str_of(info, "title")?;
    m.title = Some(match str_of(info, "subtitle") {
        Some(sub) => format!("{title}: {sub}"),
        None => title,
    });
    if let Some(a) = info.get("authors").and_then(Value::as_array) {
        for a in a.iter().filter_map(Value::as_str).filter_map(clean) {
            push_unique(&mut m.authors, a);
        }
    }
    m.publisher = str_of(info, "publisher");
    m.pubdate = info
        .get("publishedDate")
        .and_then(Value::as_str)
        .and_then(normalize_date);
    m.comments = str_of(info, "description");
    if let Some(c) = info.get("categories").and_then(Value::as_array) {
        for cat in c.iter().filter_map(Value::as_str) {
            for part in cat.split('/') {
                if let Some(p) = clean(part) {
                    push_unique(&mut m.tags, p);
                }
            }
        }
    }
    if let Some(l) = info
        .get("language")
        .and_then(Value::as_str)
        .and_then(lang::normalize)
        && l != "und"
    {
        m.languages = vec![l];
    }
    if let Some(ids) = info.get("industryIdentifiers").and_then(Value::as_array) {
        let pick = |ty: &str| {
            ids.iter()
                .find(|i| i.get("type").and_then(Value::as_str) == Some(ty))
                .and_then(|i| i.get("identifier").and_then(Value::as_str))
                .map(clean_isbn)
        };
        if let Some(isbn) = pick("ISBN_13").or_else(|| pick("ISBN_10")) {
            m.identifiers.insert("isbn".into(), isbn);
        }
    }
    if let Some(id) = id {
        m.identifiers.insert("google".into(), id.to_string());
    }
    if let Some(r) = info
        .get("seriesInfo")
        .and_then(|s| s.get("bookDisplayNumber"))
        .and_then(Value::as_str)
    {
        m.series_index = r.parse().ok();
    }
    let cover = info.get("imageLinks").and_then(|l| cover_url(l, id));
    let url = str_of(info, "canonicalVolumeLink")
        .or_else(|| str_of(info, "infoLink"))
        .or_else(|| id.map(|id| format!("https://books.google.com/books?id={id}")))
        .map(|u| u.replacen("http://", "https://", 1));
    Some(SourceResult {
        source: "Google Books".into(),
        metadata: m,
        cover_url: cover,
        url,
    })
}
