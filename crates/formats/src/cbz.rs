use std::fs::File;
use std::io::Read;
use std::path::Path;

use shelfmark_model::Metadata;
use zip::ZipArchive;

use crate::error::Result;
use crate::lang;
use crate::util::{self, clean, normalize_date, push_unique};
use crate::xml;

fn image_names(zip: &ZipArchive<File>) -> Vec<String> {
    let mut names: Vec<String> = zip
        .file_names().flatten()
        .filter(|n| {
            !n.ends_with('/')
                && !n.starts_with("__MACOSX")
                && !n.rsplit('/').next().unwrap_or("").starts_with('.')
        })
        .filter(|n| util::is_image_name(n))
        .map(|n| n.to_string())
        .collect();
    names.sort_by(|a, b| util::natural_cmp(a, b));
    names
}

fn read(zip: &mut ZipArchive<File>, name: &str) -> Option<Vec<u8>> {
    let mut f = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn comic_info(zip: &mut ZipArchive<File>) -> Option<xml::Document> {
    let name = zip
        .file_names().flatten()
        .find(|n| {
            n.rsplit('/')
                .next()
                .is_some_and(|b| b.eq_ignore_ascii_case("ComicInfo.xml"))
        })?
        .to_string();
    xml::parse(&read(zip, &name)?)
}

fn split_list(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for p in s.split([',', ';']) {
        if let Some(p) = clean(p) {
            push_unique(&mut out, p);
        }
    }
    out
}

pub fn read_metadata(path: &Path) -> Result<Metadata> {
    let mut zip = ZipArchive::new(File::open(path)?)?;
    let mut m = Metadata::default();
    let Some(doc) = comic_info(&mut zip) else {
        return Ok(m);
    };
    let r = &doc.root;
    let text = |n: &str| r.child(n).and_then(|e| clean(&e.text()));
    m.title = text("Title");
    m.series = text("Series");
    m.series_index = text("Number").and_then(|n| n.parse().ok());
    m.authors = text("Writer").map(|w| split_list(&w)).unwrap_or_default();
    m.publisher = text("Publisher");
    m.comments = text("Summary");
    m.tags = [text("Genre"), text("Tags")]
        .into_iter()
        .flatten()
        .flat_map(|s| split_list(&s))
        .fold(Vec::new(), |mut acc, t| {
            push_unique(&mut acc, t);
            acc
        });
    if let Some(y) = text("Year") {
        let date = format!(
            "{y}-{}-{}",
            text("Month").unwrap_or_else(|| "1".into()),
            text("Day").unwrap_or_else(|| "1".into())
        );
        m.pubdate = normalize_date(&date).or_else(|| normalize_date(&y));
    }
    if let Some(l) = text("LanguageISO").and_then(|l| lang::normalize(&l)) {
        m.languages = vec![l];
    }
    if m.title.is_none()
        && let Some(s) = &m.series
    {
        m.title = Some(match text("Number") {
            Some(n) => format!("{s} {n}"),
            None => s.clone(),
        });
    }
    Ok(m)
}

pub fn read_cover(path: &Path) -> Result<Option<Vec<u8>>> {
    let mut zip = ZipArchive::new(File::open(path)?)?;
    let names = image_names(&zip);
    let mut pick = names.first().cloned();
    if let Some(doc) = comic_info(&mut zip) {
        let mut pages = Vec::new();
        doc.root.find_all("Page", &mut pages);
        let front = pages
            .iter()
            .find(|p| p.attr("Type") == Some("FrontCover"))
            .and_then(|p| p.attr("Image"))
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| names.get(i));
        if let Some(f) = front {
            pick = Some(f.clone());
        }
    }
    Ok(pick
        .and_then(|n| read(&mut zip, &n))
        .filter(|d| util::sniff_image(d).is_some()))
}
