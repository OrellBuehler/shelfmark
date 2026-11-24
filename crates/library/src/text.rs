use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

const QUOTE_PAIRS: &[(char, &[char])] = &[
    ('"', &['"']),
    ('\'', &['\'']),
    ('“', &['”', '“']),
    ('”', &['”', '”']),
    ('„', &['”', '“']),
    ('‚', &['’', '‘']),
    ('’', &['’', '‘']),
    ('‘', &['’', '‘']),
    ('‹', &['›']),
    ('›', &['‹']),
    ('《', &['》']),
    ('〈', &['〉']),
    ('»', &['«', '»']),
    ('«', &['«', '»']),
    ('「', &['」']),
    ('『', &['』']),
];

const ARTICLES: &[(&str, &[&str])] = &[
    ("eng", &[r"A\s+", r"The\s+", r"An\s+"]),
    ("epo", &[r"La\s+", "L'", "L´"]),
    (
        "spa",
        &[
            r"El\s+", r"La\s+", r"Lo\s+", r"Los\s+", r"Las\s+", r"Un\s+", r"Una\s+", r"Unos\s+",
            r"Unas\s+",
        ],
    ),
    (
        "fra",
        &[
            r"Le\s+",
            r"La\s+",
            "L'",
            "L´",
            "L’",
            r"Les\s+",
            r"Un\s+",
            r"Une\s+",
            r"Des\s+",
            r"De\s+(?:La\s+)?",
            "D'",
            "D´",
            "D’",
        ],
    ),
    ("pol", &[]),
    (
        "ita",
        &[
            r"Lo\s+",
            r"Il\s+",
            "L'",
            "L´",
            r"La\s+",
            r"Gli\s+",
            r"I\s+",
            r"Le\s+",
            r"Uno\s+",
            r"Un\s+",
            r"Una\s+",
            "Un'",
            "Un´",
            r"Dei\s+",
            r"Degli\s+",
            r"Delle\s+",
            r"Del\s+",
            r"Della\s+",
            r"Dello\s+",
            "Dell'",
            "Dell´",
        ],
    ),
    (
        "por",
        &[
            r"A\s+", r"O\s+", r"Os\s+", r"As\s+", r"Um\s+", r"Uns\s+", r"Uma\s+", r"Umas\s+",
        ],
    ),
    ("ron", &[r"Un\s+", r"O\s+", r"Nişte\s+"]),
    (
        "deu",
        &[
            r"Der\s+",
            r"Die\s+",
            r"Das\s+",
            r"Den\s+",
            r"Ein\s+",
            r"Eine\s+",
            r"Einen\s+",
            r"Dem\s+",
            r"Des\s+",
            r"Einem\s+",
            r"Eines\s+",
        ],
    ),
    (
        "nld",
        &[
            r"De\s+", r"Het\s+", r"Een\s+", r"'n\s+", r"'s\s+", r"Ene\s+", r"Ener\s+", r"Enes\s+",
            r"Den\s+", r"Der\s+", r"Des\s+", r"'t\s+",
        ],
    ),
    (
        "swe",
        &[r"En\s+", r"Ett\s+", r"Det\s+", r"Den\s+", r"De\s+"],
    ),
    ("tur", &[r"Bir\s+"]),
    ("afr", &[r"'n\s+", r"Die\s+"]),
    (
        "ell",
        &[
            r"O\s+",
            r"I\s+",
            r"To\s+",
            r"Ta\s+",
            r"Tus\s+",
            r"Tis\s+",
            r"'Enas\s+",
            r"'Mia\s+",
            r"'Ena\s+",
            r"'Enan\s+",
        ],
    ),
    ("hun", &[r"A\s+", r"Az\s+", r"Egy\s+"]),
];

fn article_patterns() -> &'static HashMap<&'static str, Option<Regex>> {
    static PATS: OnceLock<HashMap<&'static str, Option<Regex>>> = OnceLock::new();
    PATS.get_or_init(|| {
        ARTICLES
            .iter()
            .map(|(lang, pats)| {
                let re = if pats.is_empty() {
                    None
                } else {
                    Some(Regex::new(&format!("(?i)^({})", pats.join("|"))).unwrap())
                };
                (*lang, re)
            })
            .collect()
    })
}

pub fn title_sort(title: &str, lang: Option<&str>) -> String {
    let mut title = title.trim().to_string();
    if let Some(first) = title.chars().next()
        && let Some((_, closers)) = QUOTE_PAIRS.iter().find(|(q, _)| *q == first)
    {
        title.remove(0);
        if let Some(last) = title.chars().last()
            && closers.contains(&last)
        {
            title.pop();
        }
    }
    let code = lang.and_then(crate::langs::canonicalize);
    let pats = article_patterns();
    let re = match code.as_deref().and_then(|c| pats.get(c)) {
        Some(re) => re,
        None => &pats["eng"],
    };
    if let Some(re) = re
        && let Some(m) = re.find(&title)
    {
        let prep = m.as_str().to_string();
        if !prep.is_empty() {
            title = format!("{}, {}", &title[prep.len()..], prep);
            if let Some(first) = title.chars().next()
                && let Some((_, closers)) = QUOTE_PAIRS.iter().find(|(q, _)| *q == first)
            {
                title.remove(0);
                if let Some(last) = title.chars().last()
                    && closers.contains(&last)
                {
                    title.pop();
                }
            }
        }
    }
    title.trim().to_string()
}

const COPY_WORDS: &[&str] = &[
    "agency",
    "corporation",
    "company",
    "co.",
    "council",
    "committee",
    "inc.",
    "institute",
    "national",
    "society",
    "club",
    "team",
    "software",
    "games",
    "entertainment",
    "media",
    "studios",
];
const NAME_PREFIXES: &[&str] = &["mr", "mrs", "ms", "dr", "prof"];
const NAME_SUFFIXES: &[&str] = &[
    "jr", "sr", "inc", "ph.d", "phd", "md", "m.d", "i", "ii", "iii", "iv", "junior", "senior",
];

fn remove_bracketed(src: &str) -> String {
    let mut counts: HashMap<char, i32> = HashMap::new();
    let mut total = 0;
    let mut out = String::new();
    for c in src.chars() {
        if matches!(c, '(' | '[' | '{') {
            *counts.entry(c).or_default() += 1;
            total += 1;
        } else if matches!(c, ')' | ']' | '}') {
            let open = match c {
                ')' => '(',
                ']' => '[',
                _ => '{',
            };
            let n = counts.entry(open).or_default();
            if *n > 0 {
                *n -= 1;
                total -= 1;
            }
        } else if total < 1 {
            out.push(c);
        }
    }
    out
}

pub fn author_to_author_sort(author: &str) -> String {
    if author.is_empty() {
        return String::new();
    }
    let sauthor = remove_bracketed(author);
    let sauthor = sauthor.trim();
    if sauthor.contains(',') {
        return author.to_string();
    }
    let tokens: Vec<&str> = sauthor.split_whitespace().collect();
    if tokens.len() < 2 {
        return author.to_string();
    }
    if tokens
        .iter()
        .any(|t| COPY_WORDS.contains(&t.to_lowercase().as_str()))
    {
        return author.to_string();
    }
    let is_prefix = |t: &str| {
        let l = t.to_lowercase();
        NAME_PREFIXES
            .iter()
            .any(|p| l == *p || l == format!("{p}."))
    };
    let is_suffix = |t: &str| {
        let l = t.to_lowercase();
        NAME_SUFFIXES
            .iter()
            .any(|p| l == *p || l == format!("{p}."))
    };
    let Some(first) = (0..tokens.len()).find(|&i| !is_prefix(tokens[i])) else {
        return author.to_string();
    };
    let Some(last) = (first..tokens.len()).rev().find(|&i| !is_suffix(tokens[i])) else {
        return author.to_string();
    };
    let suffix = tokens[last + 1..].join(" ");
    let mut atokens: Vec<String> = Vec::new();
    atokens.push(tokens[last].to_string());
    atokens.extend(tokens[first..last].iter().map(|s| s.to_string()));
    let num_toks = atokens.len();
    if !suffix.is_empty() {
        atokens.push(suffix);
    }
    if num_toks > 1 {
        atokens[0].push(',');
    }
    atokens.join(" ")
}

fn is_invalid_filename_char(c: char) -> bool {
    matches!(
        c,
        '\\' | '|' | '?' | '*' | '<' | '"' | ':' | '>' | '+' | '/'
    ) || (c as u32) < 32
}

fn splitext(name: &str) -> (&str, &str) {
    let Some(dot) = name.rfind('.') else {
        return (name, "");
    };
    if name[..dot].chars().any(|c| c != '.') {
        (&name[..dot], &name[dot..])
    } else {
        (name, "")
    }
}

pub fn sanitize_file_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| if is_invalid_filename_char(c) { '_' } else { c })
        .collect();
    let spaced: String = replaced
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let one = spaced.trim();
    let (bname, ext) = splitext(one);
    let mut bname = if !bname.is_empty() && bname.chars().all(|c| c == '.') {
        "_".to_string()
    } else {
        bname.to_string()
    };
    bname = bname.replace("..", "_");
    let mut one = format!("{bname}{ext}");
    if one.ends_with('.') || one.ends_with(' ') {
        one.pop();
        one.push('_');
    }
    if let Some(rest) = one.strip_prefix('.') {
        one = format!("_{rest}");
    }
    one
}

pub fn ascii_filename(orig: &str) -> String {
    let ascii = deunicode::deunicode(orig).replace('?', "_");
    let ans: String = ascii
        .chars()
        .map(|c| if (c as u32) < 32 { '_' } else { c })
        .collect();
    sanitize_file_name(&ans)
}

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Clone, Copy, Debug)]
pub struct PathRules {
    pub limit: usize,
    pub windows: bool,
}

impl PathRules {
    pub const WINDOWS: PathRules = PathRules {
        limit: 40,
        windows: true,
    };
    pub const UNIX: PathRules = PathRules {
        limit: 100,
        windows: false,
    };
}

impl Default for PathRules {
    fn default() -> Self {
        if cfg!(windows) {
            Self::WINDOWS
        } else {
            Self::UNIX
        }
    }
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn construct_path_name(limit: usize, book_id: i64, title: &str, author: &str) -> String {
    let id_part = format!(" ({book_id})");
    let l = limit.saturating_sub(id_part.chars().count() / 2 + 2);
    let mut author = truncate_chars(&ascii_filename(author), l);
    let mut title = truncate_chars(&ascii_filename(title.trim_start()), l)
        .trim_end()
        .to_string();
    if title.is_empty() {
        title = truncate_chars("Unknown", l);
    }
    while author.ends_with(' ') || author.ends_with('.') {
        author.pop();
    }
    if author.is_empty() {
        author = ascii_filename("Unknown");
    }
    if WINDOWS_RESERVED.contains(&author.to_uppercase().as_str()) {
        author.push('w');
    }
    format!("{author}/{title}{id_part}")
}

pub fn construct_file_name(rules: PathRules, title: &str, author: &str, extlen: usize) -> String {
    let extlen = extlen.max(14);
    let limit = rules.limit;
    let l = if rules.windows {
        limit.saturating_sub(extlen / 2 + 2)
    } else {
        limit.saturating_sub(extlen + 2) / 2
    };
    let author = truncate_chars(&ascii_filename(author), l);
    let mut title = truncate_chars(&ascii_filename(title.trim_start()), l)
        .trim_end()
        .to_string();
    if title.is_empty() {
        title = truncate_chars("Unknown", l);
    }
    let mut name = format!("{title} - {author}");
    while name.ends_with('.') {
        name.pop();
    }
    if name.is_empty() {
        name = ascii_filename("Unknown");
    }
    name
}

fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn is_ignorable(c: char) -> bool {
    if c.is_whitespace() || c.is_control() {
        return true;
    }
    if c.is_ascii() {
        return c.is_ascii_punctuation()
            && !matches!(c, '$' | '+' | '<' | '=' | '>' | '^' | '`' | '|' | '~');
    }
    matches!(c as u32, 0x00A1 | 0x00A7 | 0x00AB | 0x00B6 | 0x00B7 | 0x00BB | 0x00BF | 0x2010..=0x2027 | 0x2030..=0x205E | 0x3001..=0x3003 | 0x300C..=0x3011)
}

fn fold_char(c: char, out: &mut String) {
    match c {
        'ß' => out.push_str("ss"),
        'æ' => out.push_str("ae"),
        'œ' => out.push_str("oe"),
        'ø' => out.push('o'),
        'đ' => out.push('d'),
        'ł' => out.push('l'),
        'ı' => out.push('i'),
        _ => out.push(c),
    }
}

pub fn primary(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.nfkd() {
        if is_combining(c) {
            continue;
        }
        for l in c.to_lowercase() {
            fold_char(l, &mut out);
        }
    }
    out
}

pub fn primary_no_punc(s: &str) -> String {
    if s.is_ascii() {
        return s
            .chars()
            .filter(|c| !is_ignorable(*c))
            .map(|c| c.to_ascii_lowercase())
            .collect();
    }
    primary(s).chars().filter(|c| !is_ignorable(*c)).collect()
}

pub fn sort_key(s: &str) -> String {
    primary(s)
}

pub fn normalize_whitespace(s: &str) -> String {
    s.split(|c: char| c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{2007}' | '\u{202f}'))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn long_text(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles() {
        assert_eq!(title_sort("The Selfish Gene", None), "Selfish Gene, The");
        assert_eq!(
            title_sort("A Tale of Two Cities", None),
            "Tale of Two Cities, A"
        );
        assert_eq!(
            title_sort("Theory of Everything", None),
            "Theory of Everything"
        );
        assert_eq!(title_sort("Die Wahrheit", None), "Die Wahrheit");
        assert_eq!(title_sort("Die Wahrheit", Some("deu")), "Wahrheit, Die");
        assert_eq!(title_sort("  \"The Quote\"  ", None), "Quote, The");
        assert_eq!(title_sort("Les Misérables", Some("fra")), "Misérables, Les");
        assert_eq!(title_sort("Polski", Some("pol")), "Polski");
        assert_eq!(title_sort("", None), "");
    }

    #[test]
    fn authors() {
        assert_eq!(author_to_author_sort("Jane Doe"), "Doe, Jane");
        assert_eq!(author_to_author_sort("Doe, Jane"), "Doe, Jane");
        assert_eq!(author_to_author_sort("Madonna"), "Madonna");
        assert_eq!(author_to_author_sort("Dr. John Smith"), "Smith, John");
        assert_eq!(author_to_author_sort("John Smith Jr."), "Smith, John Jr.");
        assert_eq!(
            author_to_author_sort("Acme Software Inc"),
            "Acme Software Inc"
        );
        assert_eq!(
            author_to_author_sort("Ursula K. Le Guin"),
            "Guin, Ursula K. Le"
        );
        assert_eq!(author_to_author_sort("Jane (the Great) Doe"), "Doe, Jane");
    }

    #[test]
    fn filenames() {
        assert_eq!(ascii_filename("Joël Dicker"), "Joel Dicker");
        assert_eq!(ascii_filename("a/b:c?d"), "a_b_c_d");
        assert_eq!(ascii_filename("..."), "_");
        assert_eq!(ascii_filename("Hello."), "Hello_");
        assert_eq!(ascii_filename(".hidden"), "_hidden");
        assert_eq!(
            construct_path_name(100, 5, "The Title: Sub", "Jane Doe"),
            "Jane Doe/The Title_ Sub (5)"
        );
        assert_eq!(
            construct_path_name(
                40,
                48,
                "Developing Apps With GPT-4 and ChatGPT and more",
                "Olivier Caelen"
            ),
            "Olivier Caelen/Developing Apps With GPT-4 and ChatG (48)"
        );
        assert_eq!(construct_path_name(100, 1, "", ""), "Unknown/Unknown (1)");
        assert_eq!(construct_path_name(100, 1, "T", "CON"), "CONw/T (1)");
        assert_eq!(
            construct_file_name(PathRules::UNIX, "Title", "Jane Doe", 5),
            "Title - Jane Doe"
        );
        assert_eq!(
            construct_file_name(
                PathRules {
                    limit: 40,
                    windows: false
                },
                "Completely Different Title",
                "Zora Test",
                5
            ),
            "Completely D - Zora Test"
        );
        assert_eq!(
            construct_file_name(
                PathRules::WINDOWS,
                "Completely Different Title",
                "Zora Test",
                5
            ),
            "Completely Different Title - Zora Test"
        );
    }

    #[test]
    fn folding() {
        assert_eq!(
            primary_no_punc("Joël D'Artagnan-Müller"),
            "joeldartagnanmuller"
        );
        assert_eq!(primary("Straße"), "strasse");
        assert_eq!(normalize_whitespace("  a \t b\n c "), "a b c");
    }
}
