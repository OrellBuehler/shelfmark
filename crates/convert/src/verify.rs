use std::collections::HashMap;

use crate::palmdoc;

pub fn u16_at(b: &[u8], o: usize) -> usize {
    u16::from_be_bytes(b[o..o + 2].try_into().unwrap()) as usize
}

pub fn u32_at(b: &[u8], o: usize) -> usize {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap()) as usize
}

fn vwi(b: &[u8], mut pos: usize) -> (u64, usize) {
    let mut val = 0u64;
    let start = pos;
    loop {
        let x = b[pos];
        pos += 1;
        val = (val << 7) | (x & 0x7f) as u64;
        if x & 0x80 != 0 {
            return (val, pos - start);
        }
    }
}

fn vwi_back(b: &[u8]) -> (usize, usize) {
    let mut val = 0usize;
    let mut shift = 0;
    let mut n = 0;
    loop {
        let x = b[b.len() - 1 - n];
        val |= ((x & 0x7f) as usize) << shift;
        shift += 7;
        n += 1;
        if x & 0x80 != 0 {
            return (val, n);
        }
    }
}

pub type Tags = HashMap<u8, Vec<u64>>;

pub struct IndexData {
    pub entries: Vec<(Vec<u8>, Tags)>,
    pub cncx: HashMap<usize, String>,
    pub total_records: usize,
}

pub struct Skel {
    pub chunk_count: usize,
    pub start: usize,
    pub length: usize,
}

pub struct ChunkRow {
    pub insert_pos: usize,
    pub file: usize,
    pub start: usize,
    pub length: usize,
}

pub struct NcxRow {
    pub label: String,
    pub offset: usize,
    pub length: usize,
    pub depth: usize,
    pub parent: Option<usize>,
    pub first_child: Option<usize>,
    pub last_child: Option<usize>,
    pub pos: usize,
    pub fid: usize,
}

pub struct Book {
    pub records: Vec<Vec<u8>>,
    pub flows: Vec<Vec<u8>>,
    pub docs: Vec<String>,
    pub exth: Vec<(u32, Vec<u8>)>,
    pub ncx: Vec<NcxRow>,
    pub guide: Vec<(String, String, usize, usize)>,
    pub skels: Vec<Skel>,
    pub chunks: Vec<ChunkRow>,
    pub first_resource: Option<usize>,
    pub title: String,
    pub text_length: usize,
    pub locale: usize,
}

impl Book {
    pub fn exth(&self, code: u32) -> Vec<&[u8]> {
        self.exth
            .iter()
            .filter(|(c, _)| *c == code)
            .map(|(_, v)| v.as_slice())
            .collect()
    }

    pub fn exth_u32(&self, code: u32) -> Option<u32> {
        self.exth(code)
            .first()
            .map(|v| u32::from_be_bytes((*v).try_into().unwrap()))
    }

    pub fn exth_str(&self, code: u32) -> Vec<String> {
        self.exth(code)
            .into_iter()
            .map(|v| String::from_utf8(v.to_vec()).unwrap())
            .collect()
    }

    pub fn resources(&self) -> &[Vec<u8>] {
        match self.first_resource {
            Some(first) => &self.records[first..self.records.len() - 4],
            None => &[],
        }
    }

    pub fn all_text(&self) -> String {
        self.docs
            .iter()
            .map(|d| {
                let doc = roxmltree::Document::parse(d).unwrap();
                let body = doc.descendants().find(|n| n.has_tag_name("body")).unwrap();
                body.descendants()
                    .filter(|n| n.is_text())
                    .map(|n| n.text().unwrap())
                    .collect::<String>()
            })
            .collect()
    }
}

pub fn decode_index(records: &[Vec<u8>], start: usize) -> IndexData {
    let h = &records[start];
    assert_eq!(&h[..4], b"INDX");
    assert_eq!(u32_at(h, 4), 192);
    assert_eq!(u32_at(h, 28), 65001);
    let nrec = u32_at(h, 24);
    let ncncx = u32_at(h, 52);
    let tagx_off = u32_at(h, 180);
    assert_eq!(&h[tagx_off..tagx_off + 4], b"TAGX");
    let tagx_len = u32_at(h, tagx_off + 4);
    let ncb = u32_at(h, tagx_off + 8);
    assert_eq!(h.len() % 4, 0);
    let tags: Vec<[u8; 4]> = (0..(tagx_len - 12) / 4)
        .map(|i| {
            h[tagx_off + 12 + i * 4..tagx_off + 16 + i * 4]
                .try_into()
                .unwrap()
        })
        .collect();
    let idxt_off = u32_at(h, 20);
    assert_eq!(&h[idxt_off..idxt_off + 4], b"IDXT");

    let mut cncx = HashMap::new();
    for i in 0..ncncx {
        let rec = &records[start + nrec + 1 + i];
        assert_eq!(rec.len() % 4, 0);
        let mut pos = 0;
        while pos < rec.len() && rec[pos..].iter().any(|&b| b != 0) {
            let at = pos;
            let (len, c) = vwi(rec, pos);
            pos += c;
            let s = String::from_utf8(rec[pos..pos + len as usize].to_vec()).unwrap();
            pos += len as usize;
            cncx.insert(i * 0x10000 + at, s);
        }
    }

    let mut entries = Vec::new();
    for i in 0..nrec {
        let r = &records[start + 1 + i];
        assert_eq!(&r[..4], b"INDX");
        assert_eq!(r.len() % 4, 0);
        assert!(r.len() <= 0x10000);
        let idxt = u32_at(r, 20);
        let count = u32_at(r, 24);
        assert_eq!(&r[idxt..idxt + 4], b"IDXT");
        for j in 0..count {
            let off = u16_at(r, idxt + 4 + 2 * j);
            let len = r[off] as usize;
            let name = r[off + 1..off + 1 + len].to_vec();
            let control = &r[off + 1 + len..off + 1 + len + ncb];
            let mut pos = off + 1 + len + ncb;
            let mut cb = 0;
            let mut tm: Tags = HashMap::new();
            for t in &tags {
                let (num, vpe, mask, end) = (t[0], t[1] as usize, t[2], t[3]);
                if end & 1 != 0 {
                    cb += 1;
                    continue;
                }
                let v = control[cb] & mask;
                if v == 0 {
                    continue;
                }
                let n = (v >> mask.trailing_zeros()) as usize;
                let mut vals = Vec::new();
                for _ in 0..n * vpe {
                    let (x, c) = vwi(r, pos);
                    pos += c;
                    vals.push(x);
                }
                tm.insert(num, vals);
            }
            assert!(pos <= idxt, "entry overruns record");
            entries.push((name, tm));
        }
    }
    IndexData {
        entries,
        cncx,
        total_records: 1 + nrec + ncncx,
    }
}

pub fn parse(file: &[u8]) -> Book {
    assert_eq!(&file[60..68], b"BOOKMOBI");
    let n = u16_at(file, 76);
    let offsets: Vec<usize> = (0..n).map(|i| u32_at(file, 78 + i * 8)).collect();
    assert_eq!(offsets[0], 78 + n * 8 + 2);
    for w in offsets.windows(2) {
        assert!(w[0] <= w[1]);
    }
    for i in 0..n {
        assert_eq!(
            &file[78 + i * 8 + 5..78 + i * 8 + 8],
            &(2 * i as u32).to_be_bytes()[1..]
        );
        assert_eq!(file[78 + i * 8 + 4], 0);
    }
    let records: Vec<Vec<u8>> = (0..n)
        .map(|i| file[offsets[i]..offsets.get(i + 1).copied().unwrap_or(file.len())].to_vec())
        .collect();
    assert_eq!(records.last().unwrap(), b"\xe9\x8e\r\n");
    assert_eq!(u32_at(file, 68), 2 * n - 1);

    let r0 = &records[0];
    assert_eq!(u16_at(r0, 0), 2);
    let text_length = u32_at(r0, 4);
    let last_text = u16_at(r0, 8);
    assert_eq!(u16_at(r0, 10), 4096);
    assert_eq!(u16_at(r0, 12), 0);
    assert_eq!(&r0[16..20], b"MOBI");
    assert_eq!(u32_at(r0, 20), 264);
    assert_eq!(u32_at(r0, 24), 2);
    assert_eq!(u32_at(r0, 28), 65001);
    assert_eq!(u32_at(r0, 36), 8);
    assert_eq!(u32_at(r0, 104), 8);
    let locale = u32_at(r0, 92);
    let first_non_text = u32_at(r0, 80);
    assert!(first_non_text == last_text + 1 || first_non_text == last_text + 2);
    let title_off = u32_at(r0, 84);
    let title_len = u32_at(r0, 88);
    let title = String::from_utf8(r0[title_off..title_off + title_len].to_vec()).unwrap();
    assert_eq!(r0.len() - (title_off + title_len), 8192);
    let first_resource = match u32_at(r0, 108) {
        0xFFFF_FFFF => None,
        v => Some(v),
    };
    let flags = u32_at(r0, 240);
    assert_eq!(flags & !0b11, 0);
    assert_eq!(u32_at(r0, 200 + 4), 1);
    assert_eq!(u32_at(r0, 208 + 4), 1);

    let mut exth = Vec::new();
    assert_eq!(u32_at(r0, 128) & 0x40, 0x40);
    assert_eq!(&r0[280..284], b"EXTH");
    let exth_len = u32_at(r0, 284);
    let count = u32_at(r0, 288);
    let mut p = 292;
    for _ in 0..count {
        let code = u32_at(r0, p) as u32;
        let len = u32_at(r0, p + 4);
        exth.push((code, r0[p + 8..p + len].to_vec()));
        p += len;
    }
    assert_eq!(p - 280, exth_len);
    let pad = 4 - (exth_len - 12) % 4;
    assert_eq!(title_off, 280 + exth_len + pad);

    let mut text = Vec::new();
    for (i, record) in records.iter().enumerate().take(last_text + 1).skip(1) {
        let mut body: &[u8] = record;
        if flags & 0b10 != 0 {
            let (size, n) = vwi_back(body);
            assert!(size >= n && size <= body.len());
            check_tbs(&body[body.len() - size..body.len() - n]);
            body = &body[..body.len() - size];
        }
        let overlap_len = (body[body.len() - 1] & 0b11) as usize;
        body = &body[..body.len() - 1 - overlap_len];
        let data = palmdoc::decompress(body);
        if i < last_text {
            assert_eq!(data.len(), 4096, "record {i}");
        } else {
            assert!(data.len() <= 4096 && !data.is_empty());
        }
        text.extend_from_slice(&data);
    }
    assert_eq!(text.len(), text_length);
    for i in 1..last_text {
        let rec = &records[i];
        let mut body: &[u8] = rec;
        if flags & 0b10 != 0 {
            let (size, _) = vwi_back(body);
            body = &body[..body.len() - size];
        }
        let overlap_len = (body[body.len() - 1] & 0b11) as usize;
        let overlap = &body[body.len() - 1 - overlap_len..body.len() - 1];
        assert_eq!(
            overlap,
            &text[i * 4096..i * 4096 + overlap_len],
            "overlap of record {i}"
        );
        let mut piece = text[(i - 1) * 4096..i * 4096].to_vec();
        piece.extend_from_slice(overlap);
        let skip = piece
            .iter()
            .take(4)
            .take_while(|b| **b & 0xc0 == 0x80)
            .count();
        let _ = std::str::from_utf8(&piece[skip..]).expect("record ends inside a character");
    }
    std::str::from_utf8(&text).expect("text is valid utf-8");

    let fdst_idx = u32_at(r0, 192);
    let fdst_count = u32_at(r0, 196);
    let fdst = &records[fdst_idx];
    assert_eq!(&fdst[..4], b"FDST");
    assert_eq!(u32_at(fdst, 4), 12);
    assert_eq!(u32_at(fdst, 8), fdst_count);
    assert_eq!(fdst.len(), 12 + 8 * fdst_count);
    let mut flows = Vec::new();
    let mut expect_start = 0;
    for i in 0..fdst_count {
        let s = u32_at(fdst, 12 + i * 8);
        let e = u32_at(fdst, 16 + i * 8);
        assert_eq!(s, expect_start);
        assert!(e >= s);
        flows.push(text[s..e].to_vec());
        expect_start = e;
    }
    assert_eq!(expect_start, text_length);
    assert_eq!(fdst_idx + 1, u32_at(r0, 208));
    assert_eq!(fdst_idx + 2, u32_at(r0, 200));
    assert_eq!(&records[fdst_idx + 1][..4], b"FLIS");
    assert_eq!(&records[fdst_idx + 2][..4], b"FCIS");
    assert_eq!(u32_at(&records[fdst_idx + 2], 20), text_length);

    let skel_idx = u32_at(r0, 252);
    let chunk_idx = u32_at(r0, 248);
    let skel_data = decode_index(&records, skel_idx);
    let chunk_data = decode_index(&records, chunk_idx);
    assert_eq!(chunk_idx + chunk_data.total_records, skel_idx);
    let skels: Vec<Skel> = skel_data
        .entries
        .iter()
        .enumerate()
        .map(|(i, (name, t))| {
            assert_eq!(name, format!("SKEL{i:010}").as_bytes());
            assert_eq!(t[&1][0], t[&1][1]);
            assert_eq!(t[&6][..2], t[&6][2..]);
            Skel {
                chunk_count: t[&1][0] as usize,
                start: t[&6][0] as usize,
                length: t[&6][1] as usize,
            }
        })
        .collect();
    let chunks: Vec<ChunkRow> = chunk_data
        .entries
        .iter()
        .enumerate()
        .map(|(i, (name, t))| ChunkRow {
            insert_pos: String::from_utf8(name.clone()).unwrap().parse().unwrap(),
            file: {
                let selector = &chunk_data.cncx[&(t[&2][0] as usize)];
                assert!(
                    selector.starts_with("P-//*[@aid='") || selector.starts_with("S-//*[@aid='")
                );
                assert_eq!(t[&4][0] as usize, i);
                t[&3][0] as usize
            },
            start: t[&6][0] as usize,
            length: t[&6][1] as usize,
        })
        .collect();

    let mut docs = Vec::new();
    let mut pos = 0;
    let mut ci = 0;
    for (fi, s) in skels.iter().enumerate() {
        assert_eq!(s.start, pos);
        let skeleton = &flows[0][s.start..s.start + s.length];
        let mut chunk_area = s.start + s.length;
        let mut doc = skeleton.to_vec();
        let mut rel = 0;
        let mut offset_in_area = 0;
        for _ in 0..s.chunk_count {
            let c = &chunks[ci];
            ci += 1;
            assert_eq!(c.file, fi);
            assert_eq!(c.start, offset_in_area);
            let raw = &flows[0][chunk_area..chunk_area + c.length];
            let at = c.insert_pos - s.start;
            assert!(at >= rel && at <= doc.len(), "chunk insert position");
            doc.splice(at..at, raw.iter().copied());
            rel = at + raw.len();
            chunk_area += c.length;
            offset_in_area += c.length;
        }
        pos = chunk_area;
        docs.push(String::from_utf8(doc).expect("document is valid utf-8"));
    }
    assert_eq!(ci, chunks.len());
    assert_eq!(pos, flows[0].len());
    for d in &docs {
        assert!(d.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\""));
        roxmltree::Document::parse(d).unwrap_or_else(|e| {
            panic!(
                "document is not well-formed: {e}\n{}",
                &d[..d.len().min(600)]
            )
        });
    }

    let mut ncx = Vec::new();
    let ncx_idx = u32_at(r0, 244);
    let mut guide = Vec::new();
    let mut last_index_end = skel_idx + skel_data.total_records;
    let guide_idx = u32_at(r0, 260);
    if guide_idx != 0xFFFF_FFFF {
        assert_eq!(guide_idx, last_index_end);
        let g = decode_index(&records, guide_idx);
        for (name, t) in &g.entries {
            guide.push((
                g.cncx[&(t[&1][0] as usize)].clone(),
                String::from_utf8(name.clone()).unwrap(),
                t[&6][0] as usize,
                t[&6][1] as usize,
            ));
        }
        last_index_end += g.total_records;
    }
    if ncx_idx != 0xFFFF_FFFF {
        assert_eq!(ncx_idx, last_index_end);
        let d = decode_index(&records, ncx_idx);
        last_index_end += d.total_records;
        for (i, (name, t)) in d.entries.iter().enumerate() {
            assert_eq!(
                usize::from_str_radix(std::str::from_utf8(name).unwrap(), 16).unwrap(),
                i
            );
            ncx.push(NcxRow {
                label: d.cncx[&(t[&3][0] as usize)].clone(),
                offset: t[&1][0] as usize,
                length: t[&2][0] as usize,
                depth: t[&4][0] as usize,
                parent: t.get(&21).map(|v| v[0] as usize),
                first_child: t.get(&22).map(|v| v[0] as usize),
                last_child: t.get(&23).map(|v| v[0] as usize),
                pos: t[&6][0] as usize,
                fid: t[&6][1] as usize,
            });
        }
        assert!(flags & 0b10 != 0);
    } else {
        assert_eq!(flags & 0b10, 0);
    }
    match first_resource {
        Some(r) => assert_eq!(r, last_index_end),
        None => assert_eq!(fdst_idx, last_index_end),
    }
    assert_eq!(n, fdst_idx + 4);

    for (i, row) in ncx.iter().enumerate() {
        assert!(row.offset + row.length <= flows[0].len());
        if i > 0 {
            let p = &ncx[i - 1];
            assert!((p.depth, p.offset) <= (row.depth, row.offset), "ncx order");
        }
        if let Some(p) = row.parent {
            assert_eq!(ncx[p].depth + 1, row.depth);
            assert!(ncx[p].first_child.unwrap() <= i && i <= ncx[p].last_child.unwrap());
        } else {
            assert_eq!(row.depth, 0);
        }
        assert!(row.pos < chunks.len());
        assert!(row.fid < chunks[row.pos].length.max(1) + 1);
    }

    let mut links = Vec::new();
    for d in &docs {
        let mut rest = d.as_str();
        while let Some(at) = rest.find("kindle:pos:fid:") {
            links.push(rest[at..at + 34].to_string());
            rest = &rest[at + 34..];
        }
    }
    for l in links {
        let fid = usize::from_str_radix(&l[15..19], 32).unwrap();
        let off = usize::from_str_radix(&l[24..34], 32).unwrap();
        assert!(fid < chunks.len(), "link {l}");
        assert!(off <= chunks[fid].length, "link {l}");
    }

    Book {
        records,
        flows,
        docs,
        exth,
        ncx,
        guide,
        skels,
        chunks,
        first_resource,
        title,
        text_length,
        locale,
    }
}

fn check_tbs(mut b: &[u8]) {
    let mut first = true;
    while !b.is_empty() {
        let flag_size = if first { 3 } else { 4 };
        first = false;
        let (arg, c) = vwi(b, 0);
        b = &b[c..];
        let flags = arg & ((1 << flag_size) - 1);
        if flags & 0b10 != 0 {
            let (_, c) = vwi(b, 0);
            b = &b[c..];
        }
        if flags & 0b100 != 0 {
            b = &b[1..];
        }
        if flags & 0b1 != 0 {
            let (_, c) = vwi(b, 0);
            b = &b[c..];
        }
    }
}
