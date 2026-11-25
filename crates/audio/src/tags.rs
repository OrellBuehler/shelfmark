use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::id3::v2::{Frame, FrameId, Id3v2Tag};
use lofty::mp4::{AtomData, AtomIdent, Ilst, Mp4File};
use lofty::mpeg::MpegFile;
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileTags {
    pub title: Option<String>,
    pub album: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub genres: Vec<String>,
    pub description: Option<String>,
    pub asin: Option<String>,
    pub isbn: Option<String>,
    pub date: Option<String>,
    pub track: Option<u32>,
    pub duration_ms: u64,
    pub cover: Option<Vec<u8>>,
}

pub trait TagReader: Sync {
    fn read(&self, path: &Path, want_cover: bool) -> Result<FileTags>;
}

pub struct LoftyReader;

impl TagReader for LoftyReader {
    fn read(&self, path: &Path, want_cover: bool) -> Result<FileTags> {
        read(path, want_cover)
    }
}

fn clean(s: &str) -> Option<String> {
    let t = s.trim().trim_matches('\0').trim();
    (!t.is_empty()).then(|| t.to_string())
}

const NAME_SUFFIXES: &[&str] = &[
    "jr", "jr.", "sr", "sr.", "ii", "iii", "iv", "phd", "ph.d.", "ph.d", "md", "m.d.", "dr.",
    "esq", "esq.",
];

pub fn split_names(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in raw.split([';', ',', '/', '\u{0}']) {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if NAME_SUFFIXES.contains(&p.to_lowercase().as_str())
            && let Some(last) = out.last_mut()
        {
            last.push_str(", ");
            last.push_str(p);
            continue;
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(p)) {
            out.push(p.to_string());
        }
    }
    out
}

fn split_genres(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in raw.split([';', '/', ',', '\u{0}']) {
        let p = part.trim();
        if !p.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(p)) {
            out.push(p.to_string());
        }
    }
    out
}

fn parse_index(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if let Ok(v) = t.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    let start = t.find(|c: char| c.is_ascii_digit())?;
    let rest = &t[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end].trim_end_matches('.').parse::<f64>().ok()
}

pub fn valid_asin(raw: &str) -> Option<String> {
    let t = raw.trim().to_uppercase();
    (t.len() == 10
        && t.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
        && t.bytes().any(|b| b.is_ascii_digit()))
    .then_some(t)
}

fn fourcc(ilst: &Ilst, id: &[u8; 4]) -> Option<String> {
    let atom = ilst.get(&AtomIdent::Fourcc(*id))?;
    atom.data().find_map(|d| match d {
        AtomData::UTF8(s) | AtomData::UTF16(s) => clean(s),
        _ => None,
    })
}

fn freeform(ilst: &Ilst, mean: &str, name: &str) -> Option<String> {
    let atom = ilst.get(&AtomIdent::Freeform {
        mean: mean.into(),
        name: name.into(),
    })?;
    atom.data().find_map(|d| match d {
        AtomData::UTF8(s) | AtomData::UTF16(s) => clean(s),
        AtomData::Unknown { data, .. } => std::str::from_utf8(data).ok().and_then(clean),
        _ => None,
    })
}

fn any_freeform(ilst: &Ilst, name: &str) -> Option<String> {
    ["com.pilabor.tone", "com.apple.iTunes", "org.libation"]
        .iter()
        .find_map(|mean| freeform(ilst, mean, name))
}

fn track_number(ilst: &Ilst) -> Option<u32> {
    let atom = ilst.get(&AtomIdent::Fourcc(*b"trkn"))?;
    atom.data().find_map(|d| match d {
        AtomData::Unknown { data, .. } if data.len() >= 4 => {
            let n = u16::from_be_bytes([data[2], data[3]]);
            (n > 0).then_some(u32::from(n))
        }
        AtomData::UnsignedInteger(n) => Some(*n),
        AtomData::SignedInteger(n) => u32::try_from(*n).ok(),
        _ => None,
    })
}

fn from_ilst(ilst: &Ilst, want_cover: bool, out: &mut FileTags) {
    out.title = fourcc(ilst, b"\xa9nam");
    out.album = fourcc(ilst, b"\xa9alb");
    out.subtitle = any_freeform(ilst, "SUBTITLE");
    let authors = fourcc(ilst, b"\xa9ART").or_else(|| fourcc(ilst, b"aART"));
    out.authors = authors.as_deref().map(split_names).unwrap_or_default();
    let narrators = fourcc(ilst, b"\xa9nrt")
        .or_else(|| any_freeform(ilst, "NARRATOR"))
        .or_else(|| fourcc(ilst, b"\xa9wrt"));
    out.narrators = narrators.as_deref().map(split_names).unwrap_or_default();
    out.series = any_freeform(ilst, "SERIES").or_else(|| fourcc(ilst, b"\xa9grp"));
    out.series_index = any_freeform(ilst, "PART")
        .or_else(|| any_freeform(ilst, "SERIES-PART"))
        .as_deref()
        .and_then(parse_index);
    out.publisher = any_freeform(ilst, "PUBLISHER").or_else(|| fourcc(ilst, b"\xa9pub"));
    out.language = any_freeform(ilst, "LANGUAGE");
    out.genres = fourcc(ilst, b"\xa9gen")
        .as_deref()
        .map(split_genres)
        .unwrap_or_default();
    out.description = fourcc(ilst, b"ldes")
        .or_else(|| fourcc(ilst, b"desc"))
        .or_else(|| fourcc(ilst, b"\xa9cmt"));
    out.asin = fourcc(ilst, b"asin")
        .or_else(|| any_freeform(ilst, "AUDIBLE_ASIN"))
        .or_else(|| any_freeform(ilst, "ASIN"))
        .as_deref()
        .and_then(valid_asin);
    out.isbn = any_freeform(ilst, "ISBN");
    out.date = fourcc(ilst, b"rldt").or_else(|| fourcc(ilst, b"\xa9day"));
    out.track = track_number(ilst);
    if want_cover && let Some(mut pics) = ilst.pictures() {
        out.cover = pics.next().map(|p| p.data().to_vec());
    }
}

fn frame_text(tag: &Id3v2Tag, id: &'static str) -> Option<String> {
    tag.get_text(&FrameId::Valid(id.into())).and_then(clean)
}

fn user_text(tag: &Id3v2Tag, names: &[&str]) -> Option<String> {
    names.iter().find_map(|n| {
        tag.get_user_text(n).and_then(clean).or_else(|| {
            tag.into_iter().find_map(|f| match f {
                Frame::UserText(t) if t.description.eq_ignore_ascii_case(n) => clean(&t.content),
                _ => None,
            })
        })
    })
}

fn from_id3(tag: &Id3v2Tag, want_cover: bool, out: &mut FileTags) {
    out.title = frame_text(tag, "TIT2");
    out.album = frame_text(tag, "TALB");
    out.subtitle = frame_text(tag, "TIT3").or_else(|| user_text(tag, &["SUBTITLE"]));
    let authors = frame_text(tag, "TPE1").or_else(|| frame_text(tag, "TPE2"));
    out.authors = authors.as_deref().map(split_names).unwrap_or_default();
    let narrators = user_text(tag, &["NARRATOR", "NARRATEDBY"]).or_else(|| frame_text(tag, "TCOM"));
    out.narrators = narrators.as_deref().map(split_names).unwrap_or_default();
    out.series = user_text(tag, &["SERIES"]).or_else(|| frame_text(tag, "TIT1"));
    out.series_index = user_text(tag, &["PART", "SERIES-PART"])
        .as_deref()
        .and_then(parse_index);
    out.publisher = frame_text(tag, "TPUB").or_else(|| user_text(tag, &["PUBLISHER"]));
    out.language = user_text(tag, &["LANGUAGE"]).or_else(|| frame_text(tag, "TLAN"));
    out.genres = tag
        .genres()
        .map(|g| {
            g.flat_map(split_genres)
                .fold(Vec::<String>::new(), |mut acc, g| {
                    if !acc.contains(&g) {
                        acc.push(g);
                    }
                    acc
                })
        })
        .unwrap_or_default();
    out.description = tag.comments().find_map(|c| clean(&c.content));
    out.asin = user_text(tag, &["AUDIBLE_ASIN", "ASIN"])
        .as_deref()
        .and_then(valid_asin);
    out.isbn = user_text(tag, &["ISBN"]);
    out.date = frame_text(tag, "TDRL")
        .or_else(|| frame_text(tag, "TDRC"))
        .or_else(|| frame_text(tag, "TYER"));
    out.track = frame_text(tag, "TRCK")
        .and_then(|t| t.split('/').next().and_then(|n| n.trim().parse().ok()));
    if want_cover {
        let mut first = None;
        for f in tag {
            if let Frame::Picture(p) = f {
                if p.picture.pic_type() == PictureType::CoverFront {
                    first = Some(p.picture.data().to_vec());
                    break;
                }
                first.get_or_insert_with(|| p.picture.data().to_vec());
            }
        }
        out.cover = first;
    }
}

fn from_generic(tag: &Tag, want_cover: bool, out: &mut FileTags) {
    let get = |k: ItemKey| tag.get_string(k).and_then(clean);
    out.title = get(ItemKey::TrackTitle);
    out.album = get(ItemKey::AlbumTitle);
    out.subtitle = get(ItemKey::TrackSubtitle);
    let authors = get(ItemKey::TrackArtist).or_else(|| get(ItemKey::AlbumArtist));
    out.authors = authors.as_deref().map(split_names).unwrap_or_default();
    out.narrators = get(ItemKey::Composer)
        .as_deref()
        .map(split_names)
        .unwrap_or_default();
    out.publisher = get(ItemKey::Publisher);
    out.language = get(ItemKey::Language);
    out.genres = get(ItemKey::Genre)
        .as_deref()
        .map(split_genres)
        .unwrap_or_default();
    out.description = get(ItemKey::Description).or_else(|| get(ItemKey::Comment));
    out.date = get(ItemKey::RecordingDate).or_else(|| get(ItemKey::Year));
    out.track = get(ItemKey::TrackNumber).and_then(|t| t.parse().ok());
    if want_cover {
        out.cover = tag
            .get_picture_type(PictureType::CoverFront)
            .or_else(|| tag.pictures().first())
            .map(|p| p.data().to_vec());
    }
}

fn parse_err(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("{}: {e}", path.display()))
}

pub fn read(path: &Path, want_cover: bool) -> Result<FileTags> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let options = ParseOptions::new().read_cover_art(want_cover);
    let mut out = FileTags::default();
    match ext.as_str() {
        "m4b" | "m4a" | "mp4" => {
            let mut reader = BufReader::new(File::open(path)?);
            let file =
                Mp4File::read_from(&mut reader, options).map_err(|e| parse_err(path, e))?;
            out.duration_ms = file.properties().duration().as_millis() as u64;
            if let Some(ilst) = file.ilst() {
                from_ilst(ilst, want_cover, &mut out);
            }
        }
        "mp3" => {
            let mut reader = BufReader::new(File::open(path)?);
            let file =
                MpegFile::read_from(&mut reader, options).map_err(|e| parse_err(path, e))?;
            out.duration_ms = file.properties().duration().as_millis() as u64;
            if let Some(tag) = file.id3v2() {
                from_id3(tag, want_cover, &mut out);
            }
        }
        _ => {
            let file = Probe::open(path)
                .map_err(|e| parse_err(path, e))?
                .options(options)
                .read()
                .map_err(|e| parse_err(path, e))?;
            out.duration_ms = file.properties().duration().as_millis() as u64;
            if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
                from_generic(tag, want_cover, &mut out);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_and_keep_suffixes() {
        assert_eq!(
            split_names("Brandon Sanderson, Michael Kramer; Kate Reading"),
            ["Brandon Sanderson", "Michael Kramer", "Kate Reading"]
        );
        assert_eq!(
            split_names("Martin Luther King, Jr., Someone Else"),
            ["Martin Luther King, Jr.", "Someone Else"]
        );
        assert_eq!(split_names(" A ; a ;"), ["A"]);
    }

    #[test]
    fn asin_shape() {
        assert_eq!(valid_asin(" b002v1a0wE "), Some("B002V1A0WE".into()));
        assert_eq!(valid_asin("UNABRIDGED"), None);
        assert_eq!(valid_asin("B002V1A0W"), None);
        assert_eq!(valid_asin("1234567890"), Some("1234567890".into()));
    }

    #[test]
    fn series_index_parsing() {
        assert_eq!(parse_index("3"), Some(3.0));
        assert_eq!(parse_index("2.5"), Some(2.5));
        assert_eq!(parse_index("Book 4"), Some(4.0));
        assert_eq!(parse_index("1-3"), Some(1.0));
        assert_eq!(parse_index("none"), None);
    }
}
