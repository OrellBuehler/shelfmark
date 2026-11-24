use std::path::Path;

use shelfmark_model::Metadata;

use crate::error::{Result, invalid};
use crate::lang;
use crate::util::{self, clean, normalize_date, push_unique};
use crate::xml::{self, Element};

fn load(path: &Path) -> Result<xml::Document> {
    let bytes = std::fs::read(path)?;
    let doc = xml::parse(&bytes).ok_or_else(|| invalid("fb2", "malformed xml"))?;
    if !doc.root.is("FictionBook") {
        return Err(invalid("fb2", "not a FictionBook"));
    }
    Ok(doc)
}

fn person(a: &Element) -> Option<String> {
    let parts: Vec<String> = ["first-name", "middle-name", "last-name"]
        .iter()
        .filter_map(|n| a.child(n).and_then(|e| clean(&e.text())))
        .collect();
    if parts.is_empty() {
        a.child("nickname").and_then(|e| clean(&e.text()))
    } else {
        Some(parts.join(" "))
    }
}

fn paragraphs(el: &Element) -> Option<String> {
    let mut ps = Vec::new();
    el.find_all("p", &mut ps);
    let out: Vec<String> = ps
        .iter()
        .filter_map(|p| clean(&p.text()))
        .map(|t| format!("<p>{}</p>", xml::escape(&t)))
        .collect();
    (!out.is_empty()).then(|| out.join("\n"))
}

pub fn read_metadata(path: &Path) -> Result<Metadata> {
    let doc = load(path)?;
    let mut m = Metadata::default();
    let desc = doc.root.child("description");
    if let Some(ti) = desc.and_then(|d| d.child("title-info")) {
        m.title = ti.child("book-title").and_then(|e| clean(&e.text()));
        for a in ti.children_named("author") {
            if let Some(n) = person(a) {
                push_unique(&mut m.authors, n);
            }
        }
        for g in ti.children_named("genre") {
            if let Some(t) = clean(&g.text()) {
                push_unique(&mut m.tags, t);
            }
        }
        m.comments = ti.child("annotation").and_then(paragraphs);
        m.pubdate = ti
            .child("date")
            .and_then(|d| normalize_date(d.attr("value").unwrap_or(&d.text())));
        if let Some(l) = ti.child("lang").and_then(|e| lang::normalize(&e.text())) {
            m.languages = vec![l];
        }
        if let Some(s) = ti.child("sequence") {
            m.series = s.attr("name").and_then(clean);
            m.series_index = s.attr("number").and_then(|n| n.trim().parse().ok());
        }
    }
    if let Some(pi) = desc.and_then(|d| d.child("publish-info")) {
        m.publisher = pi.child("publisher").and_then(|e| clean(&e.text()));
        if let Some(i) = pi.child("isbn").and_then(|e| clean(&e.text())) {
            m.identifiers.insert("isbn".into(), i.replace('-', ""));
        }
        if m.pubdate.is_none() {
            m.pubdate = pi.child("year").and_then(|e| normalize_date(&e.text()));
        }
    }
    Ok(m)
}

pub fn read_cover(path: &Path) -> Result<Option<Vec<u8>>> {
    let doc = load(path)?;
    let href = doc
        .root
        .child("description")
        .and_then(|d| d.child("title-info"))
        .and_then(|t| t.child("coverpage"))
        .and_then(|c| c.child("image"))
        .and_then(|i| i.attr("href"));
    let Some(id) = href.map(|h| h.trim_start_matches('#').to_string()) else {
        return Ok(None);
    };
    for b in doc.root.children_named("binary") {
        if b.attr("id") == Some(id.as_str()) {
            let data = util::base64_decode(&b.text());
            return Ok(util::sniff_image(&data).map(|_| data));
        }
    }
    Ok(None)
}
