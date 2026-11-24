use std::collections::{HashMap, HashSet};

use crate::css;
use crate::dom::{self, Elem, Node, local};
use crate::skeleton::{AID_ABLE, to_href};
use crate::util::{resolve_href, to_base};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

pub fn to_ref(n: usize) -> String {
    to_base(n as u64, 32, 4)
}

pub struct Doc {
    pub href: String,
    pub root: Elem,
}

#[derive(Default)]
pub struct Ctx {
    pub item_map: HashMap<String, usize>,
    pub mime_map: HashMap<String, String>,
    pub fonts: HashSet<String>,
    pub used_images: HashSet<String>,
    pub flows: Vec<Vec<u8>>,
    pub sheets: HashMap<String, usize>,
    pub svgs: HashMap<String, usize>,
    pub inline_css: HashMap<String, usize>,
    pub link_map: HashMap<String, (String, String)>,
    pub id_map: HashMap<(String, String), String>,
    link_count: u64,
    cid: u64,
}

impl Ctx {
    pub fn pointer(&mut self, base: &str, oref: &str) -> Option<String> {
        let (path, _) = resolve_href(base, oref)?;
        let idx = *self.item_map.get(&path)?;
        if self.fonts.contains(&path) {
            return Some(format!("kindle:embed:{}", to_ref(idx)));
        }
        let mime = self.mime_map.get(&path)?.clone();
        self.used_images.insert(path);
        Some(format!("kindle:embed:{}?mime={mime}", to_ref(idx)))
    }

    fn rewrite_css_resources(&mut self, base: &str, text: &str) -> String {
        css::rewrite(text, &mut |u| self.pointer(base, u), &mut |_| None)
    }

    fn rewrite_css_imports(&self, base: &str, text: &str) -> String {
        css::rewrite(text, &mut |_| None, &mut |u| {
            let (path, _) = resolve_href(base, u)?;
            let idx = self.sheets.get(&path)?;
            Some(format!("kindle:flow:{}?mime=text/css", to_ref(*idx)))
        })
    }
}

fn is_svg(e: &Elem) -> bool {
    e.is("svg")
}

fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn clean_attrs(attrs: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (k, v) in attrs {
        if k == "xmlns" || k.starts_with("xmlns:") {
            continue;
        }
        let lk = local(&k).to_ascii_lowercase();
        if lk == "aid" || lk == "cid" || !valid_name(&lk) {
            continue;
        }
        match out.iter_mut().find(|(ek, _)| *ek == lk) {
            Some(existing) => existing.1 = v,
            None => out.push((lk, v)),
        }
    }
    out
}

fn is_kobo_span(e: &Elem) -> bool {
    e.is("span")
        && e.attr("class")
            .is_some_and(|c| c.split_whitespace().any(|w| w == "koboSpan"))
}

fn clean_tree(e: &mut Elem) {
    let old = std::mem::take(&mut e.children);
    for n in old {
        let Node::Elem(mut c) = n else {
            e.children.push(n);
            continue;
        };
        if c.is("script") {
            continue;
        }
        if is_svg(&c) {
            e.children.push(Node::Elem(c));
            continue;
        }
        c.name = c.lname();
        if !valid_name(&c.name) {
            clean_tree(&mut c);
            e.children.extend(c.children);
            continue;
        }
        c.attrs = clean_attrs(std::mem::take(&mut c.attrs));
        clean_tree(&mut c);
        if is_kobo_span(&c) {
            e.children.extend(c.children);
        } else {
            e.children.push(Node::Elem(c));
        }
    }
}

pub fn prepare(root: Elem) -> Elem {
    let mut html = Elem::new("html");
    let root = if root.is("html") {
        root
    } else {
        let mut wrapper = Elem::new("html");
        wrapper.children.push(Node::Elem(root));
        wrapper
    };
    for (k, v) in &root.attrs {
        let lk = local(k).to_ascii_lowercase();
        if matches!(lk.as_str(), "lang" | "class" | "style" | "dir") {
            html.set_attr(&lk, v.clone());
        }
    }
    let mut head = Elem::new("head");
    let mut body = Elem::new("body");
    let mut have_body = false;
    let mut loose: Vec<Node> = Vec::new();
    for n in root.children {
        match n {
            Node::Elem(e) if e.is("head") => head.children.extend(e.children),
            Node::Elem(e) if e.is("body") => {
                if !have_body {
                    body.attrs = clean_attrs(e.attrs);
                    have_body = true;
                }
                body.children.extend(e.children);
            }
            Node::Text(t) if t.trim().is_empty() => {}
            other => loose.push(other),
        }
    }
    loose.append(&mut body.children);
    body.children = loose;
    clean_tree(&mut head);
    clean_tree(&mut body);
    let has_content = body.children.iter().any(|n| match n {
        Node::Elem(_) => true,
        Node::Text(t) => !t.trim().is_empty(),
    });
    if !has_content {
        body.children = vec![Node::Elem(Elem::new("span"))];
    }
    html.children = vec![
        Node::Text("\n".into()),
        Node::Elem(head),
        Node::Text("\n".into()),
        Node::Elem(body),
        Node::Text("\n".into()),
    ];
    html
}

pub fn body_text_is_empty(root: &Elem) -> bool {
    let Some(body) = root.first("body") else {
        return true;
    };
    body.text().trim().is_empty()
        && body
            .find("svg")
            .map(|s| s.text().trim().is_empty())
            .unwrap_or(true)
}

pub fn remove_duplicate_anchors(root: &mut Elem) {
    let mut seen: HashSet<String> = HashSet::new();
    root.walk_mut(&mut |e| {
        let is_a = e.is("a");
        if let Some(id) = e.attr("id").map(String::from)
            && !seen.insert(id)
        {
            e.remove_attr("id");
        }
        if is_a
            && let Some(name) = e.attr("name").map(String::from)
            && !seen.insert(name)
        {
            e.remove_attr("name");
        }
    });
}

pub fn replace_resource_links(ctx: &mut Ctx, doc: &mut Doc) {
    let href = doc.href.clone();
    doc.root.walk_mut(&mut |e| {
        if e.is("img") {
            e.attrs.retain(|(k, _)| k != "srcset" && k != "sizes");
        }
        if e.is("img") || e.is("image") {
            for (k, v) in e.attrs.iter_mut() {
                let lk = local(k).to_ascii_lowercase();
                if (lk == "src" || lk == "href")
                    && let Some(r) = ctx.pointer(&href, v)
                {
                    *v = r;
                }
            }
        }
        if e.is("style") {
            for n in &mut e.children {
                if let Node::Text(t) = n {
                    *t = ctx.rewrite_css_resources(&href, t);
                }
            }
        }
        if let Some((_, v)) = e.attrs.iter_mut().find(|(k, _)| k == "style")
            && v.contains("url(")
        {
            *v = ctx.rewrite_css_resources(&href, v);
        }
    });
}

pub fn extract_css_into_flows(ctx: &mut Ctx, sheets: Vec<(String, String)>, docs: &mut [Doc]) {
    for (href, _) in &sheets {
        ctx.sheets.insert(href.clone(), ctx.flows.len());
        ctx.flows.push(Vec::new());
    }
    for (href, text) in &sheets {
        let text = ctx.rewrite_css_resources(href, text);
        let text = ctx.rewrite_css_imports(href, &text);
        let idx = ctx.sheets[href];
        ctx.flows[idx] = text.into_bytes();
    }
    for doc in docs.iter_mut() {
        let href = doc.href.clone();
        let mut root = std::mem::take(&mut doc.root);
        visit_children(&mut root, &mut |children| {
            for n in children.iter_mut() {
                let mut replacement = None;
                if let Node::Elem(e) = n {
                    if e.is("link") {
                        let target = e
                            .attr("href")
                            .and_then(|h| resolve_href(&href, h))
                            .and_then(|(path, _)| ctx.sheets.get(&path).copied());
                        if let Some(idx) = target {
                            e.set_attr(
                                "href",
                                format!("kindle:flow:{}?mime=text/css", to_ref(idx)),
                            );
                        }
                    } else if e.is("style") {
                        let raw = e.text();
                        replacement = Some(if raw.trim().is_empty() {
                            Node::Text(String::new())
                        } else {
                            let raw = ctx.rewrite_css_imports(&href, &raw);
                            let idx = match ctx.inline_css.get(&raw) {
                                Some(i) => *i,
                                None => {
                                    let i = ctx.flows.len();
                                    ctx.flows.push(raw.clone().into_bytes());
                                    ctx.inline_css.insert(raw, i);
                                    i
                                }
                            };
                            let mut link = Elem::new("link");
                            link.attrs = vec![
                                ("type".into(), "text/css".into()),
                                ("rel".into(), "stylesheet".into()),
                                (
                                    "href".into(),
                                    format!("kindle:flow:{}?mime=text/css", to_ref(idx)),
                                ),
                            ];
                            Node::Elem(link)
                        });
                    }
                }
                if let Some(r) = replacement {
                    *n = r;
                }
            }
            children.retain(|n| !matches!(n, Node::Text(t) if t.is_empty()));
        });
        doc.root = root;
    }
}

fn visit_children(e: &mut Elem, f: &mut dyn FnMut(&mut Vec<Node>)) {
    f(&mut e.children);
    for n in &mut e.children {
        if let Node::Elem(c) = n {
            visit_children(c, f);
        }
    }
}

fn svg_flow(svg: &Elem, declaration: bool) -> Vec<u8> {
    let mut svg = svg.clone();
    if !svg.name.contains(':') && svg.attr("xmlns").is_none() {
        svg.attrs.insert(0, ("xmlns".into(), SVG_NS.into()));
    }
    let mut uses_xlink = false;
    svg.walk(&mut |e| {
        uses_xlink |= e.attrs.iter().any(|(k, _)| k.starts_with("xlink:"));
    });
    if uses_xlink && svg.attr("xmlns:xlink").is_none() {
        svg.attrs.push(("xmlns:xlink".into(), XLINK_NS.into()));
    }
    let mut out = Vec::new();
    if declaration {
        out.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    dom::write_elem(&svg, &mut out, &|_| false);
    out
}

pub fn extract_svg_into_flows(ctx: &mut Ctx, svg_items: Vec<(String, Elem)>, docs: &mut [Doc]) {
    for (href, mut root) in svg_items {
        root.walk_mut(&mut |e| {
            if e.is("image") {
                for (k, v) in e.attrs.iter_mut() {
                    let lk = local(k).to_ascii_lowercase();
                    if (lk == "src" || lk == "href")
                        && let Some(r) = ctx.pointer(&href, v)
                    {
                        *v = r;
                    }
                }
            }
        });
        ctx.svgs.insert(href, ctx.flows.len());
        ctx.flows.push(svg_flow(&root, true));
    }
    for doc in docs.iter_mut() {
        let href = doc.href.clone();
        replace_inline_svgs(ctx, &mut doc.root);
        doc.root.walk_mut(&mut |e| {
            if e.is("img")
                && let Some(src) = e.attr("src")
                && let Some((path, _)) = resolve_href(&href, src)
                && let Some(idx) = ctx.svgs.get(&path)
            {
                let v = format!("kindle:flow:{}?mime=image/svg+xml", to_ref(*idx));
                e.set_attr("src", v);
            }
        });
    }
}

fn replace_inline_svgs(ctx: &mut Ctx, e: &mut Elem) {
    for n in &mut e.children {
        let Node::Elem(c) = n else { continue };
        if is_svg(c) {
            let idx = ctx.flows.len();
            ctx.flows.push(svg_flow(c, false));
            let mut img = Elem::new("img");
            img.attrs.push((
                "src".into(),
                format!("kindle:flow:{}?mime=image/svg+xml", to_ref(idx)),
            ));
            *n = Node::Elem(img);
        } else {
            replace_inline_svgs(ctx, c);
        }
    }
}

pub fn replace_internal_links(ctx: &mut Ctx, docs: &mut [Doc], spine: &HashSet<String>) {
    for doc in docs.iter_mut() {
        let base = doc.href.clone();
        doc.root.walk_mut(&mut |e| {
            if !e.is("a") {
                return;
            }
            let Some(href) = e.attr("href") else { return };
            ctx.link_count += 1;
            let Some((path, frag)) = resolve_href(&base, href) else {
                return;
            };
            if spine.contains(&path) {
                let placeholder = format!("kindle:pos:fid:0000:off:{}", to_href(ctx.link_count));
                ctx.link_map.insert(placeholder.clone(), (path, frag));
                e.set_attr("href", placeholder);
            }
        });
    }
}

pub fn insert_aid_attributes(ctx: &mut Ctx, docs: &mut [Doc]) {
    for (i, doc) in docs.iter_mut().enumerate() {
        let aidbase = i as u64 * 1_000_000;
        let mut j = 0u64;
        let href = doc.href.clone();
        assign_aids(ctx, &href, &mut doc.root, aidbase, &mut j, false, true);
    }
}

fn assign_aids(
    ctx: &mut Ctx,
    href: &str,
    e: &mut Elem,
    aidbase: u64,
    j: &mut u64,
    in_table: bool,
    is_root: bool,
) {
    if !is_root {
        if e.attr("id").is_none()
            && e.is("a")
            && let Some(name) = e.attr("name").map(String::from)
        {
            e.set_attr("id", name);
        }
        let id = e.attr("id").map(String::from);
        let name = e.lname();
        if id.is_some() || AID_ABLE.contains(&name.as_str()) {
            if name == "table" || in_table {
                if let Some(id) = id.filter(|i| !i.is_empty()) {
                    ctx.cid += 1;
                    let val = format!("c{}", ctx.cid);
                    ctx.id_map.insert((href.to_string(), id), val.clone());
                    e.set_attr("cid", val);
                }
            } else {
                let aid = to_base(aidbase + *j, 32, 0);
                e.set_attr("aid", aid.clone());
                if name == "body" {
                    ctx.id_map
                        .insert((href.to_string(), String::new()), aid.clone());
                }
                if let Some(id) = id {
                    ctx.id_map.insert((href.to_string(), id), aid);
                }
                *j += 1;
            }
        }
    }
    let table = in_table || (!is_root && e.is("table"));
    for n in &mut e.children {
        if let Node::Elem(c) = n {
            assign_aids(ctx, href, c, aidbase, j, table, false);
        }
    }
}
