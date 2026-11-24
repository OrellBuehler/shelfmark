use std::collections::HashMap;

use crate::error::{Result, internal};
use crate::util::{align_block, encint, put_u16, put_u32};

pub const HEADER_LENGTH: usize = 192;

#[derive(Clone, Copy)]
pub struct TagMeta {
    pub name: &'static str,
    pub number: u8,
    pub values_per_entry: u8,
    pub mask: u8,
    pub end_flag: u8,
}

const fn tag(name: &'static str, number: u8, values_per_entry: u8, mask: u8) -> TagMeta {
    TagMeta {
        name,
        number,
        values_per_entry,
        mask,
        end_flag: 0,
    }
}

const END: TagMeta = TagMeta {
    name: "eof",
    number: 0,
    values_per_entry: 0,
    mask: 0,
    end_flag: 1,
};

pub const SKEL_TAGS: &[TagMeta] = &[tag("chunk_count", 1, 1, 3), tag("geometry", 6, 2, 12), END];

pub const CHUNK_TAGS: &[TagMeta] = &[
    tag("cncx_offset", 2, 1, 1),
    tag("file_number", 3, 1, 2),
    tag("sequence_number", 4, 1, 4),
    tag("geometry", 6, 2, 8),
    END,
];

pub const GUIDE_TAGS: &[TagMeta] = &[tag("title", 1, 1, 1), tag("pos_fid", 6, 2, 2), END];

pub const NCX_TAGS: &[TagMeta] = &[
    tag("offset", 1, 1, 1),
    tag("length", 2, 1, 2),
    tag("label", 3, 1, 4),
    tag("depth", 4, 1, 8),
    tag("parent", 21, 1, 16),
    tag("first_child", 22, 1, 32),
    tag("last_child", 23, 1, 64),
    tag("pos_fid", 6, 2, 128),
    END,
];

pub struct IndexEntry {
    pub lead: Vec<u8>,
    pub tags: Vec<(&'static str, Vec<u64>)>,
}

pub struct Cncx {
    offsets: HashMap<String, usize>,
    pub records: Vec<Vec<u8>>,
}

pub fn utf8_text(s: &str) -> Vec<u8> {
    let t = s.trim();
    if t.is_empty() {
        b"Unknown".to_vec()
    } else {
        t.as_bytes().to_vec()
    }
}

impl Cncx {
    pub fn new<'a>(strings: impl Iterator<Item = &'a str>) -> Cncx {
        const RECORD_LIMIT: usize = 0x10000 - 1024;
        let mut offsets: HashMap<String, usize> = HashMap::new();
        let mut records = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let mut offset = 0;
        for s in strings {
            if offsets.contains_key(s) {
                continue;
            }
            let truncated: String = s.chars().take(500).collect();
            let utf8 = utf8_text(&truncated);
            let mut raw = encint(utf8.len() as u64, true);
            raw.extend(utf8);
            if buf.len() + raw.len() > RECORD_LIMIT {
                records.push(align_block(std::mem::take(&mut buf)));
                offset = records.len() * 0x10000;
            }
            offsets.insert(s.to_string(), offset);
            offset += raw.len();
            buf.extend(raw);
        }
        if !buf.is_empty() {
            records.push(align_block(buf));
        }
        Cncx { offsets, records }
    }

    pub fn get(&self, s: &str) -> u64 {
        self.offsets.get(s).copied().unwrap_or(0) as u64
    }
}

pub fn build_index(
    tag_types: &[TagMeta],
    entries: &[IndexEntry],
    cncx: &Cncx,
) -> Result<Vec<Vec<u8>>> {
    let control_byte_count = tag_types.iter().filter(|t| t.end_flag == 1).count();
    const RECORD_LIMIT: usize = 0x10000 - HEADER_LENGTH - 1048;

    let mut index_blocks: Vec<Vec<u8>> = vec![Vec::new()];
    let mut idxt_blocks: Vec<Vec<u8>> = vec![Vec::new()];
    let mut record_counts = vec![0u32];
    let mut last_indices: Vec<Vec<u8>> = vec![Vec::new()];

    for entry in entries {
        let mut control = Vec::with_capacity(control_byte_count);
        let mut acc = 0u8;
        for t in tag_types {
            if t.end_flag == 1 {
                control.push(acc);
                acc = 0;
                continue;
            }
            let n = entry
                .tags
                .iter()
                .find(|(name, _)| *name == t.name)
                .map(|(_, v)| v.len())
                .unwrap_or(0);
            let nentries = (n / t.values_per_entry as usize) as u32;
            acc |= t.mask & ((nentries << t.mask.trailing_zeros()) as u8);
        }
        if control.len() != control_byte_count {
            return Err(internal("invalid index entry"));
        }
        let mut raw = vec![entry.lead.len() as u8];
        raw.extend(&entry.lead);
        raw.extend(control);
        for t in tag_types {
            if let Some((_, values)) = entry.tags.iter().find(|(name, _)| *name == t.name) {
                for v in values {
                    raw.extend(encint(*v, true));
                }
            }
        }
        let offset = index_blocks.last().unwrap().len();
        let idxt_pos = idxt_blocks.last().unwrap().len();
        if offset + idxt_pos + raw.len() + 2 > RECORD_LIMIT {
            index_blocks.push(Vec::new());
            idxt_blocks.push(Vec::new());
            record_counts.push(0);
            last_indices.push(Vec::new());
        }
        let offset = index_blocks.last().unwrap().len();
        *record_counts.last_mut().unwrap() += 1;
        put_u16(
            idxt_blocks.last_mut().unwrap(),
            (HEADER_LENGTH + offset) as u16,
        );
        index_blocks.last_mut().unwrap().extend(raw);
        *last_indices.last_mut().unwrap() = entry.lead.clone();
    }

    let mut records = Vec::new();
    for ((index_block, idxt_block), count) in index_blocks
        .into_iter()
        .zip(idxt_blocks)
        .zip(&record_counts)
    {
        let index_block = align_block(index_block);
        let mut idxt = b"IDXT".to_vec();
        idxt.extend(idxt_block);
        let idxt = align_block(idxt);
        let mut rec = b"INDX".to_vec();
        put_u32(&mut rec, HEADER_LENGTH as u32);
        put_u32(&mut rec, 0);
        put_u32(&mut rec, 1);
        put_u32(&mut rec, 0);
        put_u32(&mut rec, (HEADER_LENGTH + index_block.len()) as u32);
        put_u32(&mut rec, *count);
        rec.extend([0xff; 8]);
        rec.extend([0u8; 156]);
        rec.extend(index_block);
        rec.extend(idxt);
        if rec.len() > 0x10000 {
            return Err(internal("index record overflow"));
        }
        records.push(rec);
    }

    let mut tagx = b"TAGX".to_vec();
    put_u32(&mut tagx, (12 + 4 * tag_types.len()) as u32);
    put_u32(&mut tagx, control_byte_count as u32);
    for t in tag_types {
        tagx.extend([t.number, t.values_per_entry, t.mask, t.end_flag]);
    }

    let mut geometry = Vec::new();
    let mut idxt = b"IDXT".to_vec();
    let mut pos = HEADER_LENGTH + tagx.len();
    for (last, num) in last_indices.iter().zip(&record_counts) {
        let start = geometry.len();
        put_u16(&mut idxt, pos as u16);
        geometry.push(last.len() as u8);
        geometry.extend(last);
        put_u16(&mut geometry, *num as u16);
        pos += geometry.len() - start;
    }
    let geometry = align_block(geometry);
    let idxt = align_block(idxt);

    let mut header = b"INDX".to_vec();
    put_u32(&mut header, HEADER_LENGTH as u32);
    header.extend([0u8; 8]);
    put_u32(&mut header, 2);
    put_u32(
        &mut header,
        (HEADER_LENGTH + tagx.len() + geometry.len()) as u32,
    );
    put_u32(&mut header, records.len() as u32);
    put_u32(&mut header, 65001);
    put_u32(&mut header, 0xFFFF_FFFF);
    put_u32(&mut header, record_counts.iter().sum());
    put_u32(&mut header, 0);
    put_u32(&mut header, 0);
    put_u32(&mut header, 0);
    put_u32(&mut header, cncx.records.len() as u32);
    header.extend([0u8; 124]);
    put_u32(&mut header, HEADER_LENGTH as u32);
    header.extend([0u8; 8]);
    header.extend(tagx);
    header.extend(geometry);
    header.extend(idxt);
    let header = align_block(header);

    let mut out = vec![header];
    out.extend(records);
    out.extend(cncx.records.iter().cloned());
    Ok(out)
}
