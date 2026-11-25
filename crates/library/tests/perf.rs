mod common;

use std::time::Instant;

use common::*;
use shelfmark_model::*;

#[test]
#[ignore]
fn large_library_timings() {
    let n: i64 = std::env::var("SHELFMARK_PERF_BOOKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);
    let (_tmp, lib) = new_library();
    {
        let mut conn = raw(&lib);
        conn.create_scalar_function(
            "uuid4",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            |_| Ok("u".to_string()),
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        for i in 0..n / 5 {
            tx.execute(
                "INSERT INTO authors(name, sort) VALUES (?1, ?2)",
                (format!("Author{i} Name{i}"), format!("Name{i}, Author{i}")),
            )
            .unwrap();
        }
        for i in 0..300 {
            tx.execute("INSERT INTO tags(name) VALUES (?1)", [format!("tag{i}")])
                .unwrap();
        }
        for i in 0..n / 10 {
            tx.execute(
                "INSERT INTO series(name) VALUES (?1)",
                [format!("Series {i}")],
            )
            .unwrap();
        }
        let comment = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(10);
        for i in 0..n {
            tx.execute(
                "INSERT INTO books(title, series_index, author_sort, path, has_cover, timestamp, pubdate, last_modified) VALUES (?1, 1.0, 'x', ?2, 0, '2024-05-01 18:49:25.003532+00:00', '2010-01-01 00:00:00+00:00', '2024-05-01 18:49:25.003532+00:00')",
                (format!("Book number {i}"), format!("A/B ({i})")),
            )
            .unwrap();
            let id = i + 1;
            tx.execute(
                "INSERT INTO books_authors_link(book, author) VALUES (?1, ?2)",
                (id, i % (n / 5) + 1),
            )
            .unwrap();
            tx.execute(
                "INSERT INTO books_tags_link(book, tag) VALUES (?1, ?2)",
                (id, i % 300 + 1),
            )
            .unwrap();
            tx.execute(
                "INSERT INTO books_tags_link(book, tag) VALUES (?1, ?2)",
                (id, (i * 7) % 300 + 1),
            )
            .ok();
            if i % 3 == 0 {
                tx.execute(
                    "INSERT INTO books_series_link(book, series) VALUES (?1, ?2)",
                    (id, i % (n / 10) + 1),
                )
                .unwrap();
            }
            tx.execute("INSERT INTO data(book, format, uncompressed_size, name) VALUES (?1, 'EPUB', 1000, 'n')", [id]).unwrap();
            tx.execute(
                "INSERT INTO identifiers(book, type, val) VALUES (?1, 'isbn', ?2)",
                (id, format!("978{i:010}")),
            )
            .unwrap();
            tx.execute(
                "INSERT INTO comments(book, text) VALUES (?1, ?2)",
                (id, &comment),
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    assert_eq!(lib.book_count().unwrap(), n as u64);
    let time = |label: &str, f: &dyn Fn() -> usize| {
        let t = Instant::now();
        let r = f();
        eprintln!("{label}: {r} results in {:?}", t.elapsed());
    };
    let list = |s: &str| {
        lib.list_books(&ListQuery {
            search: if s.is_empty() { None } else { Some(s.into()) },
            sort_by: SortField::Title,
            descending: false,
            ..Default::default()
        })
        .unwrap()
        .len()
    };
    time("list all, sorted by title", &|| list(""));
    time("list all again", &|| list(""));
    time("bare word", &|| list("number 777"));
    time("tag and rating", &|| list("tags:=tag7 and rating:>3"));
    time("comments", &|| list("comments:dolor"));
    time("bare word needing comments", &|| list("zzzzzz"));
    time("categories", &|| lib.categories(None).unwrap().len());
    time("categories restricted", &|| {
        lib.categories(Some("tags:=tag7")).unwrap().len()
    });
    time("get_book", &|| lib.get_book(500).map(|_| 1).unwrap());
}
