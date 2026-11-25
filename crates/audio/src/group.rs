use std::collections::BTreeMap;

use crate::names::{
    asin_in, common_prefix, file_name, leading_number, number_after, parent, split_ext,
};
use crate::walk::{Entry, Kind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub key: String,
    pub multi: bool,
    pub asin: Option<String>,
    pub audio: Vec<Entry>,
    pub sidecars: Vec<Entry>,
}

impl Group {
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.audio.iter().chain(self.sidecars.iter())
    }

    pub fn cover(&self) -> Option<&Entry> {
        let covers = || self.sidecars.iter().filter(|e| e.kind == Kind::Cover);
        covers()
            .find(|e| {
                let name = file_name(&e.rel).to_lowercase();
                name.starts_with("cover.") || name.starts_with("folder.")
            })
            .or_else(|| covers().next())
    }
}

fn ext_rank(ext: &str) -> usize {
    ["m4b", "m4a", "mp3", "opus", "ogg", "flac"]
        .iter()
        .position(|e| *e == ext)
        .unwrap_or(9)
}

fn stem_of(e: &Entry) -> &str {
    split_ext(file_name(&e.rel)).0
}

fn rel_stem(e: &Entry) -> String {
    let dir = parent(&e.rel);
    let stem = stem_of(e);
    if dir.is_empty() {
        stem.to_string()
    } else {
        format!("{dir}/{stem}")
    }
}

fn is_one_book(dir: &str, audio: &[Entry]) -> bool {
    if audio.len() < 2 {
        return false;
    }
    let stems: Vec<&str> = audio.iter().map(stem_of).collect();
    let asins: Vec<Option<String>> = stems.iter().map(|s| asin_in(s)).collect();
    if asins.iter().all(Option::is_some) {
        return asins.iter().all(|a| a == &asins[0]);
    }
    if asins.iter().any(Option::is_some) {
        return false;
    }
    if stems.iter().all(|s| leading_number(s).is_some()) {
        return true;
    }
    let prefix = common_prefix(&stems);
    if prefix > 0 && stems.iter().all(|s| number_after(s, prefix).is_some()) {
        return true;
    }
    let lossless_container = audio
        .iter()
        .all(|e| matches!(split_ext(file_name(&e.rel)).1.as_str(), "m4b" | "m4a"));
    !lossless_container && !dir.is_empty()
}

pub fn group(entries: &[Entry]) -> Vec<Group> {
    let mut dirs: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        dirs.entry(parent(&e.rel)).or_default().push(e);
    }
    let mut out = Vec::new();
    for (dir, files) in dirs {
        let mut by_stem: BTreeMap<String, &Entry> = BTreeMap::new();
        for e in files.iter().filter(|e| e.kind == Kind::Audio) {
            let (stem, ext) = split_ext(file_name(&e.rel));
            match by_stem.get(stem) {
                Some(old) if ext_rank(&split_ext(file_name(&old.rel)).1) <= ext_rank(&ext) => {}
                _ => {
                    by_stem.insert(stem.to_string(), e);
                }
            }
        }
        let mut audio: Vec<Entry> = by_stem.into_values().cloned().collect();
        audio.sort_by(|a, b| a.rel.cmp(&b.rel));
        if audio.is_empty() {
            continue;
        }
        let sidecars: Vec<&Entry> = files
            .iter()
            .copied()
            .filter(|e| e.kind != Kind::Audio)
            .collect();
        if is_one_book(dir, &audio) {
            let asin = asin_in(stem_of(&audio[0])).or_else(|| asin_in(file_name(dir)));
            out.push(Group {
                key: dir.to_string(),
                multi: true,
                asin,
                audio,
                sidecars: sidecars.into_iter().cloned().collect(),
            });
            continue;
        }
        let alone = audio.len() == 1;
        for a in audio {
            let stem = stem_of(&a).to_string();
            let own: Vec<Entry> = sidecars
                .iter()
                .filter(|s| {
                    let (s_stem, _) = split_ext(file_name(&s.rel));
                    s_stem == stem || (alone && !dir.is_empty())
                })
                .map(|s| (*s).clone())
                .collect();
            out.push(Group {
                key: rel_stem(&a),
                multi: false,
                asin: asin_in(&stem).or_else(|| {
                    alone
                        .then(|| asin_in(file_name(dir)))
                        .flatten()
                }),
                audio: vec![a],
                sidecars: own,
            });
        }
    }
    out
}
