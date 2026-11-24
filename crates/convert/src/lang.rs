use crate::langtable::LANGUAGES;

const ISO3: &[(&str, &str)] = &[
    ("afr", "af"),
    ("ara", "ar"),
    ("bul", "bg"),
    ("cat", "ca"),
    ("ces", "cs"),
    ("cze", "cs"),
    ("dan", "da"),
    ("deu", "de"),
    ("ger", "de"),
    ("ell", "el"),
    ("gre", "el"),
    ("eng", "en"),
    ("spa", "es"),
    ("est", "et"),
    ("eus", "eu"),
    ("baq", "eu"),
    ("fas", "fa"),
    ("per", "fa"),
    ("fin", "fi"),
    ("fra", "fr"),
    ("fre", "fr"),
    ("heb", "he"),
    ("hin", "hi"),
    ("hrv", "hr"),
    ("hun", "hu"),
    ("hye", "hy"),
    ("arm", "hy"),
    ("ind", "id"),
    ("isl", "is"),
    ("ice", "is"),
    ("ita", "it"),
    ("jpn", "ja"),
    ("kat", "ka"),
    ("geo", "ka"),
    ("kor", "ko"),
    ("lit", "lt"),
    ("lav", "lv"),
    ("mkd", "mk"),
    ("mac", "mk"),
    ("msa", "ms"),
    ("may", "ms"),
    ("mlt", "mt"),
    ("nld", "nl"),
    ("dut", "nl"),
    ("nor", "no"),
    ("nob", "no"),
    ("nno", "no"),
    ("pol", "pl"),
    ("por", "pt"),
    ("ron", "ro"),
    ("rum", "ro"),
    ("rus", "ru"),
    ("slk", "sk"),
    ("slo", "sk"),
    ("slv", "sl"),
    ("sqi", "sq"),
    ("alb", "sq"),
    ("srp", "sr"),
    ("swe", "sv"),
    ("swa", "sw"),
    ("tam", "ta"),
    ("tha", "th"),
    ("tur", "tr"),
    ("ukr", "uk"),
    ("urd", "ur"),
    ("vie", "vi"),
    ("zho", "zh"),
    ("chi", "zh"),
];

pub fn iso639_1(code: &str) -> Option<String> {
    let lower = code.to_ascii_lowercase();
    if lower.len() == 2 {
        return Some(lower);
    }
    ISO3.iter()
        .find(|(l3, _)| *l3 == lower)
        .map(|(_, l2)| l2.to_string())
}

pub fn exth_language(code: &str) -> String {
    if code.contains('-') || code.len() == 2 {
        return code.to_string();
    }
    iso639_1(code).unwrap_or_else(|| code.to_string())
}

pub fn mobi_locale(code: Option<&str>) -> u32 {
    let mut lang = 0u8;
    let mut region = 0u8;
    if let Some(code) = code {
        let mut subtags = code.split(['-', '_']);
        let primary = subtags.next().and_then(iso639_1);
        if let Some(primary) = primary {
            if let Some(&(_, _, l, s)) = LANGUAGES
                .iter()
                .find(|(p, sub, _, _)| *p == primary && sub.is_empty())
            {
                lang = l;
                region = s;
            }
            for sub in subtags {
                let upper = sub.to_ascii_uppercase();
                if let Some(&(_, _, l, s)) = LANGUAGES
                    .iter()
                    .find(|(p, r, _, _)| *p == primary && *r == upper)
                {
                    lang = l;
                    region = s;
                    break;
                }
            }
        }
    }
    (region as u32) << 8 | lang as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_codes() {
        assert_eq!(mobi_locale(Some("en")), 9);
        assert_eq!(mobi_locale(Some("en-US")), 4 << 8 | 9);
        assert_eq!(mobi_locale(Some("de-CH")), 8 << 8 | 7);
        assert_eq!(mobi_locale(Some("fra")), 12);
        assert_eq!(mobi_locale(Some("xx")), 0);
        assert_eq!(mobi_locale(None), 0);
    }
}
