use shelfmark_model::AudioLayout;

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn sanitize(s: &str, max_chars: usize, fallback: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let collapsed = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(max_chars).collect();
    let mut out = truncated.trim_matches([' ', '.']).to_string();
    if out.is_empty() {
        out = fallback.to_string();
    }
    let base = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&base.as_str()) {
        out.insert(0, '_');
    }
    out
}

pub fn book_dir(layout: AudioLayout, title: &str, authors: &[String], asin: Option<&str>) -> String {
    let suffix = asin.map(|a| format!(" [{a}]")).unwrap_or_default();
    match layout {
        AudioLayout::Libation => {
            let short = title.split(':').next().unwrap_or(title);
            format!("{}{suffix}", sanitize(short, 80, "Untitled"))
        }
        AudioLayout::AuthorTitle => {
            let author = authors.first().map(String::as_str).unwrap_or("Unknown");
            format!(
                "{}/{}{suffix}",
                sanitize(author, 60, "Unknown"),
                sanitize(title, 80, "Untitled")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        let authors = vec!["Frank Herbert".to_string(), "Other".to_string()];
        assert_eq!(
            book_dir(AudioLayout::AuthorTitle, "Dune: Book One", &authors, Some("B002V1A0WE")),
            "Frank Herbert/Dune_ Book One [B002V1A0WE]"
        );
        assert_eq!(
            book_dir(AudioLayout::Libation, "Dune: Book One", &authors, Some("B002V1A0WE")),
            "Dune [B002V1A0WE]"
        );
        assert_eq!(book_dir(AudioLayout::AuthorTitle, " ", &[], None), "Unknown/Untitled");
    }

    #[test]
    fn names_are_windows_safe() {
        assert_eq!(sanitize("a/b\\c?d*", 50, "x"), "a_b_c_d_");
        assert_eq!(sanitize("trailing. ", 50, "x"), "trailing");
        assert_eq!(sanitize("CON", 50, "x"), "_CON");
        assert_eq!(sanitize("abcdef", 3, "x"), "abc");
    }
}
