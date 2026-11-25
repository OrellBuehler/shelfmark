use std::sync::LazyLock;

use regex::Regex;

static ASIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([0-9A-Za-z]{10})\]").unwrap());
static TRACK_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+[-_]\s+\d{1,4}(\s+[-_]\s.*)?$").unwrap());
static LEADING_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\d{1,4}\s*[-_.)]?\s*").unwrap());

pub fn asin_in(name: &str) -> Option<String> {
    ASIN.captures_iter(name)
        .find_map(|c| crate::tags::valid_asin(&c[1]))
}

pub fn split_ext(name: &str) -> (&str, String) {
    let lower = name.to_lowercase();
    if lower.ends_with(".metadata.json") {
        return (&name[..name.len() - ".metadata.json".len()], "metadata.json".into());
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, ext.to_lowercase()),
        _ => (name, String::new()),
    }
}

pub fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

pub fn file_name(rel: &str) -> &str {
    rel.rsplit_once('/').map_or(rel, |(_, f)| f)
}

pub fn title_from(name: &str) -> String {
    let no_asin = ASIN.replace_all(name, "");
    let trimmed = no_asin.trim().trim_matches(['-', '_']).trim();
    let collapsed: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        name.trim().to_string()
    } else {
        collapsed
    }
}

pub fn strip_track(stem: &str) -> String {
    let no_asin = ASIN.replace_all(stem, "");
    let s = TRACK_SUFFIX.replace(no_asin.trim(), "");
    let s = LEADING_NUMBER.replace(&s, "");
    title_from(&s)
}

pub fn leading_number(stem: &str) -> Option<u32> {
    let digits: String = stem
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!digits.is_empty() && digits.len() <= 4)
        .then(|| digits.parse().ok())
        .flatten()
}

pub fn number_after(stem: &str, prefix_len: usize) -> Option<u32> {
    stem.get(prefix_len..).and_then(leading_number)
}

pub fn common_prefix(names: &[&str]) -> usize {
    let Some(first) = names.first() else { return 0 };
    let mut len = first.len();
    for n in &names[1..] {
        len = len.min(
            first
                .bytes()
                .zip(n.bytes())
                .take_while(|(a, b)| a == b)
                .count(),
        );
    }
    while len > 0 && !first.is_char_boundary(len) {
        len -= 1;
    }
    let prefix = &first[..len];
    let trimmed = prefix.trim_end_matches(|c: char| c.is_ascii_digit());
    trimmed.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_asin_in_brackets_only() {
        assert_eq!(asin_in("Dune [B002V1A0WE]").as_deref(), Some("B002V1A0WE"));
        assert_eq!(asin_in("Dune [UNABRIDGED]"), None);
        assert_eq!(asin_in("Dune B002V1A0WE"), None);
        assert_eq!(
            asin_in("Dune [Unabridged] [B002V1A0WE] - 01 - Intro").as_deref(),
            Some("B002V1A0WE")
        );
    }

    #[test]
    fn extensions() {
        assert_eq!(split_ext("A [X].metadata.json"), ("A [X]", "metadata.json".into()));
        assert_eq!(split_ext("A.b.M4B"), ("A.b", "m4b".into()));
        assert_eq!(split_ext("noext"), ("noext", String::new()));
    }

    #[test]
    fn titles() {
        assert_eq!(title_from("Dune [B002V1A0WE]"), "Dune");
        assert_eq!(strip_track("Dune [B002V1A0WE] - 01 - Arrakis"), "Dune");
        assert_eq!(strip_track("03 - The Hobbit"), "The Hobbit");
        assert_eq!(strip_track("Catch-22"), "Catch-22");
    }

    #[test]
    fn prefixes_and_numbers() {
        let names = ["Book - Part 1", "Book - Part 2", "Book - Part 10"];
        let p = common_prefix(&names);
        assert_eq!(&names[0][..p], "Book - Part ");
        assert_eq!(number_after(names[2], p), Some(10));
        assert_eq!(common_prefix(&["Dune", "Dune Messiah"]), 4);
        assert_eq!(number_after("Dune Messiah", 4), None);
        assert_eq!(leading_number("01 - Intro"), Some(1));
        assert_eq!(leading_number("Intro"), None);
    }
}
