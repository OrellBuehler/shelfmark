use std::path::Path;

use lopdf::{Dictionary, Document, Object, Stream, StringFormat, dictionary};
use shelfmark_model::Metadata;

use crate::error::Result;
use crate::lang;
use crate::util::{self, clean, normalize_date, push_unique};
use crate::xml::{self, Element};

fn decode_text(b: &[u8]) -> String {
    if let Some(r) = b.strip_prefix(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE
            .decode_without_bom_handling(r)
            .0
            .into_owned();
    }
    if let Some(r) = b.strip_prefix(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE
            .decode_without_bom_handling(r)
            .0
            .into_owned();
    }
    if let Some(r) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(r).into_owned();
    }
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(b)
            .0
            .into_owned(),
    }
}

fn encode_text(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut v = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            v.extend_from_slice(&u.to_be_bytes());
        }
        Object::String(v, StringFormat::Literal)
    }
}

fn info_dict(doc: &Document) -> Option<&Dictionary> {
    let obj = doc.trailer.get(b"Info").ok()?;
    let (_, obj) = doc.dereference(obj).ok()?;
    obj.as_dict().ok()
}

fn info_str(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<String> {
    let (_, obj) = doc.dereference(dict.get(key).ok()?).ok()?;
    match obj {
        Object::String(b, _) => clean(&decode_text(b)),
        _ => None,
    }
}

fn split_authors(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in s.split(['&', ';']).flat_map(|p| p.split(" and ")) {
        if let Some(p) = clean(part) {
            push_unique(&mut out, p);
        }
    }
    out
}

fn split_keywords(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in s.split([',', ';']) {
        if let Some(p) = clean(part) {
            push_unique(&mut out, p);
        }
    }
    out
}

fn li_texts(el: &Element) -> Vec<String> {
    let mut lis = Vec::new();
    el.find_all("li", &mut lis);
    if lis.is_empty() {
        return clean(&el.text()).into_iter().collect();
    }
    let default = lis.iter().find(|l| l.attr("lang") == Some("x-default"));
    if let Some(d) = default {
        return clean(&d.text()).into_iter().collect();
    }
    lis.iter().filter_map(|l| clean(&l.text())).collect()
}

fn xmp_values(root: &Element, name: &str) -> Vec<String> {
    let mut els = Vec::new();
    root.find_all(name, &mut els);
    els.iter().flat_map(|e| li_texts(e)).collect()
}

fn read_xmp(doc: &Document, m: &mut Metadata) -> Option<()> {
    let cat = doc.catalog().ok()?;
    let (_, obj) = doc.dereference(cat.get(b"Metadata").ok()?).ok()?;
    let stream = obj.as_stream().ok()?;
    let data = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    let start = data
        .windows(6)
        .position(|w| w == b"<x:xmp")
        .or_else(|| data.iter().position(|b| *b == b'<'))?;
    let parsed = xml::parse(&data[start..])?;
    let root = &parsed.root;
    if let Some(t) = xmp_values(root, "title").into_iter().next() {
        m.title = Some(t);
    }
    let creators = xmp_values(root, "creator");
    if !creators.is_empty() {
        m.authors = creators;
    }
    let tags = xmp_values(root, "subject");
    if !tags.is_empty() {
        m.tags = tags;
    }
    if m.tags.is_empty() {
        m.tags = xmp_values(root, "Keywords")
            .iter()
            .flat_map(|k| split_keywords(k))
            .collect();
    }
    if let Some(d) = xmp_values(root, "description").into_iter().next() {
        m.comments = Some(d);
    }
    if let Some(p) = xmp_values(root, "publisher").into_iter().next() {
        m.publisher = Some(p);
    }
    if let Some(d) = xmp_values(root, "date")
        .iter()
        .find_map(|d| normalize_date(d))
    {
        m.pubdate = Some(d);
    }
    let langs = lang::normalize_all(xmp_values(root, "language").iter().map(|s| s.as_str()));
    if !langs.is_empty() {
        m.languages = langs;
    }
    let mut ids = Vec::new();
    root.find_all("Identifier", &mut ids);
    for id in ids {
        let mut lis = Vec::new();
        id.find_all("li", &mut lis);
        for li in lis {
            let scheme = li.child("Scheme").map(|s| s.trimmed_text().to_lowercase());
            let value = li.child("value").map(|s| s.trimmed_text());
            if let (Some(s), Some(v)) = (scheme, value) {
                if s == "uuid" {
                    m.uuid = Some(v);
                } else if !s.is_empty() && !v.is_empty() {
                    m.identifiers.insert(s, v);
                }
            }
        }
    }
    if let Some(series) = root.find("series")
        && let Some(v) = series.find("value").and_then(|v| clean(&v.text()))
    {
        m.series = Some(v);
        m.series_index = series
            .find("series_index")
            .and_then(|e| e.trimmed_text().parse().ok());
    }
    if let Some(r) = xmp_values(root, "rating")
        .first()
        .and_then(|r| r.parse::<f64>().ok())
    {
        m.rating = (r > 0.0).then_some(r.round().clamp(0.0, 10.0) as u8);
    }
    if let Some(t) = xmp_values(root, "title_sort").into_iter().next() {
        m.title_sort = Some(t);
    }
    if let Some(t) = xmp_values(root, "author_sort").into_iter().next() {
        m.author_sort = Some(t);
    }
    Some(())
}

pub fn read_metadata(path: &Path) -> Result<Metadata> {
    let doc = Document::load(path)?;
    let mut m = Metadata::default();
    if let Some(info) = info_dict(&doc) {
        m.title = info_str(&doc, info, b"Title");
        if let Some(a) = info_str(&doc, info, b"Author") {
            m.authors = split_authors(&a);
        }
        m.comments = info_str(&doc, info, b"Subject");
        if let Some(k) = info_str(&doc, info, b"Keywords") {
            m.tags = split_keywords(&k);
        }
    }
    read_xmp(&doc, &mut m);
    Ok(m)
}

fn xml_esc(s: &str) -> String {
    xml::escape(s)
}

fn alt(tag: &str, v: &str) -> String {
    format!(
        "<{tag}><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></{tag}>",
        xml_esc(v)
    )
}

fn seq(tag: &str, kind: &str, vals: &[String]) -> String {
    let lis: String = vals
        .iter()
        .map(|v| format!("<rdf:li>{}</rdf:li>", xml_esc(v)))
        .collect();
    format!("<{tag}><rdf:{kind}>{lis}</rdf:{kind}></{tag}>")
}

fn build_xmp(meta: &Metadata) -> Vec<u8> {
    let mut dc = String::new();
    if let Some(t) = &meta.title {
        dc.push_str(&alt("dc:title", t));
    }
    if !meta.authors.is_empty() {
        dc.push_str(&seq("dc:creator", "Seq", &meta.authors));
    }
    if let Some(c) = &meta.comments {
        dc.push_str(&alt("dc:description", c));
    }
    if !meta.tags.is_empty() {
        dc.push_str(&seq("dc:subject", "Bag", &meta.tags));
    }
    if let Some(p) = &meta.publisher {
        dc.push_str(&seq("dc:publisher", "Bag", std::slice::from_ref(p)));
    }
    if let Some(d) = &meta.pubdate {
        dc.push_str(&seq("dc:date", "Seq", std::slice::from_ref(d)));
    }
    if !meta.languages.is_empty() {
        dc.push_str(&seq("dc:language", "Bag", &meta.languages));
    }
    let mut ids = String::new();
    let mut all: Vec<(String, String)> = meta
        .identifiers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some(u) = &meta.uuid {
        all.push(("uuid".into(), u.clone()));
    }
    for (s, v) in &all {
        ids.push_str(&format!(
            "<rdf:li rdf:parseType=\"Resource\"><xmpidq:Scheme>{}</xmpidq:Scheme><rdf:value>{}</rdf:value></rdf:li>",
            xml_esc(s),
            xml_esc(v)
        ));
    }
    let mut calibre = String::new();
    if let Some(r) = meta.rating {
        calibre.push_str(&format!("<calibre:rating>{r}</calibre:rating>"));
    }
    if let Some(s) = &meta.series {
        calibre.push_str(&format!(
            "<calibre:series rdf:parseType=\"Resource\"><rdf:value>{}</rdf:value>",
            xml_esc(s)
        ));
        if let Some(i) = meta.series_index {
            calibre.push_str(&format!(
                "<calibreSI:series_index>{i:?}</calibreSI:series_index>"
            ));
        }
        calibre.push_str("</calibre:series>");
    }
    if let Some(t) = &meta.title_sort {
        calibre.push_str(&format!(
            "<calibre:title_sort>{}</calibre:title_sort>",
            xml_esc(t)
        ));
    }
    if let Some(t) = &meta.author_sort {
        calibre.push_str(&format!(
            "<calibre:author_sort>{}</calibre:author_sort>",
            xml_esc(t)
        ));
    }
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">{dc}</rdf:Description>\
<rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmlns:xmpidq=\"http://ns.adobe.com/xmp/Identifier/qual/1.0/\"><xmp:Identifier><rdf:Bag>{ids}</rdf:Bag></xmp:Identifier></rdf:Description>\
<rdf:Description rdf:about=\"\" xmlns:calibre=\"http://calibre-ebook.com/xmp-namespace\" xmlns:calibreSI=\"http://calibre-ebook.com/xmp-namespace-series-index\">{calibre}</rdf:Description>\
</rdf:RDF></x:xmpmeta>\n<?xpacket end=\"w\"?>"
    )
    .into_bytes()
}

pub fn write_metadata(path: &Path, meta: &Metadata) -> Result<bool> {
    let mut doc = Document::load(path)?;
    if doc.is_encrypted() {
        return Ok(false);
    }
    let info_id = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) if doc.get_dictionary(*id).is_ok() => *id,
        _ => {
            let id = doc.add_object(Dictionary::new());
            doc.trailer.set("Info", Object::Reference(id));
            id
        }
    };
    let info = doc.get_dictionary_mut(info_id)?;
    let mut set = |key: &str, val: Option<String>| match val.filter(|v| !v.is_empty()) {
        Some(v) => info.set(key, encode_text(&v)),
        None => {
            info.remove(key.as_bytes());
        }
    };
    set("Title", meta.title.clone());
    set(
        "Author",
        (!meta.authors.is_empty()).then(|| meta.authors.join(" & ")),
    );
    set("Subject", meta.comments.clone());
    set(
        "Keywords",
        (!meta.tags.is_empty()).then(|| meta.tags.join(", ")),
    );

    let xmp = Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        build_xmp(meta),
    );
    let existing = doc.catalog().ok().and_then(|c| match c.get(b"Metadata") {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    });
    match existing {
        Some(id) => {
            doc.objects.insert(id, Object::Stream(xmp));
        }
        None => {
            let id = doc.add_object(xmp);
            doc.catalog_mut()?.set("Metadata", Object::Reference(id));
        }
    }
    util::atomic_write(path, |f| {
        doc.save_to(f)?;
        Ok(())
    })?;
    Ok(true)
}
