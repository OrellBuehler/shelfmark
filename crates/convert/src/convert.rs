use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use shelfmark_model::Metadata;

use crate::dom::{self, Elem};
use crate::epub::{Epub, TocNode, collapse, is_xhtml};
use crate::error::{Error, Result, invalid};
use crate::font;
use crate::image::{self, Kind};
use crate::index::{CHUNK_TAGS, Cncx, GUIDE_TAGS, IndexEntry, NCX_TAGS, SKEL_TAGS, build_index};
use crate::markup::{self, Ctx, Doc};
use crate::skeleton::{self, Chunked};
use crate::tbs::TocEntry;
use crate::util::resolve_href;
use crate::writer::{self, ExthMeta, Parts};

struct Resources {
    records: Vec<Vec<u8>>,
    image_records: Vec<(usize, String)>,
    keep: HashSet<usize>,
    cover_offset: Option<usize>,
    thumbnail_offset: Option<usize>,
    has_fonts: bool,
}

fn is_raster_item(media_type: &str) -> bool {
    media_type.starts_with("image/") && media_type != "image/svg+xml"
}

fn first_image_href(root: &Elem, base: &str) -> Option<String> {
    let mut found = None;
    root.walk(&mut |e| {
        if found.is_some() {
            return;
        }
        if e.is("img") {
            found = e
                .attr("src")
                .and_then(|s| resolve_href(base, s))
                .map(|(p, _)| p);
        } else if e.is("image") {
            found = e
                .attr_local("href")
                .and_then(|s| resolve_href(base, s))
                .map(|(p, _)| p);
        }
    });
    found
}

fn find_cover_item(epub: &Epub, docs: &[Doc]) -> Option<usize> {
    let raster = |i: usize| is_raster_item(&epub.items[i].media_type);
    if let Some(id) = &epub.cover_id
        && let Some(i) = epub.items.iter().position(|it| &it.id == id)
        && raster(i)
    {
        return Some(i);
    }
    if let Some(i) = epub
        .items
        .iter()
        .position(|it| it.properties.iter().any(|p| p == "cover-image"))
        && raster(i)
    {
        return Some(i);
    }
    if let Some(g) = epub.guide.iter().find(|g| g.kind == "cover")
        && let Some(doc) = docs.iter().find(|d| d.href == g.href)
        && let Some(src) = first_image_href(&doc.root, &doc.href)
        && let Some(i) = epub.item_by_href(&src)
        && raster(i)
    {
        return Some(i);
    }
    epub.items
        .iter()
        .position(|it| {
            let stem = it.href.rsplit('/').next().unwrap_or("");
            let stem = stem.rsplit_once('.').map(|(s, _)| s).unwrap_or(stem);
            raster_cover_name(&it.id) || raster_cover_name(stem)
        })
        .filter(|&i| raster(i))
}

fn raster_cover_name(s: &str) -> bool {
    s.eq_ignore_ascii_case("cover")
        || s.eq_ignore_ascii_case("cover-image")
        || s.eq_ignore_ascii_case("coverimage")
}

fn build_resources(
    epub: &mut Epub,
    ctx: &mut Ctx,
    cover_item: Option<usize>,
    cover: Option<&[u8]>,
) -> Resources {
    let mut res = Resources {
        records: Vec::new(),
        image_records: Vec::new(),
        keep: HashSet::new(),
        cover_offset: None,
        thumbnail_offset: None,
        has_fonts: false,
    };
    let items = epub.items.clone();
    for (idx, item) in items.iter().enumerate() {
        if !is_raster_item(&item.media_type) || ctx.item_map.contains_key(&item.href) {
            continue;
        }
        let Ok(data) = epub.read(&item.href) else {
            continue;
        };
        let is_cover = Some(idx) == cover_item;
        let processed = if is_cover {
            let own = cover.map(<[u8]>::to_vec).into_iter().chain([data]);
            own.into_iter().find_map(|d| {
                image::process_cover(&d)
                    .map(|j| (j, Kind::Jpeg))
                    .or_else(|| image::process(d))
            })
        } else {
            image::process(data)
        };
        let Some((bytes, kind)) = processed else {
            continue;
        };
        let rec = res.records.len();
        res.image_records.push((rec, item.href.clone()));
        ctx.item_map.insert(item.href.clone(), rec + 1);
        ctx.mime_map
            .insert(item.href.clone(), image::mime_for(kind).to_string());
        if is_cover && kind == Kind::Jpeg {
            res.cover_offset = Some(rec);
            res.keep.insert(rec);
            res.records.push(bytes);
            add_thumbnail(&mut res);
        } else {
            res.records.push(bytes);
        }
    }
    if res.cover_offset.is_none()
        && let Some(bytes) = cover.and_then(image::process_cover)
    {
        let rec = res.records.len();
        res.cover_offset = Some(rec);
        res.keep.insert(rec);
        res.records.push(bytes);
        add_thumbnail(&mut res);
    }
    for item in &items {
        let ext = item
            .href
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if (ext != "ttf" && ext != "otf") || epub.obfuscated.contains(&item.href) {
            continue;
        }
        let Ok(data) = epub.read(&item.href) else {
            continue;
        };
        if !font::is_font(&data) {
            continue;
        }
        res.records.push(font::font_record(&data));
        ctx.item_map.insert(item.href.clone(), res.records.len());
        ctx.fonts.insert(item.href.clone());
        res.has_fonts = true;
    }
    res
}

fn add_thumbnail(res: &mut Resources) {
    let cover = res.cover_offset.unwrap();
    if let Some(thumb) = image::thumbnail(&res.records[cover]) {
        res.thumbnail_offset = Some(res.records.len());
        res.keep.insert(res.records.len());
        res.records.push(thumb);
    }
}

fn doc_title(root: &Elem) -> String {
    if let Some(body) = root.first("body") {
        for level in ["h1", "h2", "h3"] {
            if let Some(h) = body.find(level) {
                let t = collapse(&h.text());
                if !t.is_empty() {
                    return t;
                }
            }
        }
    }
    if let Some(t) = root.find("title") {
        let t = collapse(&t.text());
        if !t.is_empty() {
            return t;
        }
    }
    String::new()
}

struct Resolved {
    label: String,
    pos: usize,
    fid: usize,
    offset: usize,
    children: Vec<Resolved>,
}

struct Flat {
    label: String,
    depth: usize,
    parent_dfs: Option<usize>,
    children_dfs: Vec<usize>,
    pos: usize,
    fid: usize,
    offset: usize,
}

type Lookup<'a> = dyn Fn(&str, &str) -> Option<(usize, usize, usize)> + 'a;

fn resolve_toc(nodes: &[TocNode], lookup: &Lookup) -> Vec<Resolved> {
    let mut out = Vec::new();
    for n in nodes {
        let children = resolve_toc(&n.children, lookup);
        let target = n
            .target
            .as_ref()
            .and_then(|(p, f)| lookup(p, f))
            .or_else(|| children.first().map(|c| (c.pos, c.fid, c.offset)));
        match target {
            Some((pos, fid, offset)) if !n.title.trim().is_empty() => out.push(Resolved {
                label: n.title.trim().to_string(),
                pos,
                fid,
                offset,
                children,
            }),
            _ => out.extend(children),
        }
    }
    out
}

fn flatten(
    mut nodes: Vec<Resolved>,
    depth: usize,
    parent: Option<usize>,
    out: &mut Vec<Flat>,
) -> Vec<usize> {
    nodes.sort_by_key(|n| n.offset);
    let mut ids = Vec::new();
    for n in nodes {
        let id = out.len();
        ids.push(id);
        out.push(Flat {
            label: n.label,
            depth,
            parent_dfs: parent,
            children_dfs: Vec::new(),
            pos: n.pos,
            fid: n.fid,
            offset: n.offset,
        });
        let children = flatten(n.children, depth + 1, Some(id), out);
        out[id].children_dfs = children;
    }
    ids
}

struct Toc {
    index_entries: Vec<IndexEntry>,
    cncx: Cncx,
    tbs: Vec<TocEntry>,
}

fn build_toc(flat: Vec<Flat>, text_len: usize) -> Toc {
    let mut order: Vec<usize> = (0..flat.len()).collect();
    order.sort_by_key(|&i| (flat[i].depth, flat[i].offset));
    let mut dfs_to_index = vec![0; flat.len()];
    for (idx, &d) in order.iter().enumerate() {
        dfs_to_index[d] = idx;
    }
    let cncx = Cncx::new(order.iter().map(|&d| flat[d].label.as_str()));
    let width = format!("{:X}", flat.len().saturating_sub(1)).len().max(2);
    let mut index_entries = Vec::new();
    let mut tbs = Vec::new();
    for (idx, &d) in order.iter().enumerate() {
        let e = &flat[d];
        let next_start = order
            .iter()
            .map(|&o| &flat[o])
            .filter(|o| o.depth <= e.depth && o.offset > e.offset)
            .map(|o| o.offset)
            .min()
            .unwrap_or(text_len);
        let length = next_start - e.offset;
        let parent = e.parent_dfs.map(|p| dfs_to_index[p]);
        let mut tags: Vec<(&'static str, Vec<u64>)> = vec![
            ("offset", vec![e.offset as u64]),
            ("length", vec![length as u64]),
            ("label", vec![cncx.get(&e.label)]),
            ("depth", vec![e.depth as u64]),
        ];
        if let Some(p) = parent {
            tags.push(("parent", vec![p as u64]));
        }
        if let (Some(first), Some(last)) = (e.children_dfs.first(), e.children_dfs.last()) {
            tags.push(("first_child", vec![dfs_to_index[*first] as u64]));
            tags.push(("last_child", vec![dfs_to_index[*last] as u64]));
        }
        tags.push(("pos_fid", vec![e.pos as u64, e.fid as u64]));
        index_entries.push(IndexEntry {
            lead: format!("{idx:0width$X}").into_bytes(),
            tags,
        });
        tbs.push(TocEntry {
            index: idx,
            offset: e.offset,
            length,
            depth: e.depth,
            parent,
        });
    }
    Toc {
        index_entries,
        cncx,
        tbs,
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn civil_date(secs: u64) -> String {
    let days = (secs / 86400) as i64 + 719468;
    let era = days.div_euclid(146097);
    let doe = days.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(now_secs());
    h.finish()
}

fn random_uuid() -> String {
    let (a, b) = (random_u64(), random_u64());
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        a & 0xfff,
        0x8000 | (b >> 48) as u16 & 0x3fff,
        b & 0xffff_ffff_ffff
    )
}

fn normalize_date(d: &str) -> String {
    let d = d.trim();
    let b = d.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {
        format!("{d}T00:00:00+00:00")
    } else if b.len() == 4 && d.chars().all(|c| c.is_ascii_digit()) {
        format!("{d}-01-01T00:00:00+00:00")
    } else {
        d.to_string()
    }
}

fn exth_meta(epub: &Epub, meta: &Metadata, src: &Path) -> ExthMeta {
    let nonempty = |s: &Option<String>| s.clone().filter(|v| !v.trim().is_empty());
    let title = nonempty(&meta.title)
        .or_else(|| epub.meta.title.clone())
        .or_else(|| src.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let authors: Vec<String> = if meta.authors.is_empty() {
        epub.meta.authors.clone()
    } else {
        meta.authors.clone()
    };
    let isbn = meta
        .identifiers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("isbn"))
        .map(|(_, v)| v.clone())
        .or_else(|| epub.meta.isbn.clone());
    let uuid = nonempty(&meta.uuid)
        .or_else(|| epub.meta.uuid.clone())
        .unwrap_or_else(random_uuid);
    let pubdate = nonempty(&meta.pubdate)
        .or_else(|| epub.meta.date.clone())
        .map(|d| normalize_date(&d))
        .unwrap_or_else(|| format!("{}T00:00:00+00:00", civil_date(now_secs())));
    ExthMeta {
        title,
        authors,
        publisher: nonempty(&meta.publisher).or_else(|| epub.meta.publisher.clone()),
        description: nonempty(&meta.comments).or_else(|| epub.meta.description.clone()),
        isbn,
        subjects: if meta.tags.is_empty() {
            epub.meta.subjects.clone()
        } else {
            meta.tags.clone()
        },
        language: meta
            .languages
            .first()
            .cloned()
            .or_else(|| epub.meta.language.clone()),
        uuid,
        pubdate,
        rtl: epub.rtl,
        writing_mode: epub.meta.writing_mode.clone(),
    }
}

pub fn convert(src: &Path, meta: &Metadata, cover: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut epub = Epub::open(src)?;

    let mut docs: Vec<Doc> = Vec::new();
    for &i in &epub.spine.clone() {
        let href = epub.items[i].href.clone();
        if !is_xhtml(&epub.items[i].media_type) {
            continue;
        }
        let Ok(raw) = epub.read(&href) else { continue };
        docs.push(Doc {
            href,
            root: markup::prepare(dom::parse(&raw)),
        });
    }
    if docs.is_empty() {
        return Err(invalid("the book has no readable content"));
    }

    let cover_item = find_cover_item(&epub, &docs);
    if cover_item.is_some()
        && let Some(g) = epub.guide.iter().find(|g| g.kind == "cover").cloned()
        && docs.len() > 1
        && let Some(pos) = docs.iter().position(|d| d.href == g.href)
        && markup::body_text_is_empty(&docs[pos].root)
    {
        docs.remove(pos);
        epub.guide.retain(|x| x.kind != "cover");
    }
    let titles: Vec<(String, String)> = docs
        .iter()
        .map(|d| (d.href.clone(), doc_title(&d.root)))
        .collect();
    let spine_hrefs: HashSet<String> = docs.iter().map(|d| d.href.clone()).collect();

    let mut ctx = Ctx::default();
    ctx.flows.push(Vec::new());
    let mut res = build_resources(&mut epub, &mut ctx, cover_item, cover);

    let mut sheets = Vec::new();
    let mut svg_items = Vec::new();
    let items = epub.items.clone();
    for item in &items {
        match item.media_type.as_str() {
            "text/css" => {
                if let Ok(raw) = epub.read(&item.href) {
                    sheets.push((item.href.clone(), dom::decode_bytes(&raw)));
                }
            }
            "image/svg+xml" => {
                if let Ok(raw) = epub.read(&item.href) {
                    svg_items.push((item.href.clone(), dom::parse(&raw)));
                }
            }
            _ => {}
        }
    }

    for d in &mut docs {
        markup::remove_duplicate_anchors(&mut d.root);
    }
    for d in &mut docs {
        markup::replace_resource_links(&mut ctx, d);
    }
    markup::extract_css_into_flows(&mut ctx, sheets, &mut docs);
    markup::extract_svg_into_flows(&mut ctx, svg_items, &mut docs);
    markup::replace_internal_links(&mut ctx, &mut docs, &spine_hrefs);
    markup::insert_aid_attributes(&mut ctx, &mut docs);

    let placeholders: HashMap<String, String> = ctx
        .link_map
        .iter()
        .filter_map(|(ph, (href, frag))| {
            let aid = ctx
                .id_map
                .get(&(href.clone(), frag.clone()))
                .or_else(|| ctx.id_map.get(&(href.clone(), String::new())))?;
            Some((ph.clone(), aid.clone()))
        })
        .collect();
    let mut roots: Vec<Elem> = docs.into_iter().map(|d| d.root).collect();
    for r in &mut roots {
        dom::merge_text(r);
    }
    let Chunked {
        text,
        skel_table,
        chunk_table,
        aid_offsets,
    } = skeleton::chunk(&mut roots, &placeholders)?;
    let text_len = text.len();
    ctx.flows[0] = text;

    for (rec, href) in &res.image_records {
        if !ctx.used_images.contains(href) && !res.keep.contains(rec) {
            res.records[*rec] = image::PLACEHOLDER_GIF.to_vec();
        }
    }

    let locate = |path: &str, frag: &str| -> Option<(usize, usize, usize)> {
        let aid = ctx
            .id_map
            .get(&(path.to_string(), frag.to_string()))
            .or_else(|| ctx.id_map.get(&(path.to_string(), String::new())))?;
        aid_offsets.get(aid).copied()
    };

    let mut resolved = resolve_toc(&epub.toc, &locate);
    if resolved.is_empty() {
        for (href, title) in &titles {
            let Some((pos, fid, offset)) = locate(href, "") else {
                continue;
            };
            let label = if title.is_empty() {
                format!("Section {}", resolved.len() + 1)
            } else {
                title.clone()
            };
            resolved.push(Resolved {
                label,
                pos,
                fid,
                offset,
                children: Vec::new(),
            });
        }
    }
    let mut flat = Vec::new();
    flatten(resolved, 0, None, &mut flat);
    let toc = build_toc(flat, text_len);

    let mut guide: Vec<(String, String, usize, usize)> = Vec::new();
    let mut start_offset = None;
    for g in &epub.guide {
        let Some((pos, fid, offset)) = locate(&g.href, &g.frag) else {
            continue;
        };
        if g.title.eq_ignore_ascii_case("start")
            || matches!(g.kind.as_str(), "start" | "other.start" | "text")
        {
            start_offset = Some(offset as u32);
        }
        let title = if g.title.is_empty() {
            "Unknown".to_string()
        } else {
            g.title.clone()
        };
        guide.push((title, g.kind.clone(), pos, fid));
    }
    guide.sort_by(|a, b| a.1.cmp(&b.1));

    let skel_entries: Vec<IndexEntry> = skel_table
        .iter()
        .map(|s| IndexEntry {
            lead: s.name.clone().into_bytes(),
            tags: vec![
                ("chunk_count", vec![s.chunk_count as u64; 2]),
                (
                    "geometry",
                    vec![
                        s.start_pos as u64,
                        s.length as u64,
                        s.start_pos as u64,
                        s.length as u64,
                    ],
                ),
            ],
        })
        .collect();
    let skel_records = build_index(SKEL_TAGS, &skel_entries, &Cncx::new(std::iter::empty()))?;

    let chunk_cncx = Cncx::new(chunk_table.iter().map(|c| c.selector.as_str()));
    let chunk_entries: Vec<IndexEntry> = chunk_table
        .iter()
        .map(|c| IndexEntry {
            lead: format!("{:010}", c.insert_pos).into_bytes(),
            tags: vec![
                ("cncx_offset", vec![chunk_cncx.get(&c.selector)]),
                ("file_number", vec![c.file_number as u64]),
                ("sequence_number", vec![c.sequence_number as u64]),
                ("geometry", vec![c.start_pos as u64, c.length as u64]),
            ],
        })
        .collect();
    let chunk_records = build_index(CHUNK_TAGS, &chunk_entries, &chunk_cncx)?;

    let guide_records = if guide.is_empty() {
        Vec::new()
    } else {
        let cncx = Cncx::new(guide.iter().map(|g| g.0.as_str()));
        let entries: Vec<IndexEntry> = guide
            .iter()
            .map(|(title, kind, pos, fid)| IndexEntry {
                lead: kind.clone().into_bytes(),
                tags: vec![
                    ("title", vec![cncx.get(title)]),
                    ("pos_fid", vec![*pos as u64, *fid as u64]),
                ],
            })
            .collect();
        build_index(GUIDE_TAGS, &entries, &cncx)?
    };
    let ncx_records = if toc.index_entries.is_empty() {
        Vec::new()
    } else {
        build_index(NCX_TAGS, &toc.index_entries, &toc.cncx)?
    };

    let now = now_secs();
    let exth = exth_meta(&epub, meta, src);
    let parts = Parts {
        flows: ctx.flows,
        chunk_records,
        skel_records,
        guide_records,
        ncx_records,
        toc: toc.tbs,
        resources: res.records,
        cover_offset: res.cover_offset.map(|c| c as u32),
        thumbnail_offset: res.thumbnail_offset.map(|c| c as u32),
        has_fonts: res.has_fonts,
        start_offset,
        meta: exth,
    };
    writer::assemble(parts, now as u32, random_u64() as u32)
}

pub fn convert_to_file(
    src: &Path,
    dst: &Path,
    meta: &Metadata,
    cover: Option<&[u8]>,
) -> Result<()> {
    let bytes = convert(src, meta, cover)?;
    let mut tmp_name = dst
        .file_name()
        .ok_or_else(|| Error::Internal("destination has no file name".into()))?
        .to_os_string();
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = dst.with_file_name(tmp_name);
    let result = std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, dst));
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}
