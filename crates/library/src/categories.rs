use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use shelfmark_model::{Category, CategoryItem};

use crate::error::Result;
use crate::langs;
use crate::text;

struct Acc {
    id: Option<i64>,
    name: String,
    sort: String,
    count: u32,
    search: String,
}

pub fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    if value.starts_with('.') {
        format!(".{escaped}")
    } else {
        escaped
    }
}

fn equals_search(field: &str, value: &str) -> String {
    format!("{field}:\"={}\"", quote(value))
}

fn finish(map: HashMap<String, Acc>) -> Vec<CategoryItem> {
    let mut items: Vec<Acc> = map.into_values().collect();
    items.sort_by(|a, b| {
        text::sort_key(&a.sort)
            .cmp(&text::sort_key(&b.sort))
            .then_with(|| a.name.cmp(&b.name))
    });
    items
        .into_iter()
        .map(|a| CategoryItem {
            id: a.id,
            name: a.name,
            count: a.count,
            search: a.search,
        })
        .collect()
}

fn named(
    conn: &Connection,
    sql: &str,
    key: &str,
    visible: Option<&HashSet<i64>>,
    transform: impl Fn(&str) -> String,
    sort_from_column: bool,
) -> Result<Vec<CategoryItem>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    let mut map: HashMap<String, Acc> = HashMap::new();
    while let Some(r) = rows.next()? {
        let book: i64 = r.get(0)?;
        if visible.is_some_and(|v| !v.contains(&book)) {
            continue;
        }
        let id: i64 = r.get(1)?;
        let name: String = transform(&r.get::<_, String>(2)?);
        let sort: Option<String> = if sort_from_column { r.get(3)? } else { None };
        let acc = map.entry(id.to_string()).or_insert_with(|| Acc {
            id: Some(id),
            sort: sort
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| name.clone()),
            search: equals_search(key, &name),
            name,
            count: 0,
        });
        acc.count += 1;
    }
    Ok(finish(map))
}

pub fn build(conn: &Connection, visible: Option<&HashSet<i64>>) -> Result<Vec<Category>> {
    let authors = named(
        conn,
        "SELECT l.book, a.id, a.name, a.sort FROM books_authors_link l JOIN authors a ON a.id=l.author",
        "authors",
        visible,
        |n| n.replace('|', ","),
        true,
    )?;
    let series = named(
        conn,
        "SELECT l.book, s.id, s.name, s.sort FROM books_series_link l JOIN series s ON s.id=l.series",
        "series",
        visible,
        str::to_string,
        true,
    )?;
    let tags = named(
        conn,
        "SELECT l.book, t.id, t.name FROM books_tags_link l JOIN tags t ON t.id=l.tag",
        "tags",
        visible,
        str::to_string,
        false,
    )?;
    let publisher = named(
        conn,
        "SELECT l.book, p.id, p.name FROM books_publishers_link l JOIN publishers p ON p.id=l.publisher",
        "publisher",
        visible,
        str::to_string,
        false,
    )?;

    let mut languages: HashMap<String, Acc> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT l.book, g.id, g.lang_code FROM books_languages_link l JOIN languages g ON g.id=l.lang_code")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            if visible.is_some_and(|v| !v.contains(&r.get::<_, i64>(0).unwrap_or(-1))) {
                continue;
            }
            let id: i64 = r.get(1)?;
            let code: String = r.get(2)?;
            languages
                .entry(id.to_string())
                .or_insert_with(|| {
                    let name = langs::name(&code)
                        .map(str::to_string)
                        .unwrap_or_else(|| code.clone());
                    Acc {
                        id: Some(id),
                        sort: name.clone(),
                        name,
                        count: 0,
                        search: equals_search("languages", &code),
                    }
                })
                .count += 1;
        }
    }

    let mut formats: HashMap<String, Acc> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT book, format FROM data")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            if visible.is_some_and(|v| !v.contains(&r.get::<_, i64>(0).unwrap_or(-1))) {
                continue;
            }
            let fmt = r.get::<_, String>(1)?.to_uppercase();
            formats
                .entry(fmt.clone())
                .or_insert_with(|| Acc {
                    id: None,
                    sort: fmt.clone(),
                    search: equals_search("formats", &fmt),
                    name: fmt.clone(),
                    count: 0,
                })
                .count += 1;
        }
    }

    let mut ratings: Vec<CategoryItem> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT l.book, r.rating FROM books_ratings_link l JOIN ratings r ON r.id=l.rating WHERE r.rating > 1")?;
        let mut rows = stmt.query([])?;
        let mut counts: HashMap<i64, u32> = HashMap::new();
        while let Some(r) = rows.next()? {
            if visible.is_some_and(|v| !v.contains(&r.get::<_, i64>(0).unwrap_or(-1))) {
                continue;
            }
            *counts.entry(r.get::<_, i64>(1)? / 2).or_default() += 1;
        }
        let mut stars: Vec<i64> = counts.keys().copied().collect();
        stars.sort_unstable_by(|a, b| b.cmp(a));
        for s in stars {
            ratings.push(CategoryItem {
                id: None,
                name: s.to_string(),
                count: counts[&s],
                search: format!("rating:{s}"),
            });
        }
    }

    let mut identifiers: HashMap<String, Acc> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT book, type FROM identifiers")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            if visible.is_some_and(|v| !v.contains(&r.get::<_, i64>(0).unwrap_or(-1))) {
                continue;
            }
            let typ: String = r.get(1)?;
            identifiers
                .entry(typ.to_lowercase())
                .or_insert_with(|| Acc {
                    id: None,
                    sort: typ.clone(),
                    search: format!("identifiers:\"={}:\"", quote(&typ)),
                    name: typ.clone(),
                    count: 0,
                })
                .count += 1;
        }
    }

    Ok(vec![
        Category {
            key: "authors".into(),
            items: authors,
        },
        Category {
            key: "series".into(),
            items: series,
        },
        Category {
            key: "tags".into(),
            items: tags,
        },
        Category {
            key: "publisher".into(),
            items: publisher,
        },
        Category {
            key: "languages".into(),
            items: finish(languages),
        },
        Category {
            key: "formats".into(),
            items: finish(formats),
        },
        Category {
            key: "rating".into(),
            items: ratings,
        },
        Category {
            key: "identifiers".into(),
            items: finish(identifiers),
        },
    ])
}
