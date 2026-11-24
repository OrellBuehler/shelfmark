use std::io::Write;

use crate::error::{Result, invalid};
use crate::index::utf8_text;
use crate::lang;
use crate::palmdoc;
use crate::tbs::{self, TocEntry};
use crate::util::{RECORD_SIZE, put_u16, put_u32, to_base};

const NULL: u32 = 0xFFFF_FFFF;

const FLIS: &[u8] = b"FLIS\0\0\0\x08\0\x41\0\0\0\0\0\0\xff\xff\xff\xff\0\x01\0\x03\0\0\0\x03\0\0\0\x01\xff\xff\xff\xff";

fn fcis(text_length: usize) -> Vec<u8> {
    let mut out = b"FCIS\x00\x00\x00\x14\x00\x00\x00\x10\x00\x00\x00\x02\x00\x00\x00\x00".to_vec();
    put_u32(&mut out, text_length as u32);
    out.extend_from_slice(b"\x00\x00\x00\x00\x00\x00\x00\x28\x00\x00\x00\x00\x00\x00\x00");
    out.extend_from_slice(b"\x28\x00\x00\x00\x08\x00\x01\x00\x01\x00\x00\x00\x00");
    out
}

pub struct ExthMeta {
    pub title: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub isbn: Option<String>,
    pub subjects: Vec<String>,
    pub language: Option<String>,
    pub uuid: String,
    pub pubdate: String,
    pub rtl: bool,
    pub writing_mode: Option<String>,
}

pub struct Parts {
    pub flows: Vec<Vec<u8>>,
    pub chunk_records: Vec<Vec<u8>>,
    pub skel_records: Vec<Vec<u8>>,
    pub guide_records: Vec<Vec<u8>>,
    pub ncx_records: Vec<Vec<u8>>,
    pub toc: Vec<TocEntry>,
    pub resources: Vec<Vec<u8>>,
    pub cover_offset: Option<u32>,
    pub thumbnail_offset: Option<u32>,
    pub has_fonts: bool,
    pub start_offset: Option<u32>,
    pub meta: ExthMeta,
}

fn overlap_len(text: &[u8], npos: usize) -> usize {
    let mut start = npos;
    let mut back = 0;
    while start > 0 && back < 4 {
        start -= 1;
        back += 1;
        if text[start] & 0xc0 != 0x80 {
            let need = match text[start] {
                b if b < 0x80 => 1,
                b if b >> 5 == 0b110 => 2,
                b if b >> 4 == 0b1110 => 3,
                b if b >> 3 == 0b11110 => 4,
                _ => 1,
            };
            return (start + need).saturating_sub(npos);
        }
    }
    0
}

fn collapse(s: &str) -> String {
    s.split([' ', '\t', '\r', '\n', '\x0b'])
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn exth_record(out: &mut Vec<u8>, count: &mut u32, code: u32, data: &[u8]) {
    put_u32(out, code);
    put_u32(out, data.len() as u32 + 8);
    out.extend_from_slice(data);
    *count += 1;
}

fn exth_num(out: &mut Vec<u8>, count: &mut u32, code: u32, value: u32) {
    exth_record(out, count, code, &value.to_be_bytes());
}

fn build_exth(p: &Parts, num_resources: u32) -> Vec<u8> {
    let m = &p.meta;
    let mut out = Vec::new();
    let mut n = 0u32;
    let text = |out: &mut Vec<u8>, n: &mut u32, code: u32, s: &str, collapse_ws: bool| {
        let s = if collapse_ws {
            collapse(s)
        } else {
            s.to_string()
        };
        exth_record(out, n, code, &utf8_text(&s));
    };
    for a in m.authors.iter().filter(|a| !a.trim().is_empty()) {
        text(&mut out, &mut n, 100, a, true);
    }
    if let Some(v) = m.publisher.as_deref().filter(|s| !s.trim().is_empty()) {
        text(&mut out, &mut n, 101, v, true);
    }
    if let Some(v) = m.description.as_deref().filter(|s| !s.trim().is_empty()) {
        text(&mut out, &mut n, 103, v, false);
    }
    if let Some(v) = m.isbn.as_deref().filter(|s| !s.trim().is_empty()) {
        text(&mut out, &mut n, 104, v, true);
    }
    for s in m.subjects.iter().filter(|s| !s.trim().is_empty()) {
        text(&mut out, &mut n, 105, s, true);
    }
    text(&mut out, &mut n, 503, &m.title, true);
    if let Some(v) = m.language.as_deref().filter(|s| !s.trim().is_empty()) {
        text(&mut out, &mut n, 524, &lang::exth_language(v), true);
    }
    exth_record(&mut out, &mut n, 113, m.uuid.as_bytes());
    exth_record(
        &mut out,
        &mut n,
        112,
        format!("shelfmark:{}", m.uuid).as_bytes(),
    );
    exth_record(&mut out, &mut n, 501, b"EBOK");
    exth_record(&mut out, &mut n, 106, m.pubdate.as_bytes());
    for (code, val) in [(204, 201), (205, 2), (206, 9), (207, 0)] {
        exth_num(&mut out, &mut n, code, val);
    }
    exth_record(&mut out, &mut n, 535, b"0730-890adc2");
    if let Some(c) = p.cover_offset {
        exth_num(&mut out, &mut n, 201, c);
        exth_num(&mut out, &mut n, 203, 0);
    }
    if let Some(t) = p.thumbnail_offset {
        exth_num(&mut out, &mut n, 202, t);
        let uri = format!("kindle:embed:{}", to_base(t as u64, 32, 4));
        exth_record(&mut out, &mut n, 129, uri.as_bytes());
    }
    if let Some(s) = p.start_offset {
        exth_num(&mut out, &mut n, 116, s);
    }
    exth_num(&mut out, &mut n, 125, num_resources);
    if num_resources > 0 {
        exth_num(&mut out, &mut n, 131, 0);
    }
    let mode = m
        .writing_mode
        .as_deref()
        .or(m.rtl.then_some("horizontal-rl"));
    if let Some(mode) = mode {
        exth_record(&mut out, &mut n, 525, mode.as_bytes());
    }
    if m.rtl {
        exth_record(&mut out, &mut n, 527, b"rtl");
    }
    exth_record(&mut out, &mut n, 528, b"true");

    let mut exth = b"EXTH".to_vec();
    put_u32(&mut exth, out.len() as u32 + 12);
    put_u32(&mut exth, n);
    exth.extend(&out);
    exth.resize(exth.len() + 4 - out.len() % 4, 0);
    exth
}

struct Layout {
    first_non_text: u32,
    last_text: u16,
    text_length: u32,
    chunk_index: u32,
    skel_index: u32,
    guide_index: u32,
    ncx_index: u32,
    first_resource: u32,
    fdst: u32,
    fdst_count: u32,
    flis: u32,
    fcis: u32,
}

fn record0(p: &Parts, l: &Layout, uid: u32, num_resources: u32) -> Vec<u8> {
    let title = utf8_text(&p.meta.title);
    let mut r = Vec::new();
    put_u16(&mut r, 2);
    put_u16(&mut r, 0);
    put_u32(&mut r, l.text_length);
    put_u16(&mut r, l.last_text);
    put_u16(&mut r, RECORD_SIZE as u16);
    put_u16(&mut r, 0);
    put_u16(&mut r, 0);
    r.extend_from_slice(b"MOBI");
    put_u32(&mut r, 264);
    put_u32(&mut r, 2);
    put_u32(&mut r, 65001);
    put_u32(&mut r, uid);
    put_u32(&mut r, 8);
    for _ in 0..10 {
        put_u32(&mut r, NULL);
    }
    put_u32(&mut r, l.first_non_text);
    let title_offset_at = r.len();
    put_u32(&mut r, 0);
    put_u32(&mut r, title.len() as u32);
    put_u32(&mut r, lang::mobi_locale(p.meta.language.as_deref()));
    put_u32(&mut r, 0);
    put_u32(&mut r, 0);
    put_u32(&mut r, 8);
    put_u32(&mut r, l.first_resource);
    r.extend_from_slice(&[0; 16]);
    put_u32(&mut r, 0b1010000 | if p.has_fonts { 0x1000 } else { 0 });
    r.extend_from_slice(&[0; 32]);
    put_u32(&mut r, NULL);
    put_u32(&mut r, NULL);
    r.extend_from_slice(&[0; 12]);
    r.extend_from_slice(&[0; 8]);
    put_u32(&mut r, l.fdst);
    put_u32(&mut r, l.fdst_count);
    put_u32(&mut r, l.fcis);
    put_u32(&mut r, 1);
    put_u32(&mut r, l.flis);
    put_u32(&mut r, 1);
    r.extend_from_slice(&[0; 8]);
    put_u32(&mut r, NULL);
    put_u32(&mut r, 0);
    r.extend_from_slice(&[0xff; 8]);
    put_u32(&mut r, if p.toc.is_empty() { 0b1 } else { 0b11 });
    put_u32(&mut r, l.ncx_index);
    put_u32(&mut r, l.chunk_index);
    put_u32(&mut r, l.skel_index);
    put_u32(&mut r, NULL);
    put_u32(&mut r, l.guide_index);
    put_u32(&mut r, NULL);
    put_u32(&mut r, 0);
    put_u32(&mut r, NULL);
    put_u32(&mut r, 0);
    r.extend(build_exth(p, num_resources));
    let title_at = r.len() as u32;
    r[title_offset_at..title_offset_at + 4].copy_from_slice(&title_at.to_be_bytes());
    r.extend(&title);
    r.extend(std::iter::repeat_n(0u8, 8192));
    r
}

fn palm_name(title: &str) -> Vec<u8> {
    let mut name = String::new();
    for c in title.chars() {
        let c = match c {
            ' ' => '_',
            c if c.is_ascii_alphanumeric() => c,
            '.' | '-' | '_' | ',' | '\'' | '(' | ')' | '!' | '&' => c,
            'à'..='å' | 'À'..='Å' => 'a',
            'ç' | 'Ç' => 'c',
            'è'..='ë' | 'È'..='Ë' => 'e',
            'ì'..='ï' | 'Ì'..='Ï' => 'i',
            'ñ' | 'Ñ' => 'n',
            'ò'..='ö' | 'Ò'..='Ö' | 'ø' | 'Ø' => 'o',
            'ù'..='ü' | 'Ù'..='Ü' => 'u',
            'ý' | 'ÿ' | 'Ý' => 'y',
            _ => continue,
        };
        name.push(c);
    }
    let mut bytes: Vec<u8> = name.into_bytes();
    bytes.truncate(31);
    bytes.resize(32, 0);
    bytes
}

pub fn assemble(mut p: Parts, now: u32, uid: u32) -> Result<Vec<u8>> {
    let text: Vec<u8> = p.flows.concat();
    let text_length = text.len();
    let mut records: Vec<Vec<u8>> = vec![Vec::new()];
    let mut lengths = Vec::new();
    let mut records_size = 0;
    let mut pos = 0;
    while pos < text_length {
        let end = (pos + RECORD_SIZE).min(text_length);
        let data = &text[pos..end];
        let overlap = overlap_len(&text, end);
        let mut rec = palmdoc::compress(data);
        rec.extend_from_slice(&text[end..end + overlap]);
        rec.push(overlap as u8);
        lengths.push(data.len());
        records_size += rec.len();
        records.push(rec);
        pos = end;
    }
    let nrecords = records.len() - 1;
    if !p.toc.is_empty() {
        tbs::apply_trailing_byte_sequences(&p.toc, &mut records[1..], &lengths);
    }
    let mut first_non_text = nrecords as u32 + 1;
    if records_size % 4 != 0 {
        records.push(vec![0; records_size % 4]);
        first_non_text += 1;
    }

    let chunk_index = records.len() as u32;
    records.append(&mut p.chunk_records);
    let skel_index = records.len() as u32;
    records.append(&mut p.skel_records);
    let mut guide_index = NULL;
    if !p.guide_records.is_empty() {
        guide_index = records.len() as u32;
        records.append(&mut p.guide_records);
    }
    let mut ncx_index = NULL;
    if !p.ncx_records.is_empty() {
        ncx_index = records.len() as u32;
        records.append(&mut p.ncx_records);
    }
    let mut first_resource = NULL;
    let before = records.len();
    if !p.resources.is_empty() {
        first_resource = records.len() as u32;
        records.extend(std::mem::take(&mut p.resources));
    }
    let num_resources = (records.len() - before) as u32;

    let mut fdst = b"FDST".to_vec();
    put_u32(&mut fdst, 12);
    put_u32(&mut fdst, p.flows.len() as u32);
    let mut start = 0;
    for f in &p.flows {
        put_u32(&mut fdst, start as u32);
        start += f.len();
        put_u32(&mut fdst, start as u32);
    }
    let fdst_index = records.len() as u32;
    records.push(fdst);
    let flis_index = records.len() as u32;
    records.push(FLIS.to_vec());
    let fcis_index = records.len() as u32;
    records.push(fcis(text_length));
    records.push(b"\xe9\x8e\r\n".to_vec());

    let layout = Layout {
        first_non_text,
        last_text: nrecords as u16,
        text_length: text_length as u32,
        chunk_index,
        skel_index,
        guide_index,
        ncx_index,
        first_resource,
        fdst: fdst_index,
        fdst_count: p.flows.len() as u32,
        flis: flis_index,
        fcis: fcis_index,
    };
    if records.len() > 0xFFFF || text_length > u32::MAX as usize / 2 {
        return Err(invalid("the book is too large for the AZW3 format"));
    }
    records[0] = record0(&p, &layout, uid, num_resources);

    let mut out = Vec::new();
    out.extend(palm_name(&p.meta.title));
    out.extend([0u8; 2]);
    out.extend([0u8; 2]);
    put_u32(&mut out, now);
    put_u32(&mut out, now);
    out.extend([0u8; 16]);
    out.extend_from_slice(b"BOOKMOBI");
    put_u32(&mut out, (2 * records.len() - 1) as u32);
    put_u32(&mut out, 0);
    put_u16(&mut out, records.len() as u16);
    let mut offset = out.len() + 8 * records.len() + 2;
    for (i, r) in records.iter().enumerate() {
        put_u32(&mut out, offset as u32);
        out.push(0);
        out.extend_from_slice(&(2 * i as u32).to_be_bytes()[1..]);
        offset += r.len();
    }
    out.extend([0u8; 2]);
    for r in &records {
        out.write_all(r)?;
    }
    Ok(out)
}
