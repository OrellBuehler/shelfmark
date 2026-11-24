use super::*;
use crate::dates;
use crate::record::FormatRec;

fn rec() -> Record {
    Record {
        id: 7,
        title: "Les Misérables".into(),
        sort: "Misérables, Les".into(),
        authors: vec!["Victor Hugo".into(), "Anne O'Neil".into()],
        author_sort: "Hugo, Victor & O'Neil, Anne".into(),
        series: Some("Rougon-Macquart".into()),
        series_index: 2.0,
        tags: vec!["Classic".into(), "A.B.C".into(), ".hidden".into()],
        publisher: Some("Éditions Gallimard".into()),
        languages: vec!["fra".into()],
        rating: 9,
        formats: vec![
            FormatRec {
                format: "EPUB".into(),
                size: 2048,
                name: "x".into(),
            },
            FormatRec {
                format: "PDF".into(),
                size: 4096,
                name: "x".into(),
            },
        ],
        identifiers: vec![
            ("isbn".into(), "9781234567897".into()),
            ("mobi-asin".into(), "B00X".into()),
        ],
        has_cover: true,
        size: 4096,
        timestamp: dates::parse("2024-05-01 18:49:25+00:00"),
        pubdate: dates::parse("1862-01-01 00:00:00+00:00"),
        last_modified: dates::parse("2024-05-02 18:49:25+00:00"),
        uuid: "abc-def".into(),
        pages: Some(1200),
        ..Default::default()
    }
}

fn m(query: &str) -> bool {
    let c = compile(query, &|_| None).unwrap();
    c.matches(
        &rec(),
        Some("<p>The <b>convict</b> Jean Valjean — 100% bread.</p>"),
    )
}

fn err(query: &str) -> bool {
    compile(query, &|_| None).is_err()
}

#[test]
fn contains_ignores_case_accents_and_punctuation() {
    assert!(m("miserables"));
    assert!(m("MISÉRABLES"));
    assert!(m("title:les mis"));
    assert!(m("title:\"les mis\""));
    assert!(m("oneil"));
    assert!(m("authors:o'neil"));
    assert!(m("authors:\"anne oneil\""));
    assert!(m("rougon macquart"));
    assert!(m("editions"));
    assert!(m("publisher:gallimard"));
    assert!(!m("title:xyz"));
    assert!(
        m("series:-"),
        "queries made only of punctuation fall back to a plain substring match"
    );
}

#[test]
fn equals_regex_accent() {
    assert!(m("title:\"=les misérables\""));
    assert!(!m("title:\"=les miserables\""));
    assert!(m("title:^miserables"));
    assert!(m("title:\"~^les\\s+mis\""));
    assert!(!m("title:\"~^mis\""));
    assert!(m("publisher:~gallimard$"));
    assert!(m("tags:=classic"));
    assert!(!m("tags:=class"));
    assert!(m("tags:\"=..hidden\""));
    assert!(m("tags:\"=.a\""), "hierarchical prefix");
    assert!(m("tags:\"=..b\""), "hierarchical component");
    assert!(!m("tags:\"=..d\""));
    assert!(!m("title:\\=x"));
    assert!(err("title:~["));
}

#[test]
fn boolean_structure() {
    assert!(m("hugo and misérables"));
    assert!(m("hugo misérables"));
    assert!(!m("hugo and zola"));
    assert!(m("zola or hugo"));
    assert!(m("not zola"));
    assert!(!m("not hugo"));
    assert!(m("(zola or hugo) and (classic or nothing)"));
    assert!(m("hugo and not (zola or dumas)"));
    assert!(m("not not hugo"));
}

#[test]
fn all_search_fields() {
    assert!(m("convict"));
    assert!(m("valjean"));
    assert!(m("gallimard"));
    assert!(m("epub"));
    assert!(
        m("french"),
        "language names are resolved for the languages field"
    );
    assert!(m("languages:french"));
    assert!(m("languages:fr"));
    assert!(m("languages:\"=fra\""));
    assert!(m("B00X"));
    assert!(m("mobi-asin"));
    assert!(m("hugo, victor"));
    assert!(m("9781234567897"));
    assert!(m("4"), "bare number matches rating in stars");
}

#[test]
fn numbers() {
    assert!(m("rating:4"));
    assert!(m("rating:>=4"));
    assert!(m("rating:>3"));
    assert!(!m("rating:>4"));
    assert!(m("rating:!=3"));
    assert!(m("rating:4.5"));
    assert!(m("size:>1k"));
    assert!(m("size:4k"));
    assert!(!m("size:>4k"));
    assert!(m("size:<1m"));
    assert!(m("pages:>1000"));
    assert!(m("pages:true"));
    assert!(m("id:7"));
    assert!(!m("id:8"));
    assert!(m("series_index:2"));
    assert!(m("cover:true"));
    assert!(!m("cover:false"));
    assert!(m("authors:#=2"));
    assert!(m("authors:#>1"));
    assert!(!m("authors:#>2"));
    assert!(m("formats:#=2"));
    assert!(err("rating:x"));
    assert!(err("id:1.5"));
    assert!(err("size:>abc"));
}

#[test]
fn dates_in_records() {
    assert!(m("date:2024"));
    assert!(m("date:2024-05"));
    assert!(m("date:>=2024-05-01"));
    assert!(!m("date:>2024-05-01"));
    assert!(m("pubdate:<1900"));
    assert!(m("pubdate:1862"));
    assert!(m("last_modified:>2024-05-01"));
    assert!(m("date:true"));
    assert!(!m("date:false"));
    assert!(err("date:bogus"));
}

#[test]
fn existence_checks() {
    assert!(m("series:true"));
    assert!(!m("series:false"));
    assert!(m("publisher:true"));
    assert!(m("comments:true"));
    assert!(m("identifiers:true"));
    assert!(m("identifiers:isbn:true"));
    assert!(!m("identifiers:amazon:true"));
    assert!(m("identifiers:amazon:false"));
    assert!(m("identifiers:\"=isbn:\""));
    assert!(
        m("identifiers:\"=isbn:978\""),
        "values are matched with contains"
    );
    assert!(!m("identifiers:\"=isbn:=978\""));
    assert!(m("identifiers:isbn:978"));
    assert!(m("identifiers:\"=mobi-asin:b00x\""));
    assert!(m("isbn:9781234567897"));
    assert!(!m("isbn:999"));
    assert!(m("languages:true"));
    assert!(m("tags:true"));
    assert!(m("formats:true"));
}

#[test]
fn saved_searches_expand_and_detect_cycles() {
    let saved = |name: &str| match name {
        "a" => Some("title:misérables".to_string()),
        "b" => Some("search:a and hugo".to_string()),
        "loop1" => Some("search:loop2".to_string()),
        "loop2" => Some("search:loop1".to_string()),
        "bad" => Some("(".to_string()),
        _ => None,
    };
    let r = rec();
    assert!(compile("search:B", &saved).unwrap().matches(&r, None));
    assert!(compile("search:=a", &saved).unwrap().matches(&r, None));
    assert!(
        compile("search:\"a\" and search:a", &saved)
            .unwrap()
            .matches(&r, None)
    );
    assert!(compile("search:loop1", &saved).is_err());
    assert!(compile("search:bad", &saved).is_err());
    assert!(compile("search:missing", &saved).is_err());
}

#[test]
fn comments_need_flag() {
    assert!(
        compile("comments:convict", &|_| None)
            .unwrap()
            .needs_comments()
    );
    assert!(compile("anything", &|_| None).unwrap().needs_comments());
    assert!(
        !compile("title:x and tags:y", &|_| None)
            .unwrap()
            .needs_comments()
    );
    assert!(m("comments:\"100%\""));
    assert!(m("comments:\"~jean\\s+valjean\""));
    assert!(m("comments:\"jean valjean\""));
}

#[test]
fn unknown_fields_are_plain_text() {
    assert!(!m("foo:bar"));
    assert!(!m("search"));
    assert!(!m("title:\"\""));
    assert!(m("author:hugo"));
    assert!(m("tag:classic"));
    assert!(m("language:fra"));
    assert!(m("format:pdf"));
    assert!(m("title_sort:\"=misérables, les\""));
    assert!(m("author_sort:\"hugo, victor\""));
    assert!(m("series_sort:\"=rougon-macquart\""));
    assert!(m("uuid:abc-def"));
}
