use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use shelfmark_model::Metadata;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{Result, invalid};
use crate::lang;
use crate::util::{self, clean, normalize_date, push_unique, sniff_image};
use crate::xml::{self, Document, Element, Node};

const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
const OPF_NS: &str = "http://www.idpf.org/2007/opf";

type Zip = ZipArchive<File>;

fn open(path: &Path) -> Result<Zip> {
    Ok(ZipArchive::new(File::open(path)?)?)
}

fn read_entry(zip: &mut Zip, name: &str) -> Option<Vec<u8>> {
    let real = if zip.by_name(name).is_ok() {
        name.to_string()
    } else {
        let lower = name.to_lowercase();
        zip.file_names()
            .find(|n| n.to_lowercase() == lower)?
            .to_string()
    };
    let mut f = zip.by_name(&real).ok()?;
    let mut buf = Vec::with_capacity(f.size().min(1 << 28) as usize);
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resolve(dir: &str, href: &str) -> String {
    let href = href.split(['#', '?']).next().unwrap_or(href);
    let href = percent_decode(href);
    let mut parts: Vec<&str> = if href.starts_with('/') {
        Vec::new()
    } else {
        dir.split('/').filter(|p| !p.is_empty()).collect()
    };
    for p in href.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

fn dir_of(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default()
}

struct Opf {
    path: String,
    dir: String,
    doc: Document,
}

fn load_opf(zip: &mut Zip) -> Result<Opf> {
    let mut opf_path = None;
    if let Some(container) = read_entry(zip, "META-INF/container.xml").and_then(|b| xml::parse(&b))
    {
        let mut roots = Vec::new();
        container.root.find_all("rootfile", &mut roots);
        let pick = roots
            .iter()
            .find(|r| {
                r.attr("media-type")
                    .is_some_and(|m| m.contains("oebps-package"))
            })
            .or(roots.first());
        opf_path = pick
            .and_then(|r| r.attr("full-path"))
            .map(|p| resolve("", p));
    }
    let opf_path = match opf_path.filter(|p| zip.file_names().any(|n| n.eq_ignore_ascii_case(p))) {
        Some(p) => p,
        None => zip
            .file_names()
            .filter(|n| n.to_lowercase().ends_with(".opf"))
            .min_by_key(|n| n.matches('/').count())
            .map(str::to_string)
            .ok_or_else(|| invalid("epub", "no package document"))?,
    };
    let bytes =
        read_entry(zip, &opf_path).ok_or_else(|| invalid("epub", "unreadable package document"))?;
    let doc = xml::parse(&bytes).ok_or_else(|| invalid("epub", "malformed package document"))?;
    Ok(Opf {
        dir: dir_of(&opf_path),
        path: opf_path,
        doc,
    })
}

fn metadata_children(root: &Element) -> Vec<&Element> {
    let mut out = Vec::new();
    if let Some(md) = root.child("metadata") {
        for e in md.elements() {
            if e.is("dc-metadata") || e.is("x-metadata") {
                out.extend(e.elements());
            } else {
                out.push(e);
            }
        }
    }
    out
}

fn refines_map(children: &[&Element]) -> HashMap<String, Vec<(String, String)>> {
    let mut map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for e in children.iter().filter(|e| e.is("meta")) {
        if let (Some(r), Some(p)) = (e.attr("refines"), e.attr("property")) {
            map.entry(r.trim_start_matches('#').to_string())
                .or_default()
                .push((p.to_string(), e.trimmed_text()));
        }
    }
    map
}

fn refinement<'a>(
    map: &'a HashMap<String, Vec<(String, String)>>,
    el: &Element,
    prop: &str,
) -> Option<&'a str> {
    let id = el.attr("id")?;
    map.get(id)?
        .iter()
        .find(|(p, v)| p == prop && !v.is_empty())
        .map(|(_, v)| v.as_str())
}

fn is_isbn(s: &str) -> bool {
    let c: String = s
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .collect();
    match c.len() {
        10 => {
            c[..9].chars().all(|d| d.is_ascii_digit())
                && c[9..]
                    .chars()
                    .all(|d| d.is_ascii_digit() || d == 'X' || d == 'x')
        }
        13 => {
            c.chars().all(|d| d.is_ascii_digit()) && (c.starts_with("978") || c.starts_with("979"))
        }
        _ => false,
    }
}

fn is_uuid(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    p.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(&p)
            .all(|(n, s)| s.len() == *n && s.chars().all(|c| c.is_ascii_hexdigit()))
}

fn classify_identifier(el: &Element) -> Option<(String, String)> {
    let value = el.trimmed_text();
    if value.is_empty() {
        return None;
    }
    let mut scheme = el
        .attr("scheme")
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let mut v = value.clone();
    if let Some(s) = &scheme {
        let pfx = format!("{s}:");
        let low = v.to_lowercase();
        let urn = format!("urn:{s}:");
        if low.starts_with(&urn) {
            v = v[urn.len()..].to_string();
        } else if low.starts_with(&pfx) {
            v = v[pfx.len()..].to_string();
        }
    } else {
        let stripped = if v.len() > 4 && v[..4].eq_ignore_ascii_case("urn:") {
            &v[4..]
        } else {
            v.as_str()
        };
        match stripped.split_once(':') {
            Some((s, rest))
                if !s.is_empty()
                    && s.len() <= 20
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                    && !rest.is_empty()
                    && !matches!(s.to_lowercase().as_str(), "http" | "https" | "urn") =>
            {
                scheme = Some(s.to_lowercase());
                v = rest.to_string();
            }
            _ => {
                if is_isbn(stripped) {
                    scheme = Some("isbn".into());
                    v = stripped.to_string();
                } else if is_uuid(stripped)
                    || el
                        .attr("id")
                        .is_some_and(|i| i.to_lowercase().contains("uuid"))
                {
                    scheme = Some("uuid".into());
                    v = stripped.to_string();
                }
            }
        }
    }
    let scheme = scheme?;
    let v = v.trim().to_string();
    if v.is_empty() {
        return None;
    }
    if scheme == "isbn" {
        return Some((
            scheme,
            v.chars()
                .filter(|c| *c != '-' && !c.is_whitespace())
                .collect(),
        ));
    }
    Some((scheme, v))
}

fn meta_content<'a>(children: &[&'a Element], name: &str) -> Option<&'a str> {
    children
        .iter()
        .filter(|e| e.is("meta") && e.attr("name") == Some(name))
        .find_map(|e| e.attr("content").map(str::trim).filter(|s| !s.is_empty()))
}

fn parse_metadata(root: &Element) -> Metadata {
    let kids = metadata_children(root);
    let refines = refines_map(&kids);
    let mut m = Metadata::default();

    let titles: Vec<&&Element> = kids.iter().filter(|e| e.is("title")).collect();
    let main = titles
        .iter()
        .find(|t| refinement(&refines, t, "title-type") == Some("main"))
        .or(titles.first());
    if let Some(t) = main {
        m.title = clean(&t.text());
        m.title_sort = refinement(&refines, t, "file-as").and_then(clean);
    }
    if m.title_sort.is_none() {
        m.title_sort = meta_content(&kids, "calibre:title_sort").and_then(clean);
    }

    let mut file_as = Vec::new();
    for c in kids.iter().filter(|e| e.is("creator")) {
        let role = c
            .attr("role")
            .map(str::to_string)
            .or_else(|| refinement(&refines, c, "role").map(str::to_string));
        if role
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty() && !r.trim().eq_ignore_ascii_case("aut"))
        {
            continue;
        }
        if let Some(name) = clean(&c.text())
            && !m.authors.contains(&name)
        {
            m.authors.push(name);
            let fa = c
                .attr("file-as")
                .map(str::to_string)
                .or_else(|| refinement(&refines, c, "file-as").map(str::to_string))
                .and_then(|s| clean(&s));
            file_as.push(fa);
        }
    }
    let present: Vec<&String> = file_as.iter().flatten().collect();
    if !present.is_empty() {
        m.author_sort =
            if present.len() == m.authors.len() || m.authors.len() > 1 && present.len() > 1 {
                Some(
                    present
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" & "),
                )
            } else {
                Some(present[0].clone())
            };
    }

    for s in kids.iter().filter(|e| e.is("subject")) {
        if let Some(t) = clean(&s.text()) {
            push_unique(&mut m.tags, t);
        }
    }
    m.publisher = kids
        .iter()
        .find(|e| e.is("publisher"))
        .and_then(|e| clean(&e.text()));
    m.pubdate = kids
        .iter()
        .filter(|e| e.is("date") && e.attr("event").is_none_or(|ev| ev != "modification"))
        .find_map(|e| normalize_date(&e.text()));
    m.languages = lang::normalize_all(
        kids.iter()
            .filter(|e| e.is("language"))
            .map(|e| e.text())
            .collect::<Vec<_>>()
            .iter()
            .map(|s| s.as_str()),
    );
    m.comments = kids.iter().find(|e| e.is("description")).and_then(|e| {
        let s = if e.elements().next().is_some() {
            e.inner_xml()
        } else {
            e.text()
        };
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_string())
    });

    let uid_ref = root.attr("unique-identifier");
    let mut uuid_unique = None;
    let mut uuid_first = None;
    let mut uuid_calibre = None;
    for e in kids.iter().filter(|e| e.is("identifier")) {
        let Some((scheme, value)) = classify_identifier(e) else {
            continue;
        };
        match scheme.as_str() {
            "uuid" => {
                if e.attr("id").is_some() && e.attr("id") == uid_ref {
                    uuid_unique.get_or_insert(value);
                } else {
                    uuid_first.get_or_insert(value);
                }
            }
            "calibre" => {
                uuid_calibre.get_or_insert(value);
            }
            _ => {
                m.identifiers.entry(scheme).or_insert(value);
            }
        }
    }
    m.uuid = uuid_unique.or(uuid_first).or(uuid_calibre);

    m.series = meta_content(&kids, "calibre:series").and_then(clean);
    m.series_index = meta_content(&kids, "calibre:series_index").and_then(|s| s.parse().ok());
    if m.series.is_none() {
        for c in kids
            .iter()
            .filter(|e| e.is("meta") && e.attr("property") == Some("belongs-to-collection"))
        {
            let ty = refinement(&refines, c, "collection-type");
            if ty.is_none_or(|t| t == "series")
                && let Some(name) = clean(&c.text())
            {
                m.series = Some(name);
                m.series_index =
                    refinement(&refines, c, "group-position").and_then(|s| s.trim().parse().ok());
                break;
            }
        }
    }
    m.rating = meta_content(&kids, "calibre:rating")
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|r| *r > 0.0)
        .map(|r| r.round().clamp(0.0, 10.0) as u8);
    m
}

struct Item<'a> {
    el: &'a Element,
}

impl Item<'_> {
    fn id(&self) -> &str {
        self.el.attr("id").unwrap_or("")
    }
    fn href(&self) -> &str {
        self.el.attr("href").unwrap_or("")
    }
    fn is_image(&self) -> bool {
        match self.el.attr("media-type") {
            Some(mt) if !mt.is_empty() => mt.to_lowercase().starts_with("image/"),
            _ => util::is_image_name(self.href()),
        }
    }
}

fn manifest_items(root: &Element) -> Vec<Item<'_>> {
    root.child("manifest")
        .map(|m| m.children_named("item").map(|el| Item { el }).collect())
        .unwrap_or_default()
}

fn cover_item(root: &Element) -> Option<&Element> {
    let items = manifest_items(root);
    let by_id = |id: &str| items.iter().find(|i| i.id() == id);
    if let Some(i) = items.iter().find(|i| {
        i.is_image()
            && i.el.attr("properties").is_some_and(|p| {
                p.split_whitespace()
                    .any(|x| x.eq_ignore_ascii_case("cover-image"))
            })
    }) {
        return Some(i.el);
    }
    let kids = metadata_children(root);
    if let Some(cid) = meta_content(&kids, "cover")
        && let Some(i) = by_id(cid)
            .or_else(|| items.iter().find(|i| i.href() == cid))
            .filter(|i| i.is_image())
    {
        return Some(i.el);
    }
    if let Some(guide) = root.child("guide") {
        for ty in [
            "cover",
            "other.ms-coverimage-standard",
            "other.ms-coverimage",
        ] {
            for r in guide.children_named("reference") {
                if r.attr("type").is_some_and(|t| t.eq_ignore_ascii_case(ty)) {
                    let href = r.attr("href").unwrap_or("").split('#').next().unwrap_or("");
                    if let Some(i) = items.iter().find(|i| i.href() == href && i.is_image()) {
                        return Some(i.el);
                    }
                }
            }
        }
    }
    let named = |i: &&Item| {
        let stem = i.href().rsplit('/').next().unwrap_or("").to_lowercase();
        (i.id().to_lowercase().contains("cover") || stem.contains("cover"))
            && !stem.contains("thumb")
    };
    let candidates: Vec<&Item> = items
        .iter()
        .filter(|i| i.is_image())
        .filter(named)
        .collect();
    candidates
        .iter()
        .find(|i| {
            let id = i.id().to_lowercase();
            id == "cover" || id == "cover-image" || id == "cover_image" || id == "coverimage"
        })
        .or(candidates.first())
        .map(|i| i.el)
}

fn html_cover_hrefs(root: &Element) -> Vec<String> {
    let items = manifest_items(root);
    let mut out = Vec::new();
    let kids = metadata_children(root);
    if let Some(cid) = meta_content(&kids, "cover")
        && let Some(i) = items.iter().find(|i| i.id() == cid && !i.is_image())
    {
        out.push(i.href().to_string());
    }
    if let Some(guide) = root.child("guide") {
        for r in guide.children_named("reference") {
            if r.attr("type")
                .is_some_and(|t| t.eq_ignore_ascii_case("cover"))
                && let Some(h) = r.attr("href")
            {
                out.push(h.split('#').next().unwrap_or(h).to_string());
            }
        }
    }
    if out.is_empty()
        && let Some(first) = root
            .child("spine")
            .and_then(|s| s.children_named("itemref").next())
            .and_then(|r| r.attr("idref"))
            .and_then(|id| items.iter().find(|i| i.id() == id))
    {
        out.push(first.href().to_string());
    }
    out
}

fn image_from_html(zip: &mut Zip, html_path: &str) -> Option<Vec<u8>> {
    let doc = xml::parse(&read_entry(zip, html_path)?)?;
    let mut imgs = Vec::new();
    doc.root.find_all("img", &mut imgs);
    doc.root.find_all("image", &mut imgs);
    let src = imgs
        .iter()
        .find_map(|e| e.attr("src").or_else(|| e.attr("href")))?;
    let path = resolve(&dir_of(html_path), src);
    read_entry(zip, &path).filter(|d| sniff_image(d).is_some())
}

pub fn read_metadata(path: &Path) -> Result<Metadata> {
    let mut zip = open(path)?;
    let opf = load_opf(&mut zip)?;
    Ok(parse_metadata(&opf.doc.root))
}

pub fn read_cover(path: &Path) -> Result<Option<Vec<u8>>> {
    let mut zip = open(path)?;
    let opf = load_opf(&mut zip)?;
    let root = &opf.doc.root;
    if let Some(item) = cover_item(root) {
        let p = resolve(&opf.dir, item.attr("href").unwrap_or(""));
        if let Some(d) = read_entry(&mut zip, &p).filter(|d| sniff_image(d).is_some()) {
            return Ok(Some(d));
        }
    }
    for href in html_cover_hrefs(root) {
        let p = resolve(&opf.dir, &href);
        if let Some(d) = image_from_html(&mut zip, &p) {
            return Ok(Some(d));
        }
    }
    Ok(None)
}

struct Ctx {
    dc: String,
    meta_name: String,
    opf_attr: String,
    v3: bool,
    indent: String,
}

fn find_prefix(el: &Element, uri: &str, need_prefix: bool) -> Option<String> {
    el.attrs.iter().find_map(|(k, v)| {
        if v != uri {
            None
        } else if k == "xmlns" {
            (!need_prefix).then(String::new)
        } else {
            k.strip_prefix("xmlns:").map(str::to_string)
        }
    })
}

fn ensure_ns(root: &mut Element, uri: &str, preferred: &str, need_prefix: bool) -> String {
    if let Some(p) = find_prefix(root, uri, need_prefix) {
        return p;
    }
    if let Some(md) = root.child("metadata")
        && let Some(p) = find_prefix(md, uri, need_prefix)
    {
        return p;
    }
    root.set_attr(&format!("xmlns:{preferred}"), uri);
    preferred.to_string()
}

fn collect_ids(el: &Element, out: &mut HashSet<String>) {
    if let Some(id) = el.attr("id") {
        out.insert(id.to_string());
    }
    for c in el.elements() {
        collect_ids(c, out);
    }
}

fn unique_id(used: &mut HashSet<String>, base: &str) -> String {
    let mut id = base.to_string();
    let mut n = 1;
    while used.contains(&id) {
        n += 1;
        id = format!("{base}-{n}");
    }
    used.insert(id.clone());
    id
}

fn is_ws(n: &Node) -> bool {
    matches!(n, Node::Text(t) if t.trim().is_empty())
}

fn remove_where(md: &mut Element, mut pred: impl FnMut(&Element) -> bool) -> usize {
    let mut removed = 0;
    let mut i = 0;
    while i < md.children.len() {
        let hit = matches!(&md.children[i], Node::Element(e) if pred(e));
        if hit {
            md.children.remove(i);
            removed += 1;
            if i > 0 && is_ws(&md.children[i - 1]) {
                md.children.remove(i - 1);
                i -= 1;
            }
        } else {
            i += 1;
        }
    }
    removed
}

fn append(md: &mut Element, ctx: &Ctx, el: Element) {
    let ws = Node::Text(format!("\n{}", ctx.indent));
    let at = if md.children.last().is_some_and(is_ws) {
        md.children.len() - 1
    } else {
        md.children.len()
    };
    md.children.insert(at, Node::Element(el));
    md.children.insert(at, ws);
    if at == md.children.len() - 2 {
        md.children.push(Node::Text("\n  ".into()));
    }
}

impl Ctx {
    fn dc(&self, name: &str) -> Element {
        Element::new(format!("{}{}", self.dc, name))
    }
    fn meta(&self, name: &str, content: &str) -> Element {
        Element::new(&self.meta_name)
            .with_attr("name", name)
            .with_attr("content", content)
    }
    fn prop(&self, refines: &str, property: &str, value: &str) -> Element {
        Element::new(&self.meta_name)
            .with_attr("refines", format!("#{refines}"))
            .with_attr("property", property)
            .with_text(value)
    }
    fn opf_attr(&self, name: &str) -> String {
        if self.opf_attr.is_empty() {
            name.to_string()
        } else {
            format!("{}:{name}", self.opf_attr)
        }
    }
}

fn fmt_num(n: f64) -> String {
    format!("{n:?}")
}

fn rewrap(old: &str, new: &str) -> String {
    let lower = old.to_lowercase();
    if let Some(rest) = lower.strip_prefix("urn:")
        && let Some((s, _)) = rest.split_once(':')
    {
        return format!("{}{s}:{new}", &old[..4]);
    }
    if let Some((s, _)) = old.split_once(':')
        && s.len() <= 20
        && !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return format!("{s}:{new}");
    }
    new.to_string()
}

fn flatten_dc_metadata(md: &mut Element) {
    let mut out = Vec::new();
    for n in std::mem::take(&mut md.children) {
        match n {
            Node::Element(e) if e.is("dc-metadata") || e.is("x-metadata") => {
                out.extend(e.children.into_iter().filter(|c| !is_ws(c)));
            }
            n => out.push(n),
        }
    }
    md.children = out;
}

fn apply_metadata(root: &mut Element, meta: &Metadata) {
    let v3 = root
        .attr("version")
        .is_some_and(|v| v.trim().starts_with('3'));
    if root.child("metadata").is_none() {
        root.children
            .insert(0, Node::Element(Element::new("metadata")));
    }
    let opf_attr = if v3 {
        String::new()
    } else {
        ensure_ns(root, OPF_NS, "opf", true)
    };
    let dc_known = root
        .child("metadata")
        .and_then(|md| {
            md.elements()
                .find(|e| e.local() == "title" && xml::prefix(&e.name).is_some())
        })
        .and_then(|e| xml::prefix(&e.name).map(str::to_string));
    let dc = dc_known.unwrap_or_else(|| ensure_ns(root, DC_NS, "dc", false));
    let dc = if dc.is_empty() {
        String::new()
    } else {
        format!("{dc}:")
    };
    let uid_ref = root.attr("unique-identifier").map(str::to_string);
    let mut used = HashSet::new();
    collect_ids(root, &mut used);

    let md = root.child_mut("metadata").expect("metadata exists");
    flatten_dc_metadata(md);
    let meta_name = {
        let first_meta = md.elements().find(|e| e.is("meta")).map(|e| e.name.clone());
        first_meta.unwrap_or_else(|| match xml::prefix(&md.name) {
            Some(p) => format!("{p}:meta"),
            None => "meta".into(),
        })
    };
    let indent = md
        .children
        .iter()
        .find_map(|n| match n {
            Node::Text(t) if t.contains('\n') && t.trim().is_empty() => {
                t.rsplit_once('\n').map(|(_, i)| i.to_string())
            }
            _ => None,
        })
        .unwrap_or_else(|| "    ".into());
    let ctx = Ctx {
        dc,
        meta_name,
        opf_attr,
        v3,
        indent,
    };

    write_title(md, &ctx, meta, &mut used);
    write_creators(md, &ctx, meta, &mut used);

    remove_where(md, |e| e.is("subject"));
    for t in &meta.tags {
        append(md, &ctx, ctx.dc("subject").with_text(t.as_str()));
    }
    remove_where(md, |e| e.is("publisher"));
    if let Some(p) = &meta.publisher {
        append(md, &ctx, ctx.dc("publisher").with_text(p.as_str()));
    }
    remove_where(md, |e| {
        e.is("date") && e.attr("event").is_none_or(|ev| ev != "modification")
    });
    if let Some(d) = &meta.pubdate {
        append(md, &ctx, ctx.dc("date").with_text(d.as_str()));
    }
    if !meta.languages.is_empty() {
        remove_where(md, |e| e.is("language"));
        for l in &meta.languages {
            append(md, &ctx, ctx.dc("language").with_text(l.as_str()));
        }
    }
    remove_where(md, |e| e.is("description"));
    if let Some(c) = meta.comments.as_ref().filter(|c| !c.trim().is_empty()) {
        append(md, &ctx, ctx.dc("description").with_text(c.as_str()));
    }

    let new_uid = write_identifiers(md, &ctx, meta, uid_ref.as_deref(), &mut used);
    write_series(md, &ctx, meta, &mut used);

    remove_where(md, |e| {
        e.is("meta") && e.attr("name") == Some("calibre:rating")
    });
    if let Some(r) = meta.rating {
        append(md, &ctx, ctx.meta("calibre:rating", &format!("{r}.0")));
    }
    if let Some(id) = new_uid {
        root.set_attr("unique-identifier", id);
    }
}

fn ids_of(md: &Element, mut pred: impl FnMut(&Element) -> bool) -> Vec<String> {
    md.elements()
        .filter(|e| pred(e))
        .filter_map(|e| e.attr("id").map(str::to_string))
        .collect()
}

fn remove_refinements(md: &mut Element, ids: &[String], props: &[&str]) {
    if ids.is_empty() {
        return;
    }
    remove_where(md, |e| {
        e.is("meta")
            && e.attr("refines")
                .is_some_and(|r| ids.iter().any(|i| r.trim_start_matches('#') == i))
            && (props.is_empty() || e.attr("property").is_some_and(|p| props.contains(&p)))
    });
}

fn write_title(md: &mut Element, ctx: &Ctx, meta: &Metadata, used: &mut HashSet<String>) {
    let refines = refines_map(&md.elements().collect::<Vec<_>>());
    let main_idx = {
        let titles: Vec<(usize, &Element)> = md
            .children
            .iter()
            .enumerate()
            .filter_map(|(i, n)| match n {
                Node::Element(e) if e.is("title") => Some((i, e)),
                _ => None,
            })
            .collect();
        titles
            .iter()
            .find(|(_, t)| refinement(&refines, t, "title-type") == Some("main"))
            .or(titles.first())
            .map(|(i, _)| *i)
    };
    let title_id = match (main_idx, &meta.title) {
        (Some(i), Some(t)) => {
            let Node::Element(e) = &mut md.children[i] else {
                unreachable!()
            };
            e.set_text(t.as_str());
            if e.attr("id").is_none() && ctx.v3 && meta.title_sort.is_some() {
                e.set_attr("id", unique_id(used, "title"));
            }
            e.attr("id").map(str::to_string)
        }
        (None, Some(t)) => {
            let mut e = ctx.dc("title").with_text(t.as_str());
            if ctx.v3 && meta.title_sort.is_some() {
                e.set_attr("id", unique_id(used, "title"));
            }
            let id = e.attr("id").map(str::to_string);
            append(md, ctx, e);
            id
        }
        (Some(i), None) => match &md.children[i] {
            Node::Element(e) => e.attr("id").map(str::to_string),
            _ => None,
        },
        (None, None) => None,
    };
    remove_where(md, |e| {
        e.is("meta") && e.attr("name") == Some("calibre:title_sort")
    });
    if let Some(id) = &title_id {
        remove_refinements(md, std::slice::from_ref(id), &["file-as"]);
    }
    if let Some(ts) = &meta.title_sort {
        append(md, ctx, ctx.meta("calibre:title_sort", ts));
        if let (true, Some(id)) = (ctx.v3, &title_id) {
            append(md, ctx, ctx.prop(id, "file-as", ts));
        }
    }
}

fn write_creators(md: &mut Element, ctx: &Ctx, meta: &Metadata, used: &mut HashSet<String>) {
    let refines = refines_map(&md.elements().collect::<Vec<_>>());
    let is_author = |e: &Element| {
        let role = e
            .attr("role")
            .map(str::to_string)
            .or_else(|| refinement(&refines, e, "role").map(str::to_string));
        e.is("creator")
            && role.is_none_or(|r| r.trim().is_empty() || r.trim().eq_ignore_ascii_case("aut"))
    };
    let ids = ids_of(md, |e| is_author(e));
    remove_where(md, |e| is_author(e));
    remove_refinements(md, &ids, &["role", "file-as"]);

    let sorts: Vec<Option<String>> = match &meta.author_sort {
        None => vec![None; meta.authors.len()],
        Some(s) => {
            let parts: Vec<&str> = s.split(" & ").collect();
            if parts.len() == meta.authors.len() {
                parts.iter().map(|p| Some(p.to_string())).collect()
            } else {
                let mut v = vec![None; meta.authors.len()];
                if let Some(f) = v.first_mut() {
                    *f = Some(s.clone());
                }
                v
            }
        }
    };
    for (name, sort) in meta.authors.iter().zip(sorts) {
        let mut e = ctx.dc("creator").with_text(name.as_str());
        if ctx.v3 {
            let id = unique_id(used, "creator");
            e.set_attr("id", id.as_str());
            append(md, ctx, e);
            append(
                md,
                ctx,
                ctx.prop(&id, "role", "aut")
                    .with_attr("scheme", "marc:relators"),
            );
            if let Some(s) = sort {
                append(md, ctx, ctx.prop(&id, "file-as", &s));
            }
        } else {
            e.set_attr(&ctx.opf_attr("role"), "aut");
            if let Some(s) = sort {
                e.set_attr(&ctx.opf_attr("file-as"), s);
            }
            append(md, ctx, e);
        }
    }
}

fn identifier_element(ctx: &Ctx, scheme: &str, value: &str) -> Element {
    let e = ctx.dc("identifier");
    if ctx.v3 {
        if scheme == "isbn" || scheme == "uuid" {
            e.with_text(format!("urn:{scheme}:{value}"))
        } else {
            e.with_text(format!("{scheme}:{value}"))
        }
    } else {
        e.with_attr(&ctx.opf_attr("scheme"), scheme.to_uppercase())
            .with_text(value)
    }
}

fn write_identifiers(
    md: &mut Element,
    ctx: &Ctx,
    meta: &Metadata,
    uid_ref: Option<&str>,
    used: &mut HashSet<String>,
) -> Option<String> {
    remove_where(md, |e| {
        e.is("identifier") && !(e.attr("id").is_some() && e.attr("id") == uid_ref)
    });
    let mut kept_scheme = None;
    let mut new_uid = None;
    let mut uuid_written = false;
    let kept = md.children.iter_mut().find_map(|n| match n {
        Node::Element(e) if e.is("identifier") => Some(e),
        _ => None,
    });
    match kept {
        Some(e) => {
            if let Some((scheme, value)) = classify_identifier(e) {
                let wanted = if scheme == "uuid" || scheme == "calibre" {
                    uuid_written = true;
                    meta.uuid.as_ref()
                } else {
                    meta.identifiers.get(&scheme)
                };
                if let Some(w) = wanted.filter(|w| **w != value) {
                    let old = e.trimmed_text();
                    e.set_text(rewrap(&old, w));
                }
                kept_scheme = Some(scheme);
            }
        }
        None => {
            let uuid = meta.uuid.clone().unwrap_or_else(util::random_uuid);
            let id = unique_id(used, "uuid_id");
            let mut e = identifier_element(ctx, "uuid", &uuid);
            e.set_attr("id", id.as_str());
            append(md, ctx, e);
            new_uid = Some(id);
            uuid_written = true;
        }
    }
    if let (Some(u), false) = (&meta.uuid, uuid_written) {
        append(md, ctx, identifier_element(ctx, "uuid", u));
    }
    let known: BTreeMap<&String, &String> = meta.identifiers.iter().collect();
    for (scheme, value) in known {
        let s = scheme.trim().to_lowercase();
        if s.is_empty()
            || value.trim().is_empty()
            || s == "uuid"
            || kept_scheme.as_deref() == Some(s.as_str())
        {
            continue;
        }
        append(md, ctx, identifier_element(ctx, &s, value.trim()));
    }
    new_uid
}

fn write_series(md: &mut Element, ctx: &Ctx, meta: &Metadata, used: &mut HashSet<String>) {
    remove_where(md, |e| {
        e.is("meta")
            && matches!(
                e.attr("name"),
                Some("calibre:series" | "calibre:series_index")
            )
    });
    let refines = refines_map(&md.elements().collect::<Vec<_>>());
    let coll_ids = ids_of(md, |e| {
        e.is("meta")
            && e.attr("property") == Some("belongs-to-collection")
            && refinement(&refines, e, "collection-type").is_none_or(|t| t == "series")
    });
    remove_where(md, |e| {
        e.is("meta")
            && e.attr("property") == Some("belongs-to-collection")
            && e.attr("id")
                .is_some_and(|i| coll_ids.iter().any(|c| c == i))
    });
    remove_refinements(md, &coll_ids, &["collection-type", "group-position"]);
    let Some(series) = &meta.series else { return };
    append(md, ctx, ctx.meta("calibre:series", series));
    if let Some(i) = meta.series_index {
        append(md, ctx, ctx.meta("calibre:series_index", &fmt_num(i)));
    }
    if ctx.v3 {
        let id = unique_id(used, "series");
        append(
            md,
            ctx,
            Element::new(&ctx.meta_name)
                .with_attr("property", "belongs-to-collection")
                .with_attr("id", id.as_str())
                .with_text(series.as_str()),
        );
        append(md, ctx, ctx.prop(&id, "collection-type", "series"));
        if let Some(i) = meta.series_index {
            append(md, ctx, ctx.prop(&id, "group-position", &fmt_num(i)));
        }
    }
}

struct CoverEdit {
    replace: Option<(String, Vec<u8>)>,
}

fn set_cover(
    root: &mut Element,
    opf_dir: &str,
    names: &HashSet<String>,
    data: &[u8],
) -> Option<CoverEdit> {
    let mime = sniff_image(data)?;
    let existing = cover_item(root).map(|e| {
        (
            e.attr("href").unwrap_or("").to_string(),
            e.attr("id").unwrap_or("").to_string(),
        )
    });
    if let Some((href, id)) = existing {
        if let Some(manifest) = root.child_mut("manifest") {
            for n in &mut manifest.children {
                if let Node::Element(e) = n
                    && e.is("item")
                    && e.attr("href") == Some(href.as_str())
                    && e.attr("id").unwrap_or("") == id
                {
                    e.set_attr("media-type", mime);
                }
            }
        }
        return Some(CoverEdit {
            replace: Some((resolve(opf_dir, &href), data.to_vec())),
        });
    }
    let v3 = root
        .attr("version")
        .is_some_and(|v| v.trim().starts_with('3'));
    let mut used = HashSet::new();
    collect_ids(root, &mut used);
    let id = unique_id(&mut used, "cover-image");
    let ext = util::image_ext(mime);
    let mut href = format!("images/cover.{ext}");
    let mut n = 1;
    while names.contains(&resolve(opf_dir, &href)) {
        n += 1;
        href = format!("images/cover{n}.{ext}");
    }
    let mut item = Element::new("item")
        .with_attr("id", id.as_str())
        .with_attr("href", href.as_str())
        .with_attr("media-type", mime);
    if v3 {
        item.set_attr("properties", "cover-image");
    }
    if root.child("manifest").is_none() {
        root.children.push(Node::Element(Element::new("manifest")));
    }
    let manifest = root.child_mut("manifest")?;
    manifest.children.push(Node::Element(item));
    manifest.children.push(Node::Text("\n  ".into()));
    let at = manifest.children.len() - 2;
    manifest.children.insert(at, Node::Text("\n    ".into()));
    let md = root.child_mut("metadata")?;
    let meta_name = md
        .elements()
        .find(|e| e.is("meta"))
        .map(|e| e.name.clone())
        .unwrap_or_else(|| "meta".into());
    remove_where(md, |e| e.is("meta") && e.attr("name") == Some("cover"));
    let ctx = Ctx {
        dc: String::new(),
        meta_name,
        opf_attr: String::new(),
        v3,
        indent: "    ".into(),
    };
    append(md, &ctx, ctx.meta("cover", &id));
    Some(CoverEdit {
        replace: Some((resolve(opf_dir, &href), data.to_vec())),
    })
}

pub fn write_metadata(path: &Path, meta: &Metadata, cover: Option<&[u8]>) -> Result<bool> {
    let mut zip = open(path)?;
    let mut opf = load_opf(&mut zip)?;
    apply_metadata(&mut opf.doc.root, meta);
    let mut replace: HashMap<String, Vec<u8>> = HashMap::new();
    if let Some(data) = cover {
        let names: HashSet<String> = zip.file_names().map(str::to_string).collect();
        if let Some(edit) = set_cover(&mut opf.doc.root, &opf.dir, &names, data)
            && let Some((name, bytes)) = edit.replace
        {
            let real = zip
                .file_names()
                .find(|n| n.eq_ignore_ascii_case(&name))
                .map(str::to_string)
                .unwrap_or(name);
            replace.insert(real, bytes);
        }
    }
    let opf_real = zip
        .file_names()
        .find(|n| *n == opf.path)
        .map(str::to_string)
        .unwrap_or(opf.path.clone());
    replace.insert(opf_real, opf.doc.serialize().into_bytes());
    rewrite_zip(path, zip, &replace)?;
    Ok(true)
}

fn rewrite_zip(path: &Path, mut zin: Zip, replace: &HashMap<String, Vec<u8>>) -> Result<()> {
    let order: Vec<usize> = (0..zin.len()).collect();
    util::atomic_write(path, |out| {
        let mut w = ZipWriter::new(out);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        w.start_file("mimetype", stored)?;
        w.write_all(b"application/epub+zip")?;
        let mut written = HashSet::from(["mimetype".to_string()]);
        for i in order {
            let f = zin.by_index_raw(i)?;
            let name = f.name().to_string();
            if !written.insert(name.clone()) {
                continue;
            }
            match replace.get(&name) {
                Some(data) => {
                    drop(f);
                    let opts = if util::is_image_name(&name) {
                        stored
                    } else {
                        deflated
                    };
                    w.start_file(name.as_str(), opts)?;
                    w.write_all(data)?;
                }
                None => w.raw_copy_file(f)?,
            }
        }
        let known: HashSet<String> = written;
        let mut extra: Vec<&String> = replace.keys().filter(|k| !known.contains(*k)).collect();
        extra.sort();
        for name in extra {
            let opts = if util::is_image_name(name) {
                stored
            } else {
                deflated
            };
            w.start_file(name.as_str(), opts)?;
            w.write_all(&replace[name])?;
        }
        w.finish()?;
        Ok(())
    })?;
    Ok(())
}
