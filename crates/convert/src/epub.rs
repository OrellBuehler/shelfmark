use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use zip::ZipArchive;

use crate::dom::{self, Elem};
use crate::error::{Error, Result, invalid};
use crate::util::resolve_href;

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct GuideRef {
    pub kind: String,
    pub title: String,
    pub href: String,
    pub frag: String,
}

#[derive(Clone, Debug, Default)]
pub struct TocNode {
    pub title: String,
    pub target: Option<(String, String)>,
    pub children: Vec<TocNode>,
}

#[derive(Clone, Debug, Default)]
pub struct EpubMeta {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub subjects: Vec<String>,
    pub date: Option<String>,
    pub writing_mode: Option<String>,
    pub isbn: Option<String>,
    pub uuid: Option<String>,
}

pub struct Epub {
    zip: ZipArchive<File>,
    names: HashMap<String, usize>,
    pub items: Vec<Item>,
    pub spine: Vec<usize>,
    pub guide: Vec<GuideRef>,
    pub toc: Vec<TocNode>,
    pub meta: EpubMeta,
    pub rtl: bool,
    pub cover_id: Option<String>,
    pub obfuscated: Vec<String>,
}

const MAX_ENTRY_SIZE: u64 = 512 * 1024 * 1024;

pub fn is_xhtml(media_type: &str) -> bool {
    matches!(
        media_type,
        "application/xhtml+xml" | "text/html" | "application/x-dtbook+xml" | "text/x-oeb1-document"
    )
}

impl Epub {
    pub fn open(path: &Path) -> Result<Epub> {
        let file = File::open(path)?;
        let zip = ZipArchive::new(file).map_err(|e| match e {
            zip::result::ZipError::Io(io) => Error::Io(io),
            e => invalid(format!("not a zip archive: {e}")),
        })?;
        let mut names = HashMap::new();
        for i in 0..zip.len() {
            if let Some(Ok(name)) = zip.name_for_index(i) {
                names.insert(name.to_string(), i);
            }
        }
        let mut epub = Epub {
            zip,
            names,
            items: Vec::new(),
            spine: Vec::new(),
            guide: Vec::new(),
            toc: Vec::new(),
            meta: EpubMeta::default(),
            rtl: false,
            cover_id: None,
            obfuscated: Vec::new(),
        };
        epub.check_drm()?;
        let opf_path = epub.find_opf()?;
        epub.read_opf(&opf_path)?;
        Ok(epub)
    }

    pub fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        let idx = self
            .names
            .get(path)
            .copied()
            .or_else(|| {
                self.names
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(path))
                    .map(|(_, v)| *v)
            })
            .ok_or_else(|| invalid(format!("missing file {path}")))?;
        let mut f = self.zip.by_index(idx)?;
        if f.size() > MAX_ENTRY_SIZE {
            return Err(invalid(format!("{path} is too large")));
        }
        let mut buf = Vec::with_capacity(f.size() as usize);
        f.by_ref().take(MAX_ENTRY_SIZE).read_to_end(&mut buf)?;
        Ok(buf)
    }

    pub fn has(&self, path: &str) -> bool {
        self.names.contains_key(path)
    }

    fn check_drm(&mut self) -> Result<()> {
        if self.has("META-INF/rights.xml") || self.has("META-INF/sinf.xml") {
            return Err(Error::Drm);
        }
        if self.has("META-INF/encryption.xml") {
            let raw = self.read("META-INF/encryption.xml")?;
            let root = dom::parse(&raw);
            let mut encrypted = Vec::new();
            root.find_all("EncryptedData", &mut encrypted);
            for data in encrypted {
                let algorithm = data
                    .find("EncryptionMethod")
                    .and_then(|m| m.attr("Algorithm"))
                    .unwrap_or("");
                let uri = data
                    .find("CipherReference")
                    .and_then(|c| c.attr("URI"))
                    .unwrap_or("");
                if algorithm == "http://www.idpf.org/2008/embedding"
                    || algorithm == "http://ns.adobe.com/pdf/enc#RC"
                {
                    self.obfuscated.push(crate::util::percent_decode(uri));
                } else {
                    return Err(Error::Drm);
                }
            }
        }
        Ok(())
    }

    fn find_opf(&mut self) -> Result<String> {
        if self.has("META-INF/container.xml") {
            let raw = self.read("META-INF/container.xml")?;
            let root = dom::parse(&raw);
            let mut roots = Vec::new();
            root.find_all("rootfile", &mut roots);
            let found = roots.iter().find_map(|r| {
                let path = r.attr("full-path")?;
                let kind = r
                    .attr("media-type")
                    .unwrap_or("application/oebps-package+xml");
                (kind.contains("oebps-package") || path.ends_with(".opf"))
                    .then(|| crate::util::percent_decode(path))
            });
            if let Some(p) = found
                && self.has(&p)
            {
                return Ok(p);
            }
        }
        let mut opfs: Vec<&String> = self.names.keys().filter(|n| n.ends_with(".opf")).collect();
        opfs.sort();
        opfs.first()
            .map(|s| s.to_string())
            .ok_or_else(|| invalid("no package document found"))
    }

    fn read_opf(&mut self, opf_path: &str) -> Result<()> {
        let raw = self.read(opf_path)?;
        let package = dom::parse(&raw);
        if !package.is("package") {
            return Err(invalid("package document has no package element"));
        }
        let manifest = package
            .first("manifest")
            .ok_or_else(|| invalid("package has no manifest"))?;
        for it in manifest.elements().filter(|e| e.is("item")) {
            let (Some(id), Some(href)) = (it.attr("id"), it.attr("href")) else {
                continue;
            };
            let Some((path, _)) = resolve_href(opf_path, href) else {
                continue;
            };
            self.items.push(Item {
                id: id.to_string(),
                href: path,
                media_type: it
                    .attr("media-type")
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase(),
                properties: it
                    .attr("properties")
                    .unwrap_or("")
                    .split_whitespace()
                    .map(String::from)
                    .collect(),
            });
        }

        if let Some(spine) = package.first("spine") {
            self.rtl = spine
                .attr("page-progression-direction")
                .is_some_and(|d| d.eq_ignore_ascii_case("rtl"));
            for r in spine.elements().filter(|e| e.is("itemref")) {
                let Some(idref) = r.attr("idref") else {
                    continue;
                };
                if let Some(i) = self.items.iter().position(|it| it.id == idref)
                    && is_xhtml(&self.items[i].media_type)
                    && !self.spine.contains(&i)
                {
                    self.spine.push(i);
                }
            }
        }
        if self.spine.is_empty() {
            self.spine = (0..self.items.len())
                .filter(|&i| is_xhtml(&self.items[i].media_type))
                .collect();
        }
        if self.spine.is_empty() {
            return Err(invalid("the book has no readable content"));
        }

        self.read_metadata(&package);
        self.read_guide(&package, opf_path);
        let ncx_id = package
            .first("spine")
            .and_then(|s| s.attr("toc"))
            .map(String::from);
        self.read_toc(ncx_id.as_deref());
        if self.guide.is_empty() {
            self.read_landmarks();
        }
        Ok(())
    }

    fn read_metadata(&mut self, package: &Elem) {
        let Some(md) = package.first("metadata") else {
            return;
        };
        let text = |e: &Elem| {
            let t = e.text();
            let t = t.trim();
            (!t.is_empty()).then(|| t.to_string())
        };
        let mut m = EpubMeta::default();
        for e in md.elements() {
            let name = e.lname();
            match name.as_str() {
                "title" if m.title.is_none() => m.title = text(e),
                "creator" => m.authors.extend(text(e)),
                "language" if m.language.is_none() => m.language = text(e),
                "publisher" if m.publisher.is_none() => m.publisher = text(e),
                "description" if m.description.is_none() => m.description = text(e),
                "subject" => m.subjects.extend(text(e)),
                "date" if m.date.is_none() => m.date = text(e),
                "identifier" => {
                    let Some(v) = text(e) else { continue };
                    let scheme = e.attr_local("scheme").unwrap_or("").to_ascii_lowercase();
                    let lower = v.to_ascii_lowercase();
                    if let Some(isbn) = lower.strip_prefix("urn:isbn:") {
                        m.isbn.get_or_insert(isbn.to_string());
                    } else if scheme == "isbn" {
                        m.isbn.get_or_insert(v);
                    } else if let Some(u) = lower.strip_prefix("urn:uuid:") {
                        m.uuid.get_or_insert(u.to_string());
                    } else if scheme == "uuid" {
                        m.uuid.get_or_insert(lower);
                    }
                }
                "meta" => match e.attr("name") {
                    Some("cover") => self.cover_id = e.attr("content").map(String::from),
                    Some("primary-writing-mode") => {
                        m.writing_mode = e.attr("content").map(String::from)
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        self.meta = m;
    }

    fn read_guide(&mut self, package: &Elem, opf_path: &str) {
        let Some(guide) = package.first("guide") else {
            return;
        };
        for r in guide.elements().filter(|e| e.is("reference")) {
            let (Some(kind), Some(href)) = (r.attr("type"), r.attr("href")) else {
                continue;
            };
            if let Some((path, frag)) = resolve_href(opf_path, href) {
                self.guide.push(GuideRef {
                    kind: kind.to_ascii_lowercase(),
                    title: r.attr("title").unwrap_or("").trim().to_string(),
                    href: path,
                    frag,
                });
            }
        }
    }

    fn read_landmarks(&mut self) {
        let Some(nav) = self
            .items
            .iter()
            .find(|i| i.properties.iter().any(|p| p == "nav"))
            .cloned()
        else {
            return;
        };
        let Ok(raw) = self.read(&nav.href) else {
            return;
        };
        let root = dom::parse(&raw);
        let mut navs = Vec::new();
        root.find_all("nav", &mut navs);
        let Some(landmarks) = navs.iter().find(|n| {
            n.attr_local("type")
                .is_some_and(|t| t.split_whitespace().any(|w| w == "landmarks"))
        }) else {
            return;
        };
        let mut links = Vec::new();
        landmarks.find_all("a", &mut links);
        for a in links {
            let Some(kind) = a.attr_local("type") else {
                continue;
            };
            let kind = match kind.split_whitespace().next().unwrap_or("") {
                "bodymatter" => "text",
                "cover" => "cover",
                "toc" => "toc",
                "titlepage" => "title-page",
                "copyright-page" => "copyright-page",
                "index" => "index",
                "glossary" => "glossary",
                "bibliography" => "bibliography",
                "foreword" => "foreword",
                "preface" => "preface",
                "acknowledgments" => "acknowledgements",
                "dedication" => "dedication",
                "epigraph" => "epigraph",
                _ => continue,
            };
            let Some(href) = a.attr("href") else { continue };
            if let Some((path, frag)) = resolve_href(&nav.href, href) {
                self.guide.push(GuideRef {
                    kind: kind.to_string(),
                    title: collapse(&a.text()),
                    href: path,
                    frag,
                });
            }
        }
    }

    fn read_toc(&mut self, ncx_id: Option<&str>) {
        let nav = self
            .items
            .iter()
            .find(|i| i.properties.iter().any(|p| p == "nav"))
            .cloned();
        let ncx = ncx_id
            .and_then(|id| self.items.iter().find(|i| i.id == id))
            .or_else(|| {
                self.items
                    .iter()
                    .find(|i| i.media_type == "application/x-dtbncx+xml")
            })
            .cloned();
        if let Some(nav) = nav
            && let Ok(raw) = self.read(&nav.href)
        {
            let toc = parse_nav(&dom::parse(&raw), &nav.href);
            if !toc.is_empty() {
                self.toc = toc;
                return;
            }
        }
        if let Some(ncx) = ncx
            && let Ok(raw) = self.read(&ncx.href)
        {
            self.toc = parse_ncx(&dom::parse(&raw), &ncx.href);
        }
    }

    pub fn item_by_href(&self, href: &str) -> Option<usize> {
        self.items.iter().position(|i| i.href == href)
    }
}

pub fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_nav(root: &Elem, nav_href: &str) -> Vec<TocNode> {
    let mut navs = Vec::new();
    root.find_all("nav", &mut navs);
    let toc = navs
        .iter()
        .find(|n| {
            n.attr_local("type")
                .is_some_and(|t| t.split_whitespace().any(|w| w == "toc"))
        })
        .or(navs.first());
    let Some(toc) = toc else { return Vec::new() };
    let Some(list) = toc.find("ol").or_else(|| toc.find("ul")) else {
        return Vec::new();
    };
    nav_list(list, nav_href)
}

fn nav_list(list: &Elem, nav_href: &str) -> Vec<TocNode> {
    let mut out = Vec::new();
    for li in list.elements().filter(|e| e.is("li")) {
        let label = li.elements().find(|e| e.is("a") || e.is("span"));
        let title = label.map(|l| collapse(&l.text())).unwrap_or_default();
        let target = li
            .elements()
            .find(|e| e.is("a"))
            .and_then(|a| a.attr("href"))
            .and_then(|h| resolve_href(nav_href, h));
        let children = li
            .elements()
            .find(|e| e.is("ol") || e.is("ul"))
            .map(|l| nav_list(l, nav_href))
            .unwrap_or_default();
        out.push(TocNode {
            title,
            target,
            children,
        });
    }
    out
}

fn parse_ncx(root: &Elem, ncx_href: &str) -> Vec<TocNode> {
    let Some(map) = root.find("navMap") else {
        return Vec::new();
    };
    ncx_points(map, ncx_href)
}

fn ncx_points(parent: &Elem, ncx_href: &str) -> Vec<TocNode> {
    parent
        .elements()
        .filter(|e| e.is("navPoint"))
        .map(|p| TocNode {
            title: p
                .first("navLabel")
                .map(|l| collapse(&l.text()))
                .unwrap_or_default(),
            target: p
                .first("content")
                .and_then(|c| c.attr("src"))
                .and_then(|s| resolve_href(ncx_href, s)),
            children: ncx_points(p, ncx_href),
        })
        .collect()
}
