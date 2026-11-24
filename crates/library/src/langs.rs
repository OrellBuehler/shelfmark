const LANGS: &[(&str, &str, &str, &[&str])] = &[
    ("eng", "en", "English", &["englisch"]),
    ("deu", "de", "German", &["deutsch"]),
    (
        "fra",
        "fr",
        "French",
        &["francais", "französisch", "franzosisch"],
    ),
    ("spa", "es", "Spanish", &["espanol", "spanisch"]),
    ("ita", "it", "Italian", &["italiano", "italienisch"]),
    ("por", "pt", "Portuguese", &["portugues", "portugiesisch"]),
    (
        "nld",
        "nl",
        "Dutch",
        &["nederlands", "niederländisch", "niederlandisch"],
    ),
    ("swe", "sv", "Swedish", &["svenska", "schwedisch"]),
    ("nor", "no", "Norwegian", &["norsk", "norwegisch"]),
    ("dan", "da", "Danish", &["dansk", "dänisch", "danisch"]),
    ("fin", "fi", "Finnish", &["suomi", "finnisch"]),
    ("pol", "pl", "Polish", &["polski", "polnisch"]),
    ("ces", "cs", "Czech", &["cestina", "tschechisch"]),
    ("slk", "sk", "Slovak", &["slovencina", "slowakisch"]),
    ("hun", "hu", "Hungarian", &["magyar", "ungarisch"]),
    ("ron", "ro", "Romanian", &["rumänisch", "rumanisch"]),
    ("bul", "bg", "Bulgarian", &["bulgarisch"]),
    ("rus", "ru", "Russian", &["russisch"]),
    ("ukr", "uk", "Ukrainian", &["ukrainisch"]),
    ("ell", "el", "Greek", &["griechisch"]),
    ("tur", "tr", "Turkish", &["türkisch", "turkisch"]),
    ("heb", "he", "Hebrew", &["hebräisch", "hebraisch"]),
    ("ara", "ar", "Arabic", &["arabisch"]),
    ("fas", "fa", "Persian", &["farsi", "persisch"]),
    ("hin", "hi", "Hindi", &[]),
    ("ben", "bn", "Bengali", &[]),
    ("jpn", "ja", "Japanese", &["japanisch"]),
    ("zho", "zh", "Chinese", &["chinesisch"]),
    ("kor", "ko", "Korean", &["koreanisch"]),
    ("vie", "vi", "Vietnamese", &["vietnamesisch"]),
    ("tha", "th", "Thai", &[]),
    ("ind", "id", "Indonesian", &["indonesisch"]),
    ("msa", "ms", "Malay", &[]),
    ("cat", "ca", "Catalan", &["katalanisch"]),
    ("eus", "eu", "Basque", &["baskisch"]),
    ("glg", "gl", "Galician", &["galicisch"]),
    ("hrv", "hr", "Croatian", &["kroatisch"]),
    ("srp", "sr", "Serbian", &["serbisch"]),
    ("slv", "sl", "Slovenian", &["slowenisch"]),
    ("lit", "lt", "Lithuanian", &["litauisch"]),
    ("lav", "lv", "Latvian", &["lettisch"]),
    ("est", "et", "Estonian", &["estnisch"]),
    ("isl", "is", "Icelandic", &["isländisch", "islandisch"]),
    ("gle", "ga", "Irish", &["irisch"]),
    ("lat", "la", "Latin", &["latein", "lateinisch"]),
    ("epo", "eo", "Esperanto", &[]),
    ("afr", "af", "Afrikaans", &[]),
    ("sqi", "sq", "Albanian", &["albanisch"]),
    ("mkd", "mk", "Macedonian", &["mazedonisch"]),
    ("bos", "bs", "Bosnian", &["bosnisch"]),
    ("bel", "be", "Belarusian", &["weissrussisch"]),
    ("kat", "ka", "Georgian", &["georgisch"]),
    ("hye", "hy", "Armenian", &["armenisch"]),
    ("cym", "cy", "Welsh", &["walisisch"]),
    ("mya", "my", "Burmese", &[]),
    ("swa", "sw", "Swahili", &[]),
    ("tam", "ta", "Tamil", &[]),
    ("urd", "ur", "Urdu", &[]),
    ("yid", "yi", "Yiddish", &["jiddisch"]),
];

const BIBLIOGRAPHIC: &[(&str, &str)] = &[
    ("ger", "deu"),
    ("fre", "fra"),
    ("chi", "zho"),
    ("cze", "ces"),
    ("gre", "ell"),
    ("per", "fas"),
    ("ice", "isl"),
    ("mac", "mkd"),
    ("may", "msa"),
    ("dut", "nld"),
    ("rum", "ron"),
    ("slo", "slk"),
    ("alb", "sqi"),
    ("arm", "hye"),
    ("geo", "kat"),
    ("baq", "eus"),
    ("wel", "cym"),
    ("bur", "mya"),
];

pub fn canonicalize(s: &str) -> Option<String> {
    let q = s.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let q = q.replace('_', "-");
    let base = q.split('-').next().unwrap_or(&q);
    for (c3, c2, name, others) in LANGS {
        if base == *c3 || base == *c2 || q == name.to_lowercase() || others.contains(&q.as_str()) {
            return Some((*c3).to_string());
        }
    }
    BIBLIOGRAPHIC
        .iter()
        .find(|(b, _)| *b == base)
        .map(|(_, c)| (*c).to_string())
}

pub fn name(code: &str) -> Option<&'static str> {
    LANGS
        .iter()
        .find(|(c3, ..)| *c3 == code)
        .map(|(_, _, n, _)| *n)
}

pub fn clean_code(s: &str) -> Option<String> {
    if let Some(c) = canonicalize(s) {
        return Some(c);
    }
    let q = s.trim().to_lowercase();
    if q.len() == 3
        && q.chars().all(|c| c.is_ascii_lowercase())
        && !matches!(q.as_str(), "und" | "zxx" | "mis" | "mul")
    {
        return Some(q);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical() {
        assert_eq!(canonicalize("en").as_deref(), Some("eng"));
        assert_eq!(canonicalize("German").as_deref(), Some("deu"));
        assert_eq!(canonicalize("deutsch").as_deref(), Some("deu"));
        assert_eq!(canonicalize("ger").as_deref(), Some("deu"));
        assert_eq!(canonicalize("de_CH").as_deref(), Some("deu"));
        assert_eq!(canonicalize("xx"), None);
        assert_eq!(clean_code("und"), None);
        assert_eq!(clean_code("qqq").as_deref(), Some("qqq"));
    }
}
