const TABLE: &[(&str, &str)] = &[
    ("af", "afr"),
    ("sq", "sqi"),
    ("ar", "ara"),
    ("hy", "hye"),
    ("az", "aze"),
    ("eu", "eus"),
    ("be", "bel"),
    ("bn", "ben"),
    ("bs", "bos"),
    ("br", "bre"),
    ("bg", "bul"),
    ("my", "mya"),
    ("ca", "cat"),
    ("zh", "zho"),
    ("hr", "hrv"),
    ("cs", "ces"),
    ("da", "dan"),
    ("nl", "nld"),
    ("en", "eng"),
    ("eo", "epo"),
    ("et", "est"),
    ("fi", "fin"),
    ("fr", "fra"),
    ("gl", "glg"),
    ("ka", "kat"),
    ("de", "deu"),
    ("el", "ell"),
    ("gu", "guj"),
    ("he", "heb"),
    ("hi", "hin"),
    ("hu", "hun"),
    ("is", "isl"),
    ("id", "ind"),
    ("ga", "gle"),
    ("gd", "gla"),
    ("it", "ita"),
    ("ja", "jpn"),
    ("kn", "kan"),
    ("kk", "kaz"),
    ("ko", "kor"),
    ("la", "lat"),
    ("lv", "lav"),
    ("lt", "lit"),
    ("lb", "ltz"),
    ("mk", "mkd"),
    ("ms", "msa"),
    ("ml", "mal"),
    ("mt", "mlt"),
    ("mi", "mri"),
    ("mr", "mar"),
    ("ne", "nep"),
    ("no", "nor"),
    ("nb", "nob"),
    ("nn", "nno"),
    ("fa", "fas"),
    ("pl", "pol"),
    ("pt", "por"),
    ("pa", "pan"),
    ("ro", "ron"),
    ("ru", "rus"),
    ("sr", "srp"),
    ("si", "sin"),
    ("sk", "slk"),
    ("sl", "slv"),
    ("es", "spa"),
    ("sw", "swa"),
    ("sv", "swe"),
    ("tl", "tgl"),
    ("ta", "tam"),
    ("te", "tel"),
    ("th", "tha"),
    ("bo", "bod"),
    ("tr", "tur"),
    ("uk", "ukr"),
    ("ur", "urd"),
    ("uz", "uzb"),
    ("vi", "vie"),
    ("cy", "cym"),
    ("yi", "yid"),
];

const MARC_ALIASES: &[(&str, &str)] = &[
    ("alb", "sqi"),
    ("arm", "hye"),
    ("baq", "eus"),
    ("bur", "mya"),
    ("chi", "zho"),
    ("cze", "ces"),
    ("dut", "nld"),
    ("fre", "fra"),
    ("geo", "kat"),
    ("ger", "deu"),
    ("gre", "ell"),
    ("ice", "isl"),
    ("mac", "mkd"),
    ("mao", "mri"),
    ("may", "msa"),
    ("per", "fas"),
    ("rum", "ron"),
    ("slo", "slk"),
    ("tib", "bod"),
    ("wel", "cym"),
    ("scc", "srp"),
    ("scr", "hrv"),
];

pub fn normalize(code: &str) -> Option<String> {
    let lower = code.trim().to_ascii_lowercase();
    let primary = lower.split(['-', '_']).next()?;
    if !primary.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    match primary.len() {
        2 => TABLE
            .iter()
            .find(|(a, _)| *a == primary)
            .map(|(_, b)| b.to_string()),
        3 => Some(
            MARC_ALIASES
                .iter()
                .find(|(a, _)| *a == primary)
                .map_or(primary, |(_, b)| b)
                .to_string(),
        ),
        _ => None,
    }
}

pub fn normalize_all<'a>(codes: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for code in codes {
        if let Some(c) = normalize(code)
            && c != "und"
            && !out.contains(&c)
        {
            out.push(c);
        }
    }
    out
}
