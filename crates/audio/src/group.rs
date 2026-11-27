use std::collections::{BTreeMap, HashSet};

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
    pub duplicates: Vec<Entry>,
}

impl Group {
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.audio.iter().chain(self.sidecars.iter())
    }

    fn sidecar(&self, kind: Kind) -> Option<&Entry> {
        let all = || self.sidecars.iter().filter(move |e| e.kind == kind);
        let stem = self.audio.first().map(stem_of);
        all()
            .find(|e| {
                let name = file_name(&e.rel).to_lowercase();
                kind == Kind::Cover && (name.starts_with("cover.") || name.starts_with("folder."))
            })
            .or_else(|| all().find(|e| Some(stem_of(e)) == stem))
            .or_else(|| all().next())
    }

    pub fn cover(&self) -> Option<&Entry> {
        self.sidecar(Kind::Cover)
    }

    pub fn meta(&self) -> Option<&Entry> {
        self.sidecar(Kind::Meta)
    }

    pub fn cue(&self) -> Option<&Entry> {
        self.sidecar(Kind::Cue)
    }

    pub fn single(&self, index: usize) -> Group {
        let a = self.audio[index].clone();
        let stem = stem_of(&a).to_string();
        let mut sidecars: Vec<Entry> = self
            .sidecars
            .iter()
            .filter(|s| stem_of(s) == stem)
            .cloned()
            .collect();
        if index == 0 {
            let claimed = |s: &Entry| self.audio.iter().any(|a| stem_of(a) == stem_of(s));
            sidecars.extend(self.sidecars.iter().filter(|s| !claimed(s)).cloned());
        }
        Group {
            key: rel_stem(&a),
            multi: false,
            asin: asin_in(&stem),
            audio: vec![a],
            sidecars,
            duplicates: if index == 0 {
                self.duplicates.clone()
            } else {
                Vec::new()
            },
        }
    }

    pub fn names_have_asin(&self) -> bool {
        self.audio.iter().all(|a| asin_in(stem_of(a)).is_some())
    }
}

fn whole_book_asin(e: &Entry) -> Option<String> {
    stem_asin(stem_of(e))
}

pub(crate) fn stem_asin(stem: &str) -> Option<String> {
    let stem = stem.trim_end();
    let asin = asin_in(stem)?;
    stem.to_uppercase()
        .ends_with(&format!("[{asin}]"))
        .then_some(asin)
}

fn drop_duplicates(audio: &mut Vec<Entry>, picks: &HashSet<String>) -> Vec<(String, Entry)> {
    let asins: Vec<Option<String>> = audio.iter().map(whole_book_asin).collect();
    let mut best: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, asin) in asins.iter().enumerate() {
        let Some(asin) = asin.as_deref() else { continue };
        let rank = |i: usize| (picks.contains(&audio[i].rel), audio[i].mtime, audio[i].size);
        match best.get(asin) {
            Some(&b) if rank(b) >= rank(i) => {}
            _ => {
                best.insert(asin, i);
            }
        }
    }
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (i, e) in audio.drain(..).enumerate() {
        match asins[i].as_deref() {
            Some(asin) if best.get(asin) != Some(&i) => dropped.push((asin.to_string(), e)),
            _ => kept.push(e),
        }
    }
    *audio = kept;
    dropped
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
    group_with(entries, &HashSet::new())
}

pub fn group_with(entries: &[Entry], picks: &HashSet<String>) -> Vec<Group> {
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
        let dropped = drop_duplicates(&mut audio, picks);
        let dups_for = |g: &[Entry]| -> Vec<Entry> {
            dropped
                .iter()
                .filter(|(asin, _)| g.iter().any(|a| whole_book_asin(a).as_ref() == Some(asin)))
                .map(|(_, e)| e.clone())
                .collect()
        };
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
                duplicates: dups_for(&audio),
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
                duplicates: dups_for(std::slice::from_ref(&a)),
                audio: vec![a],
                sidecars: own,
            });
        }
    }
    out
}
