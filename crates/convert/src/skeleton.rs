use std::collections::HashMap;

use crate::dom::{self, Elem, Node, is_void};
use crate::error::{Result, internal};
use crate::util::to_base;

pub const CHUNK_SIZE: usize = 8192;
pub const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

pub const AID_ABLE: &[&str] = &[
    "a",
    "abbr",
    "address",
    "article",
    "aside",
    "audio",
    "b",
    "bdo",
    "blockquote",
    "body",
    "button",
    "cite",
    "code",
    "dd",
    "del",
    "details",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "i",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "map",
    "mark",
    "meter",
    "nav",
    "ol",
    "output",
    "p",
    "pre",
    "progress",
    "q",
    "rp",
    "rt",
    "samp",
    "section",
    "select",
    "small",
    "span",
    "strong",
    "sub",
    "summary",
    "sup",
    "textarea",
    "time",
    "ul",
    "var",
    "video",
];

const ALWAYS_EXPAND: &[&str] = &["script", "style", "title", "head"];

pub fn to_href(n: u64) -> String {
    to_base(n, 32, 10)
}

pub fn expand(e: &Elem) -> bool {
    let n = e.lname();
    AID_ABLE.contains(&n.as_str())
        || ALWAYS_EXPAND.contains(&n.as_str())
        || (e.attr("aid").is_some() && !is_void(&n))
}

pub fn serialize_elem(e: &Elem) -> Vec<u8> {
    let mut out = Vec::new();
    dom::write_elem(e, &mut out, &expand);
    out
}

struct Chunk {
    raw: Vec<u8>,
    starts: Vec<String>,
    ends: Vec<String>,
    insert_pos: usize,
    selector: String,
}

impl Chunk {
    fn new(raw: Vec<u8>, selector: &(char, String)) -> Chunk {
        Chunk {
            raw,
            starts: Vec::new(),
            ends: Vec::new(),
            insert_pos: 0,
            selector: format!("{}-//*[@aid='{}']", selector.0, selector.1),
        }
    }
}

struct Stepper {
    selector: (char, String),
}

impl Stepper {
    fn chunk_up_text(&self, text: &str) -> Vec<Chunk> {
        let mut escaped = Vec::new();
        dom::escape_text(text, &mut escaped);
        let mut parts = Vec::new();
        let mut rest: &[u8] = &escaped;
        let mut first = true;
        loop {
            let (head, tail) = split_multibyte(rest);
            if first {
                parts.push(head.to_vec());
                first = false;
            } else {
                let mut wrapped = b"<span class=\"AmznBigTextBlock\">".to_vec();
                wrapped.extend_from_slice(head);
                wrapped.extend_from_slice(b"</span>");
                parts.push(wrapped);
            }
            if tail.is_empty() {
                break;
            }
            rest = tail;
        }
        parts
            .into_iter()
            .map(|p| Chunk::new(p, &self.selector))
            .collect()
    }

    fn step_into(&mut self, tag: &mut Elem, chunks: &mut Vec<Chunk>) {
        let aid = tag.attr("aid").unwrap_or("").to_string();
        self.selector = ('P', aid.clone());
        let first_chunk = chunks.len();

        let old = std::mem::take(&mut tag.children);
        let mut kept: Vec<Node> = Vec::new();
        let mut iter = old.into_iter().peekable();
        if let Some(Node::Text(t)) = iter.peek() {
            if !t.trim().is_empty() {
                chunks.extend(self.chunk_up_text(t));
            } else {
                kept.push(Node::Text(t.clone()));
            }
            iter.next();
        }
        while let Some(node) = iter.next() {
            let Node::Elem(mut child) = node else {
                continue;
            };
            let tail = match iter.peek() {
                Some(Node::Text(_)) => match iter.next() {
                    Some(Node::Text(t)) => Some(t),
                    _ => None,
                },
                _ => None,
            };
            let raw = serialize_elem(&child);
            if raw.len() > CHUNK_SIZE && child.attr("aid").is_some() && has_content(&child) {
                self.step_into(&mut child, chunks);
                kept.push(Node::Elem(child));
                if let Some(t) = tail {
                    if !t.trim().is_empty() {
                        chunks.extend(self.chunk_up_text(&t));
                    } else {
                        kept.push(Node::Text(t));
                    }
                }
            } else {
                chunks.push(Chunk::new(raw, &self.selector));
                if let Some(t) = tail {
                    chunks.extend(self.chunk_up_text(&t));
                }
            }
        }
        tag.children = kept;

        if chunks.len() > first_chunk {
            chunks[first_chunk].starts.push(aid.clone());
            chunks.last_mut().unwrap().ends.push(aid.clone());
        }
        self.selector = ('S', aid);
    }
}

fn has_content(e: &Elem) -> bool {
    e.children.iter().any(|n| match n {
        Node::Elem(_) => true,
        Node::Text(t) => !t.trim().is_empty(),
    })
}

fn split_multibyte(raw: &[u8]) -> (&[u8], &[u8]) {
    if raw.len() <= CHUNK_SIZE {
        return (raw, &[]);
    }
    let mut cut = CHUNK_SIZE;
    while cut > 0 && (raw[cut] & 0xc0) == 0x80 {
        cut -= 1;
    }
    raw.split_at(cut)
}

fn merge_small_chunks(chunks: Vec<Chunk>) -> Vec<Chunk> {
    let mut out: Vec<Chunk> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        match out.last_mut() {
            Some(prev)
                if chunk.starts.is_empty()
                    && chunk.raw.len() + prev.raw.len() <= CHUNK_SIZE
                    && prev.ends.is_empty() =>
            {
                prev.raw.extend(chunk.raw);
                prev.ends = chunk.ends;
            }
            _ => out.push(chunk),
        }
    }
    out
}

fn metrics(e: &Elem, tail_len: usize, out: &mut HashMap<String, (usize, usize)>) {
    if let Some(aid) = e.attr("aid") {
        let mut start = Vec::new();
        dom::write_start_tag(e, &mut start, false);
        let mut start_len = start.len();
        if let Some(Node::Text(t)) = e.children.first() {
            start_len += t.len();
        }
        out.insert(aid.to_string(), (start_len, e.name.len() + 3 + tail_len));
    }
    for (i, n) in e.children.iter().enumerate() {
        if let Node::Elem(c) = n {
            let tail = match e.children.get(i + 1) {
                Some(Node::Text(t)) => t.len(),
                _ => 0,
            };
            metrics(c, tail, out);
        }
    }
}

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

fn render(root: &Elem) -> Vec<u8> {
    let mut with_ns = root.clone();
    with_ns
        .attrs
        .insert(0, ("xmlns".to_string(), XHTML_NS.to_string()));
    let mut out = XML_DECL.as_bytes().to_vec();
    dom::write_elem(&with_ns, &mut out, &expand);
    out.push(b'\n');
    out
}

pub struct Skeleton {
    pub skeleton: Vec<u8>,
    chunks: Vec<Chunk>,
    pub start_pos: usize,
}

impl Skeleton {
    pub fn len(&self) -> usize {
        self.skeleton.len() + self.chunks.iter().map(|c| c.raw.len()).sum::<usize>()
    }

    fn rebuild(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len());
        let mut last = 0;
        let mut inserted = 0;
        for c in &self.chunks {
            let at = c.insert_pos - inserted;
            out.extend_from_slice(&self.skeleton[last..at]);
            out.extend_from_slice(&c.raw);
            last = at;
            inserted += c.raw.len();
        }
        out.extend_from_slice(&self.skeleton[last..]);
        out
    }

    fn raw_text(&self) -> Vec<u8> {
        let mut out = self.skeleton.clone();
        for c in &self.chunks {
            out.extend_from_slice(&c.raw);
        }
        out
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn unwrap_big_blocks(raw: &[u8]) -> Vec<u8> {
    const OPEN: &[u8] = b"<span class=\"AmznBigTextBlock\">";
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while let Some(p) = find(&raw[i..], OPEN) {
        out.extend_from_slice(&raw[i..i + p]);
        let start = i + p + OPEN.len();
        let Some(len) = find(&raw[start..], b"</span>") else {
            i += p;
            break;
        };
        out.extend_from_slice(&raw[start..start + len]);
        i = start + len + 7;
    }
    out.extend_from_slice(&raw[i..]);
    out
}

fn build(root: &mut Elem) -> Result<Skeleton> {
    let body_idx = root
        .children
        .iter()
        .position(|n| matches!(n, Node::Elem(e) if e.is("body")))
        .ok_or_else(|| internal("document has no body"))?;
    match root.children.get_mut(body_idx + 1) {
        Some(Node::Text(t)) => *t = "\n".to_string(),
        _ => root
            .children
            .insert(body_idx + 1, Node::Text("\n".to_string())),
    }
    let original = render(root);

    let mut chunks = Vec::new();
    {
        let Node::Elem(body) = &mut root.children[body_idx] else {
            unreachable!()
        };
        let mut stepper = Stepper {
            selector: ('P', String::new()),
        };
        stepper.step_into(body, &mut chunks);
    }
    if chunks.is_empty() {
        return Err(internal("document body produced no chunks"));
    }
    let mut chunks = merge_small_chunks(chunks);

    let skeleton = render(root);
    let body_offset = find(&skeleton, b"<body").ok_or_else(|| internal("skeleton has no body"))?;
    let mut metric_map = HashMap::new();
    metrics(root, 0, &mut metric_map);
    let mut pos = body_offset;
    for chunk in &mut chunks {
        for tag in &chunk.starts {
            pos += metric_map
                .get(tag)
                .ok_or_else(|| internal("missing tag metric"))?
                .0;
        }
        chunk.insert_pos = pos;
        pos += chunk.raw.len();
        for tag in &chunk.ends {
            pos += metric_map
                .get(tag)
                .ok_or_else(|| internal("missing tag metric"))?
                .1;
        }
    }
    let skel = Skeleton {
        skeleton,
        chunks,
        start_pos: 0,
    };
    let rebuilt = unwrap_big_blocks(&skel.rebuild());
    let original = unwrap_big_blocks(&original);
    if rebuilt != original {
        let at = rebuilt
            .iter()
            .zip(&original)
            .position(|(a, b)| a != b)
            .unwrap_or(rebuilt.len().min(original.len()));
        let ctx = |b: &[u8]| {
            String::from_utf8_lossy(&b[at.saturating_sub(40)..(at + 40).min(b.len())]).into_owned()
        };
        return Err(internal(format!(
            "skeleton rebuild differs from the source markup at byte {at}: {:?} vs {:?}",
            ctx(&rebuilt),
            ctx(&original)
        )));
    }
    Ok(skel)
}

pub struct SkelEntry {
    pub name: String,
    pub chunk_count: usize,
    pub start_pos: usize,
    pub length: usize,
}

pub struct ChunkEntry {
    pub insert_pos: usize,
    pub selector: String,
    pub file_number: usize,
    pub sequence_number: usize,
    pub start_pos: usize,
    pub length: usize,
}

pub struct Chunked {
    pub text: Vec<u8>,
    pub skel_table: Vec<SkelEntry>,
    pub chunk_table: Vec<ChunkEntry>,
    pub aid_offsets: HashMap<String, (usize, usize, usize)>,
}

pub fn chunk(docs: &mut [Elem], placeholders: &HashMap<String, String>) -> Result<Chunked> {
    let mut skeletons = Vec::with_capacity(docs.len());
    for root in docs.iter_mut() {
        skeletons.push(build(root)?);
    }
    let mut sp = 0;
    for s in &mut skeletons {
        s.start_pos = sp;
        sp += s.len();
    }
    let skel_table = skeletons
        .iter()
        .enumerate()
        .map(|(i, s)| SkelEntry {
            name: format!("SKEL{i:010}"),
            chunk_count: s.chunks.len(),
            start_pos: s.start_pos,
            length: s.skeleton.len(),
        })
        .collect();
    let mut chunk_table = Vec::new();
    for (i, s) in skeletons.iter().enumerate() {
        let mut cp = 0;
        for c in &s.chunks {
            chunk_table.push(ChunkEntry {
                insert_pos: c.insert_pos + s.start_pos,
                selector: c.selector.clone(),
                file_number: i,
                sequence_number: chunk_table.len(),
                start_pos: cp,
                length: c.raw.len(),
            });
            cp += c.raw.len();
        }
    }

    let mut text = Vec::with_capacity(sp);
    let mut rebuilt = Vec::with_capacity(sp);
    for s in &skeletons {
        text.extend(s.raw_text());
        rebuilt.extend(s.rebuild());
    }
    let aid_offsets = locate_aids(&rebuilt, &chunk_table)?;
    replace_placeholders(&mut text, placeholders, &aid_offsets);
    Ok(Chunked {
        text,
        skel_table,
        chunk_table,
        aid_offsets,
    })
}

fn locate_aids(
    rebuilt: &[u8],
    chunk_table: &[ChunkEntry],
) -> Result<HashMap<String, (usize, usize, usize)>> {
    let mut map = HashMap::new();
    let last = chunk_table.len().saturating_sub(1);
    let mut first = 0;
    let mut i = 0;
    while let Some(rel) = rebuilt[i..].iter().position(|&b| b == b'<') {
        let start = i + rel;
        i = start + 1;
        if matches!(rebuilt.get(i), Some(b'/' | b'!' | b'?')) {
            continue;
        }
        let Some(end_rel) = rebuilt[start..].iter().position(|&b| b == b'>') else {
            break;
        };
        let tag = &rebuilt[start..start + end_rel];
        let value = [&b" aid=\""[..], &b" cid=\""[..]].iter().find_map(|key| {
            let at = find(tag, key)? + key.len();
            let len = tag[at..].iter().position(|&b| b == b'"')?;
            Some(String::from_utf8_lossy(&tag[at..at + len]).into_owned())
        });
        let Some(aid) = value else { continue };
        let mut found = None;
        for (k, c) in chunk_table.iter().enumerate().skip(first) {
            if c.insert_pos <= start && start < c.insert_pos + c.length {
                found = Some((k, c.sequence_number, start - c.insert_pos));
                break;
            }
            if c.insert_pos > start {
                found = Some((k, c.sequence_number, 0));
                break;
            }
            if k == last {
                found = Some((k, c.sequence_number, start - c.insert_pos));
                break;
            }
        }
        let (k, seq, off) = found.ok_or_else(|| internal("no chunk for aid"))?;
        first = k;
        map.insert(aid, (seq, off, start));
    }
    Ok(map)
}

fn replace_placeholders(
    text: &mut [u8],
    placeholders: &HashMap<String, String>,
    aid_offsets: &HashMap<String, (usize, usize, usize)>,
) {
    const PREFIX: &[u8] = b"kindle:pos:fid:0000:off:";
    let mut i = 0;
    while let Some(rel) = find(&text[i..], PREFIX) {
        let at = i + rel;
        let end = at + PREFIX.len() + 10;
        i = at + PREFIX.len();
        if end > text.len() {
            break;
        }
        let key = String::from_utf8_lossy(&text[at..end]).into_owned();
        let Some(&(seq, off, _)) = placeholders.get(&key).and_then(|aid| aid_offsets.get(aid))
        else {
            continue;
        };
        let replacement = format!("{}:off:{}", to_base(seq as u64, 32, 4), to_href(off as u64));
        text[at + PREFIX.len() - 9..end].copy_from_slice(replacement.as_bytes());
        i = end;
    }
}
