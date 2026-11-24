use std::path::Path;

use shelfmark_model::Metadata;

use crate::error::{Result, invalid};
use crate::lang;
use crate::util::{clean, normalize_date, push_unique, sniff_image};

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

struct Mobi {
    data: Vec<u8>,
    offsets: Vec<usize>,
}

impl Mobi {
    fn parse(data: Vec<u8>) -> Result<Mobi> {
        let n = u16_at(&data, 76).ok_or_else(|| invalid("mobi", "truncated header"))? as usize;
        if n == 0 {
            return Err(invalid("mobi", "no records"));
        }
        let mut offsets = Vec::with_capacity(n);
        for i in 0..n {
            let off = u32_at(&data, 78 + i * 8)
                .ok_or_else(|| invalid("mobi", "truncated record list"))?
                as usize;
            if off > data.len() {
                return Err(invalid("mobi", "record offset out of range"));
            }
            offsets.push(off);
        }
        Ok(Mobi { data, offsets })
    }

    fn record(&self, i: usize) -> Option<&[u8]> {
        let start = *self.offsets.get(i)?;
        let end = self.offsets.get(i + 1).copied().unwrap_or(self.data.len());
        self.data.get(start..end.max(start))
    }
}

fn decode(b: &[u8], utf8: bool) -> String {
    if utf8 {
        String::from_utf8_lossy(b).into_owned()
    } else {
        match std::str::from_utf8(b) {
            Ok(s) if !s.is_ascii() => s.to_string(),
            _ => encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(b)
                .0
                .into_owned(),
        }
    }
}

fn lcid_lang(locale: u32) -> Option<&'static str> {
    Some(match locale & 0xFF {
        0x09 => "eng",
        0x07 => "deu",
        0x0c => "fra",
        0x0a => "spa",
        0x10 => "ita",
        0x13 => "nld",
        0x16 => "por",
        0x1d => "swe",
        0x06 => "dan",
        0x14 => "nor",
        0x0b => "fin",
        0x19 => "rus",
        0x15 => "pol",
        0x05 => "ces",
        0x0e => "hun",
        0x08 => "ell",
        0x1f => "tur",
        0x04 => "zho",
        0x11 => "jpn",
        0x12 => "kor",
        0x01 => "ara",
        0x0d => "heb",
        0x39 => "hin",
        _ => return None,
    })
}

struct Header {
    meta: Metadata,
    cover_record: Option<usize>,
}

fn parse(mobi: &Mobi) -> Result<Header> {
    let r0 = mobi
        .record(0)
        .ok_or_else(|| invalid("mobi", "missing record 0"))?;
    if r0.get(16..20) != Some(b"MOBI") {
        return Err(invalid("mobi", "no MOBI header"));
    }
    let hlen = u32_at(r0, 20).unwrap_or(0) as usize;
    let utf8 = u32_at(r0, 28) == Some(65001);
    let mut m = Metadata::default();

    if let (Some(off), Some(len)) = (u32_at(r0, 84), u32_at(r0, 88))
        && let Some(b) = r0.get(off as usize..(off as usize).saturating_add(len as usize))
    {
        m.title = clean(&decode(b, utf8));
    }
    if let Some(l) = u32_at(r0, 92).and_then(lcid_lang) {
        m.languages = vec![l.to_string()];
    }
    let first_image = u32_at(r0, 108)
        .filter(|v| *v != 0xFFFF_FFFF)
        .map(|v| v as usize);
    let exth_flags = u32_at(r0, 128).unwrap_or(0);
    let mut cover_offset = None;
    if exth_flags & 0x40 != 0 && hlen >= 116 {
        let base = 16 + hlen;
        if r0.get(base..base + 4) == Some(b"EXTH") {
            let count = u32_at(r0, base + 8).unwrap_or(0) as usize;
            let mut pos = base + 12;
            let mut authors = Vec::new();
            let mut asin = None;
            for _ in 0..count {
                let (Some(ty), Some(len)) = (u32_at(r0, pos), u32_at(r0, pos + 4)) else {
                    break;
                };
                let len = len as usize;
                if len < 8 {
                    break;
                }
                let Some(data) = r0.get(pos + 8..pos + len) else {
                    break;
                };
                pos += len;
                match ty {
                    100 => {
                        for a in decode(data, utf8).split('&') {
                            if let Some(a) = clean(a) {
                                push_unique(&mut authors, a);
                            }
                        }
                    }
                    101 => m.publisher = clean(&decode(data, utf8)),
                    103 => m.comments = clean(&decode(data, utf8)),
                    104 => {
                        if let Some(i) = clean(&decode(data, utf8)) {
                            m.identifiers.insert("isbn".into(), i.replace('-', ""));
                        }
                    }
                    105 => {
                        for t in decode(data, utf8).split(';') {
                            if let Some(t) = clean(t) {
                                push_unique(&mut m.tags, t);
                            }
                        }
                    }
                    106 => m.pubdate = normalize_date(&decode(data, utf8)),
                    113 | 504 => {
                        let v = decode(data, utf8);
                        let v = v.trim();
                        if !v.is_empty() && (ty == 113 || asin.is_none()) {
                            asin = Some(v.to_string());
                        }
                    }
                    201 => {
                        cover_offset = u32_at(data, 0)
                            .filter(|v| *v != 0xFFFF_FFFF)
                            .map(|v| v as usize)
                    }
                    503 => {
                        if let Some(t) = clean(&decode(data, utf8)) {
                            m.title = Some(t);
                        }
                    }
                    524 => {
                        if let Some(l) = lang::normalize(&decode(data, utf8)) {
                            m.languages = vec![l];
                        }
                    }
                    _ => {}
                }
            }
            m.authors = authors;
            if let Some(a) = asin {
                m.identifiers.insert("mobi-asin".into(), a);
            }
        }
    }
    let cover_record = first_image.zip(cover_offset).map(|(f, o)| f + o);
    Ok(Header {
        meta: m,
        cover_record,
    })
}

pub fn read_metadata(path: &Path) -> Result<Metadata> {
    Ok(parse(&Mobi::parse(std::fs::read(path)?)?)?.meta)
}

pub fn read_cover(path: &Path) -> Result<Option<Vec<u8>>> {
    let mobi = Mobi::parse(std::fs::read(path)?)?;
    let h = parse(&mobi)?;
    Ok(h.cover_record
        .and_then(|i| mobi.record(i))
        .filter(|d| sniff_image(d).is_some())
        .map(<[u8]>::to_vec))
}
