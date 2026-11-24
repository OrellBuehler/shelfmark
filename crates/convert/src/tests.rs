use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use shelfmark_model::Metadata;

use crate::verify::{self, Book};
use crate::{Error, can_convert, epub_to_azw3};

type Files = Vec<(String, Vec<u8>)>;

fn f(name: &str, data: impl AsRef<[u8]>) -> (String, Vec<u8>) {
    (name.to_string(), data.as_ref().to_vec())
}

fn write_epub(dir: &Path, name: &str, files: &Files) -> PathBuf {
    let path = dir.join(name);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    for (n, d) in files {
        zip.start_file(n.as_str(), deflated).unwrap();
        zip.write_all(d).unwrap();
    }
    zip.finish().unwrap();
    path
}

const CONTAINER: &str = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

fn xhtml(title: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"en\"><head><title>{title}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head><body>{body}</body></html>"
    )
}

struct Spec {
    version: &'static str,
    metadata: String,
    manifest: Vec<(String, String, String, String)>,
    spine: Vec<String>,
    spine_attrs: String,
    guide: String,
}

impl Spec {
    fn new(version: &'static str) -> Spec {
        Spec {
            version,
            metadata: "<dc:title>Test Book</dc:title><dc:creator>Ann Author</dc:creator><dc:language>en</dc:language><dc:identifier id=\"id\">urn:uuid:12345678-1234-1234-1234-123456789abc</dc:identifier>".into(),
            manifest: Vec::new(),
            spine: Vec::new(),
            spine_attrs: String::new(),
            guide: String::new(),
        }
    }

    fn item(mut self, id: &str, href: &str, mime: &str, props: &str) -> Spec {
        self.manifest
            .push((id.into(), href.into(), mime.into(), props.into()));
        self
    }

    fn page(mut self, id: &str, href: &str) -> Spec {
        self = self.item(id, href, "application/xhtml+xml", "");
        self.spine.push(id.into());
        self
    }

    fn opf(&self) -> String {
        let mut s = format!(
            "<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"{}\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:opf=\"http://www.idpf.org/2007/opf\">{}</metadata><manifest>",
            self.version, self.metadata
        );
        for (id, href, mime, props) in &self.manifest {
            let p = if props.is_empty() {
                String::new()
            } else {
                format!(" properties=\"{props}\"")
            };
            s += &format!("<item id=\"{id}\" href=\"{href}\" media-type=\"{mime}\"{p}/>");
        }
        s += &format!("</manifest><spine {}>", self.spine_attrs);
        for id in &self.spine {
            s += &format!("<itemref idref=\"{id}\"/>");
        }
        s += "</spine>";
        if !self.guide.is_empty() {
            s += &format!("<guide>{}</guide>", self.guide);
        }
        s + "</package>"
    }

    fn files(&self, mut extra: Files) -> Files {
        let mut files = vec![
            f("META-INF/container.xml", CONTAINER),
            f("OEBPS/content.opf", self.opf()),
        ];
        files.append(&mut extra);
        files
    }
}

fn ncx(points: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\"><head/><docTitle><text>T</text></docTitle><navMap>{points}</navMap></ncx>"
    )
}

fn point(id: usize, label: &str, src: &str, children: &str) -> String {
    format!(
        "<navPoint id=\"n{id}\" playOrder=\"{id}\"><navLabel><text>{label}</text></navLabel><content src=\"{src}\"/>{children}</navPoint>"
    )
}

fn convert_files(
    files: &Files,
    meta: &Metadata,
    cover: Option<&[u8]>,
) -> (Vec<u8>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let src = write_epub(dir.path(), "in.epub", files);
    let dst = dir.path().join("out.azw3");
    epub_to_azw3(&src, &dst, meta, cover).unwrap();
    (std::fs::read(&dst).unwrap(), dir)
}

fn convert_book(files: &Files) -> Book {
    let (bytes, _dir) = convert_files(files, &Metadata::default(), None);
    verify::parse(&bytes)
}

fn convert_err(files: &Files) -> Error {
    let dir = tempfile::tempdir().unwrap();
    let src = write_epub(dir.path(), "in.epub", files);
    let dst = dir.path().join("out.azw3");
    let err = epub_to_azw3(&src, &dst, &Metadata::default(), None).unwrap_err();
    assert!(!dst.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    err
}

fn png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let img = if alpha {
        ::image::DynamicImage::ImageRgba8(::image::RgbaImage::from_fn(w, h, |x, y| {
            ::image::Rgba([
                (x % 250) as u8,
                (y % 250) as u8,
                100,
                if x == 0 { 0 } else { 255 },
            ])
        }))
    } else {
        ::image::DynamicImage::ImageRgb8(::image::RgbImage::from_fn(w, h, |x, y| {
            ::image::Rgb([(x % 250) as u8, (y % 250) as u8, 100])
        }))
    };
    img.write_to(
        &mut std::io::Cursor::new(&mut out),
        ::image::ImageFormat::Png,
    )
    .unwrap();
    out
}

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let img = ::image::DynamicImage::ImageRgb8(::image::RgbImage::from_fn(w, h, |x, y| {
        ::image::Rgb([(x % 250) as u8, (y % 250) as u8, 100])
    }));
    img.write_to(
        &mut std::io::Cursor::new(&mut out),
        ::image::ImageFormat::Jpeg,
    )
    .unwrap();
    out
}

fn gif() -> Vec<u8> {
    let mut out = Vec::new();
    let img = ::image::DynamicImage::ImageRgba8(::image::RgbaImage::from_fn(16, 16, |x, y| {
        ::image::Rgba([(x * 16) as u8, (y * 16) as u8, 0, 255])
    }));
    img.write_to(
        &mut std::io::Cursor::new(&mut out),
        ::image::ImageFormat::Gif,
    )
    .unwrap();
    out
}

fn webp() -> Vec<u8> {
    let mut out = Vec::new();
    let img = ::image::RgbaImage::from_fn(20, 20, |x, y| {
        ::image::Rgba([(x * 12) as u8, (y * 12) as u8, 7, 255])
    });
    ::image::codecs::webp::WebPEncoder::new_lossless(&mut out)
        .encode(img.as_raw(), 20, 20, ::image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

fn magic(rec: &[u8]) -> &'static str {
    match crate::image::sniff(rec) {
        crate::image::Kind::Jpeg => "jpeg",
        crate::image::Kind::Png => "png",
        crate::image::Kind::Gif => "gif",
        crate::image::Kind::Webp => "webp",
        crate::image::Kind::Other => {
            if rec.starts_with(b"FONT") {
                "font"
            } else {
                "other"
            }
        }
    }
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rebuilt_at(book: &Book, pos: usize, fid: usize) -> String {
    let chunk = &book.chunks[pos];
    let skel = &book.skels[chunk.file];
    let doc = &book.docs[chunk.file];
    let at = chunk.insert_pos - skel.start + fid;
    doc[at..doc.len().min(at + 60)].to_string()
}

#[test]
fn epub2_with_ncx_guide_and_cover() {
    let spec = Spec::new("2.0");
    let mut spec = spec
        .item("ncx", "toc.ncx", "application/x-dtbncx+xml", "")
        .page("c1", "text/ch1.xhtml")
        .page("c2", "text/ch2.xhtml")
        .item("css", "text/style.css", "text/css", "")
        .item("cover-img", "images/cover.jpg", "image/jpeg", "")
        .item("pic", "images/pic.png", "image/png", "");
    spec.metadata += "<dc:publisher>Pub House</dc:publisher><dc:subject>Fiction</dc:subject><dc:subject>Tests</dc:subject><dc:identifier opf:scheme=\"ISBN\">9781234567897</dc:identifier><dc:date>2020-05-06</dc:date><dc:description>A &lt;b&gt;test&lt;/b&gt; book</dc:description><meta name=\"cover\" content=\"cover-img\"/>";
    spec.spine_attrs = "toc=\"ncx\"".into();
    spec.guide = "<reference type=\"text\" title=\"Start\" href=\"text/ch1.xhtml\"/>".into();
    let toc = ncx(&(point(1, "Chapter One", "text/ch1.xhtml", "")
        + &point(2, "Chapter Two", "text/ch2.xhtml#sec", "")));
    let files = spec.files(vec![
        f("OEBPS/toc.ncx", toc),
        f("OEBPS/text/ch1.xhtml", xhtml("One", "<h1 id=\"top\">Chapter One</h1><p>Hello <a name=\"old\"/>world &amp; friends.</p><p><img src=\"../images/pic.png\" alt=\"pic\"/> <a href=\"ch2.xhtml#sec\">next</a></p>")),
        f("OEBPS/text/ch2.xhtml", xhtml("Two", "<h1>Chapter Two</h1><p id=\"sec\">Second chapter with <i>style</i>.</p>")),
        f("OEBPS/text/style.css", "p { margin: 0; background: url(../images/pic.png) }"),
        f("OEBPS/images/cover.jpg", jpeg(60, 80)),
        f("OEBPS/images/pic.png", png(10, 10, false)),
    ]);
    let book = convert_book(&files);
    assert_eq!(book.title, "Test Book");
    assert_eq!(book.docs.len(), 2);
    assert_eq!(book.flows.len(), 2);
    assert!(
        String::from_utf8_lossy(&book.flows[1]).contains("url(kindle:embed:0003?mime=image/png)")
    );
    assert_eq!(
        norm(&book.all_text()),
        "Chapter OneHello world & friends. nextChapter TwoSecond chapter with style."
    );
    assert_eq!(book.exth_str(100), ["Ann Author"]);
    assert_eq!(book.exth_str(101), ["Pub House"]);
    assert_eq!(book.exth_str(103), ["A <b>test</b> book"]);
    assert_eq!(book.exth_str(104), ["9781234567897"]);
    assert_eq!(book.exth_str(105), ["Fiction", "Tests"]);
    assert_eq!(book.exth_str(106), ["2020-05-06T00:00:00+00:00"]);
    assert_eq!(book.exth_str(113), ["12345678-1234-1234-1234-123456789abc"]);
    assert_eq!(book.exth_str(501), ["EBOK"]);
    assert_eq!(book.exth_str(503), ["Test Book"]);
    assert_eq!(book.exth_str(524), ["en"]);
    assert_eq!(book.exth_u32(204), Some(201));
    assert_eq!(book.exth_str(535), ["0730-890adc2"]);
    assert_eq!(book.exth_str(528), ["true"]);
    assert_eq!(book.exth_u32(125), Some(3));
    assert_eq!(book.exth_u32(131), Some(0));
    assert_eq!(book.locale, 9);
    let res = book.resources();
    assert_eq!(res.len(), 3);
    let cover = book.exth_u32(201).unwrap() as usize;
    let thumb = book.exth_u32(202).unwrap() as usize;
    assert_eq!(magic(&res[cover]), "jpeg");
    assert_eq!(magic(&res[thumb]), "jpeg");
    assert_eq!(thumb, cover + 1);
    assert_eq!(book.exth_str(129), ["kindle:embed:0001"]);
    assert_eq!(magic(&res[2]), "png");
    assert_eq!(book.ncx.len(), 2);
    assert_eq!(book.ncx[0].label, "Chapter One");
    assert_eq!(book.ncx[1].label, "Chapter Two");
    assert!(rebuilt_at(&book, book.ncx[1].pos, book.ncx[1].fid).contains("id=\"sec\""));
    assert_eq!(book.guide.len(), 1);
    assert_eq!(book.guide[0].1, "text");
    assert_eq!(book.exth_u32(116), Some(book.ncx[0].offset as u32));
    let ch1 = &book.docs[0];
    assert!(ch1.contains("src=\"kindle:embed:0003?mime=image/png\""));
    assert!(ch1.contains("href=\"kindle:flow:0001?mime=text/css\""));
    assert!(ch1.contains("<a name=\"old\" id=\"old\" aid="));
    assert!(ch1.contains("<a href=\"kindle:pos:fid:0001:off:"));
}

#[test]
fn nested_toc_from_nav_document() {
    let mut spec = Spec::new("3.0")
        .item("nav", "nav.xhtml", "application/xhtml+xml", "nav")
        .page("c1", "c1.xhtml")
        .page("c2", "c2.xhtml");
    spec.metadata += "<dc:date>2021</dc:date>";
    let nav = "<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><body><nav epub:type=\"toc\"><ol>\
        <li><a href=\"c1.xhtml\">Part One</a><ol><li><a href=\"c1.xhtml#a\">Section A</a><ol><li><a href=\"c1.xhtml#a1\">Sub A1</a></li></ol></li><li><a href=\"c1.xhtml#b\">Section B</a></li></ol></li>\
        <li><span>Part Two</span><ol><li><a href=\"c2.xhtml#x\">Section X</a></li><li><a href=\"c2.xhtml#y\">Section Y</a></li></ol></li>\
        <li><a href=\"gone.xhtml\">Dangling</a></li></ol></nav>\
        <nav epub:type=\"landmarks\"><ol><li><a epub:type=\"bodymatter\" href=\"c1.xhtml\">Start</a></li></ol></nav></body></html>";
    let files = spec.files(vec![
        f("OEBPS/nav.xhtml", nav),
        f("OEBPS/c1.xhtml", xhtml("c1", "<h1>One</h1><h2 id=\"a\">A</h2><p>a</p><h3 id=\"a1\">A1</h3><p>a1</p><h2 id=\"b\">B</h2><p>b</p>")),
        f("OEBPS/c2.xhtml", xhtml("c2", "<h1>Two</h1><h2 id=\"x\">X</h2><p>x</p><h2 id=\"y\">Y</h2><p>y</p>")),
    ]);
    let book = convert_book(&files);
    type Row = (String, usize, Option<usize>, Option<usize>, Option<usize>);
    let rows: Vec<Row> = book
        .ncx
        .iter()
        .map(|r| {
            (
                r.label.clone(),
                r.depth,
                r.parent,
                r.first_child,
                r.last_child,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("Part One".into(), 0, None, Some(2), Some(3)),
            ("Part Two".into(), 0, None, Some(4), Some(5)),
            ("Section A".into(), 1, Some(0), Some(6), Some(6)),
            ("Section B".into(), 1, Some(0), None, None),
            ("Section X".into(), 1, Some(1), None, None),
            ("Section Y".into(), 1, Some(1), None, None),
            ("Sub A1".into(), 2, Some(2), None, None),
        ]
    );
    for r in &book.ncx {
        let at = rebuilt_at(&book, r.pos, r.fid);
        if let Some(id) = r
            .label
            .strip_prefix("Section ")
            .or(r.label.strip_prefix("Sub "))
        {
            assert!(
                at.contains(&format!("id=\"{}\"", id.to_lowercase())),
                "{} -> {at}",
                r.label
            );
        }
    }
    assert!(book.ncx[1].offset > book.ncx[0].offset);
    assert_eq!(book.ncx[0].length, book.ncx[1].offset - book.ncx[0].offset);
    assert_eq!(book.guide[0].1, "text");
    assert_eq!(book.exth_str(106).len(), 1);
    assert_eq!(book.exth_str(106)[0], "2021-01-01T00:00:00+00:00");
}

#[test]
fn toc_falls_back_to_spine() {
    let spec = Spec::new("2.0")
        .page("a", "a.xhtml")
        .page("b", "b.xhtml")
        .page("c", "c.xhtml");
    let files = spec.files(vec![
        f("OEBPS/a.xhtml", xhtml("Book", "<h1>Alpha</h1><p>x</p>")),
        f("OEBPS/b.xhtml", xhtml("Book", "<p>nothing</p>")),
        f("OEBPS/c.xhtml", xhtml("Gamma title", "<p>y</p>")),
    ]);
    let book = convert_book(&files);
    let labels: Vec<&str> = book.ncx.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["Alpha", "Book", "Gamma title"]);
}

#[test]
fn many_small_files() {
    let mut spec = Spec::new("2.0").item("ncx", "toc.ncx", "application/x-dtbncx+xml", "");
    spec.spine_attrs = "toc=\"ncx\"".into();
    let mut extra = Files::new();
    let mut points = String::new();
    for i in 0..150 {
        spec = spec.page(&format!("p{i}"), &format!("p{i}.xhtml"));
        extra.push(f(&format!("OEBPS/p{i}.xhtml"), xhtml("t", &format!("<h2 id=\"h\">Part {i}</h2><p>Paragraph of file {i} <a href=\"p{}.xhtml#h\">next</a></p>", (i + 1) % 150))));
        points += &point(i + 1, &format!("Part {i}"), &format!("p{i}.xhtml#h"), "");
    }
    extra.push(f("OEBPS/toc.ncx", ncx(&points)));
    let book = convert_book(&spec.files(extra));
    assert_eq!(book.docs.len(), 150);
    assert_eq!(book.skels.len(), 150);
    assert_eq!(book.chunks.len(), 150);
    assert_eq!(book.ncx.len(), 150);
    for (i, row) in book.ncx.iter().enumerate() {
        assert_eq!(row.label, format!("Part {i}"));
        assert_eq!(book.chunks[row.pos].file, i);
    }
    assert!(norm(&book.all_text()).starts_with("Part 0Paragraph of file 0 nextPart 1Paragraph"));
}

#[test]
fn huge_single_file_is_chunked() {
    let mut body = String::from("<section id=\"all\"><h1 id=\"top\">Huge</h1>");
    for i in 0..3000 {
        body += &format!(
            "<p id=\"p{i}\">Paragraph number {i}: the quick brown fox jumps over the lazy dog, again and again and again.</p>\n"
        );
    }
    body += "<div><div>";
    for i in 0..400 {
        body += &format!("<p>Nested {i} lorem ipsum dolor sit amet</p>");
    }
    body += "</div></div><p>";
    body += &"giant ".repeat(6000);
    body += "</p></section>";
    let spec = Spec::new("2.0").page("c", "c.xhtml");
    let files = spec.files(vec![f("OEBPS/c.xhtml", xhtml("t", &body))]);
    let (bytes, _dir) = convert_files(&files, &Metadata::default(), None);
    let book = verify::parse(&bytes);
    assert!(book.docs[0].len() > 300_000);
    assert!(book.chunks.len() > 50, "{}", book.chunks.len());
    assert_eq!(book.skels[0].chunk_count, book.chunks.len());
    for c in &book.chunks {
        assert!(c.length < 8192 + 64, "chunk of {} bytes", c.length);
    }
    assert!(book.docs[0].contains("<span class=\"AmznBigTextBlock\">"));
    let text = book.all_text();
    assert!(text.contains("Paragraph number 2999:"));
    assert_eq!(text.matches("giant ").count(), 6000);
    assert_eq!(text.matches("Nested ").count(), 400);
    assert!(book.text_length > 300_000);
    assert!(book.records.len() > 100);
}

#[test]
fn unicode_text_survives_record_boundaries() {
    let samples = [
        "日本語のテキストです。漢字とひらがな",
        "שלום עולם! זהו טקסט בעברית",
        "مرحبا بالعالم هذا نص عربي",
        "emoji 😀🎉 and café naïve ß",
        "한국어 텍스트 입니다",
    ];
    let mut body = String::new();
    let mut expected = String::new();
    for i in 0..400 {
        let s = samples[i % samples.len()];
        body += &format!("<p dir=\"auto\">{s} {i}</p>");
        expected += &format!("{s} {i}");
    }
    let spec = Spec::new("2.0").page("c", "c.xhtml");
    let files = spec.files(vec![f("OEBPS/c.xhtml", xhtml("日本語", &body))]);
    let (bytes, _dir) = convert_files(
        &files,
        &Metadata {
            title: Some("日本語 Title".into()),
            authors: vec!["Ünï Cödé".into()],
            languages: vec!["ja".into()],
            ..Default::default()
        },
        None,
    );
    let book = verify::parse(&bytes);
    assert_eq!(book.all_text(), expected);
    assert!(book.records.len() > 10);
    assert_eq!(book.title, "日本語 Title");
    assert_eq!(book.exth_str(100), ["Ünï Cödé"]);
    assert_eq!(book.locale, 17);
    assert_eq!(&bytes[..6], b"_Title");
}

#[test]
fn images_of_each_type() {
    let mut spec = Spec::new("3.0")
        .page("c", "c.xhtml")
        .item("jpg", "i/a.jpg", "image/jpeg", "")
        .item("png", "i/b.png", "image/png", "")
        .item("gif", "i/c.gif", "image/gif", "")
        .item("webp", "i/d.webp", "image/webp", "")
        .item("bmp", "i/e.bmp", "image/bmp", "")
        .item("big", "i/f.png", "image/png", "")
        .item("unused", "i/g.png", "image/png", "")
        .item("svgimg", "i/h.svg", "image/svg+xml", "");
    spec.metadata += "<meta name=\"cover\" content=\"png\"/>";
    let body = "<p><img src=\"i/a.jpg\"/><img src=\"i/b.png\"/><img src=\"i/c.gif\"/><img src=\"i/d.webp\"/><img src=\"i/e.bmp\"/><img src=\"i/f.png\"/><img src=\"i/h.svg\"/><img src=\"https://example.com/x.png\"/><img src=\"i/a.jpg\" srcset=\"i/a.jpg 2x\"/></p>";
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 10 10\"><image xlink:href=\"b.png\" width=\"10\" height=\"10\"/></svg>";
    let files = spec.files(vec![
        f("OEBPS/c.xhtml", xhtml("t", body)),
        f("OEBPS/i/a.jpg", jpeg(32, 32)),
        f("OEBPS/i/b.png", png(40, 60, true)),
        f("OEBPS/i/c.gif", gif()),
        f("OEBPS/i/d.webp", webp()),
        f("OEBPS/i/e.bmp", b"BM not a real bitmap"),
        f("OEBPS/i/f.png", png(4300, 40, false)),
        f("OEBPS/i/g.png", png(5, 5, false)),
        f("OEBPS/i/h.svg", svg),
    ]);
    let book = convert_book(&files);
    let res = book.resources();
    let kinds: Vec<&str> = res.iter().map(|r| magic(r)).collect();
    assert_eq!(
        kinds,
        ["jpeg", "jpeg", "jpeg", "gif", "jpeg", "jpeg", "gif"]
    );
    assert_eq!(res[6], crate::image::PLACEHOLDER_GIF);
    assert_eq!(::image::load_from_memory(&res[5]).unwrap().width(), 3000);
    assert_eq!(book.exth_u32(201), Some(1));
    assert_eq!(book.exth_u32(202), Some(2));
    assert_eq!(book.exth_u32(125), Some(7));
    let doc = &book.docs[0];
    for src in [
        "kindle:embed:0001?mime=image/jpeg",
        "kindle:embed:0002?mime=image/jpeg",
        "kindle:embed:0004?mime=image/gif",
        "kindle:embed:0005?mime=image/jpeg",
        "kindle:embed:0006?mime=image/jpeg",
        "kindle:flow:0001?mime=image/svg+xml",
        "https://example.com/x.png",
        "i/e.bmp",
    ] {
        assert!(doc.contains(&format!("src=\"{src}\"")), "{src}");
    }
    assert!(!doc.contains("srcset"));
    let svg_flow = String::from_utf8(book.flows[1].clone()).unwrap();
    assert!(svg_flow.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg "));
    assert!(svg_flow.contains("xlink:href=\"kindle:embed:0002?mime=image/jpeg\""));
}

#[test]
fn missing_cover_and_cover_override() {
    let spec = Spec::new("2.0").page("c", "c.xhtml");
    let files = spec.files(vec![f("OEBPS/c.xhtml", xhtml("t", "<p>text</p>"))]);
    let book = convert_book(&files);
    assert!(book.exth(201).is_empty() && book.exth(202).is_empty() && book.exth(129).is_empty());
    assert_eq!(book.first_resource, None);
    assert_eq!(book.exth_u32(125), Some(0));
    assert!(book.exth(131).is_empty());

    let cover = png(300, 500, true);
    let (bytes, _dir) = convert_files(&files, &Metadata::default(), Some(&cover));
    let book = verify::parse(&bytes);
    let res = book.resources();
    assert_eq!(res.len(), 2);
    assert_eq!(book.exth_u32(201), Some(0));
    assert_eq!(book.exth_u32(202), Some(1));
    assert_eq!(magic(&res[0]), "jpeg");
    let thumb = ::image::load_from_memory(&res[1]).unwrap();
    assert!(thumb.width() <= 180 && thumb.height() <= 240);
    assert!(res[1].len() <= 16 * 1024);

    let mut spec =
        Spec::new("2.0")
            .page("c", "c.xhtml")
            .item("ci", "cover.jpg", "image/jpeg", "cover-image");
    spec.metadata += "<meta name=\"cover\" content=\"ci\"/>";
    let files = spec.files(vec![
        f("OEBPS/c.xhtml", xhtml("t", "<p>text</p>")),
        f("OEBPS/cover.jpg", jpeg(10, 10)),
    ]);
    let (bytes, _dir) = convert_files(&files, &Metadata::default(), Some(&jpeg(77, 99)));
    let book = verify::parse(&bytes);
    let res = book.resources();
    assert_eq!(res.len(), 2);
    assert_eq!(::image::load_from_memory(&res[0]).unwrap().width(), 77);
}

#[test]
fn cover_page_in_spine_is_dropped() {
    let mut spec = Spec::new("2.0")
        .page("cp", "cover.xhtml")
        .page("c", "c.xhtml")
        .item("ci", "cover.jpg", "image/jpeg", "");
    spec.metadata += "<meta name=\"cover\" content=\"ci\"/>";
    spec.guide = "<reference type=\"cover\" title=\"Cover\" href=\"cover.xhtml\"/><reference type=\"text\" href=\"c.xhtml\"/>".into();
    let files = spec.files(vec![
        f(
            "OEBPS/cover.xhtml",
            xhtml("Cover", "<div><img src=\"cover.jpg\" alt=\"\"/></div>"),
        ),
        f("OEBPS/c.xhtml", xhtml("c", "<p>real content</p>")),
        f("OEBPS/cover.jpg", jpeg(30, 40)),
    ]);
    let book = convert_book(&files);
    assert_eq!(book.docs.len(), 1);
    assert_eq!(book.guide.len(), 1);
    assert_eq!(book.guide[0].1, "text");
    assert_eq!(book.exth_u32(125), Some(2));
}

#[test]
fn broken_markup_is_repaired() {
    let spec = Spec::new("2.0")
        .page("c", "c.xhtml")
        .page("d", "d.xhtml")
        .page("e", "e.xhtml");
    let broken = "<html><head><title>x</title><body><p>one<p>two &nbsp;&copy; &unknown; 1 < 2 & 3<br><img src=x.png><b>bold<i>it</b></i><ul><li>a<li>b</ul><table><tr><td>cell<td>cell2</table><script>if (a && b) {}</script></body>";
    let files = spec.files(vec![
        f("OEBPS/c.xhtml", broken),
        f("OEBPS/d.xhtml", "<p>no html element &amp; <em>unclosed"),
        f("OEBPS/e.xhtml", "\u{feff}<?xml version=\"1.0\" encoding=\"iso-8859-1\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>caf\u{e9}</p></body></html>".chars().map(|c| if c == '\u{e9}' { '\u{e9}' } else { c }).collect::<String>()),
    ]);
    let book = convert_book(&files);
    assert_eq!(book.docs.len(), 3);
    let t0 = norm(
        &roxmltree::Document::parse(&book.docs[0])
            .unwrap()
            .descendants()
            .filter(|n| n.is_text())
            .map(|n| n.text().unwrap())
            .collect::<String>(),
    );
    assert_eq!(t0, "x onetwo \u{a9} &unknown; 1 < 2 & 3bolditabcellcell2");
    assert!(!book.docs[0].contains("<script"));
    let t1 = book.docs[1].clone();
    assert!(t1.contains("<em aid=") && t1.contains("no html element &amp; "));
    assert!(book.docs[2].contains("café"));
}

#[test]
fn css_fonts_and_inline_styles() {
    let mut spec = Spec::new("3.0")
        .page("c", "c.xhtml")
        .item("css", "css/main.css", "text/css", "")
        .item("css2", "css/extra.css", "text/css", "")
        .item("font", "fonts/a.ttf", "font/ttf", "")
        .item("ofont", "fonts/b.otf", "font/otf", "")
        .item("img", "img/bg.png", "image/png", "");
    spec.metadata += "<dc:date>1999-12-31T10:00:00Z</dc:date>";
    let ttf = {
        let mut d = vec![0u8, 1, 0, 0];
        let mut x = 12345u32;
        d.extend((0..6000).map(|_| {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            (x >> 24) as u8
        }));
        d
    };
    let css = "@import url(extra.css);\n@font-face { font-family: X; src: url(../fonts/a.ttf) format('truetype'), url('../fonts/b.otf'); }\nbody { background: url( '../img/bg.png' ) }";
    let html = "<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>t</title><link rel=\"stylesheet\" href=\"css/main.css\"/><style type=\"text/css\">p { background: url(img/bg.png) } @import \"css/extra.css\";</style><style>p { background: url(img/bg.png) } @import \"css/extra.css\";</style><style>   </style></head><body style=\"background-image: url(img/bg.png)\"><p>x</p></body></html>";
    let files = spec.files(vec![
        f("OEBPS/c.xhtml", html),
        f("OEBPS/css/main.css", css),
        f("OEBPS/css/extra.css", "a { color: red }"),
        f("OEBPS/fonts/a.ttf", &ttf),
        f("OEBPS/fonts/b.otf", b"not really a font at all, obfuscated"),
        f("OEBPS/img/bg.png", png(4, 4, false)),
        f("META-INF/encryption.xml", "<encryption xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\" xmlns:enc=\"http://www.w3.org/2001/04/xmlenc#\"><enc:EncryptedData><enc:EncryptionMethod Algorithm=\"http://www.idpf.org/2008/embedding\"/><enc:CipherData><enc:CipherReference URI=\"OEBPS/fonts/b.otf\"/></enc:CipherData></enc:EncryptedData></encryption>"),
    ]);
    let book = convert_book(&files);
    assert_eq!(book.flows.len(), 4);
    let main = String::from_utf8(book.flows[1].clone()).unwrap();
    assert!(main.contains("@import url(kindle:flow:0002?mime=text/css);"));
    assert!(main.contains("src: url(kindle:embed:0002) format('truetype'), url('../fonts/b.otf')"));
    assert!(main.contains("background: url(kindle:embed:0001?mime=image/png)"));
    assert_eq!(book.flows[2], b"a { color: red }");
    let inline = String::from_utf8(book.flows[3].clone()).unwrap();
    assert!(inline.contains("@import \"kindle:flow:0002?mime=text/css\";"));
    assert!(inline.contains("url(kindle:embed:0001?mime=image/png)"));
    let doc = &book.docs[0];
    assert_eq!(doc.matches("rel=\"stylesheet\"").count(), 3);
    assert!(doc.contains("href=\"kindle:flow:0001?mime=text/css\""));
    assert!(doc.contains("href=\"kindle:flow:0003?mime=text/css\""));
    assert!(doc.contains("style=\"background-image: url(kindle:embed:0001?mime=image/png)\""));
    let res = book.resources();
    assert_eq!(res.len(), 2);
    assert_eq!(magic(&res[0]), "png");
    assert_eq!(&res[1][..4], b"FONT");
    assert_eq!(u32::from_be_bytes(res[1][8..12].try_into().unwrap()), 0b11);
    assert_eq!(
        u32::from_be_bytes(res[1][4..8].try_into().unwrap()) as usize,
        ttf.len()
    );
    let rec = &res[1];
    let (data_start, key_len, key_start) = (
        verify::u32_at(rec, 12),
        verify::u32_at(rec, 16),
        verify::u32_at(rec, 20),
    );
    let mut packed = rec[data_start..].to_vec();
    for i in 0..1040 {
        packed[i] ^= rec[key_start + i % key_len];
    }
    let mut unpacked = Vec::new();
    flate2::read::ZlibDecoder::new(&packed[..])
        .read_to_end(&mut unpacked)
        .unwrap();
    assert_eq!(unpacked, ttf);
    assert_eq!(book.exth_str(106), ["1999-12-31T10:00:00Z"]);
    let r0 = &book.records[0];
    assert_eq!(verify::u32_at(r0, 128), 0x50 | 0x1000);
}

#[test]
fn svg_pages_become_flows() {
    let spec = Spec::new("3.0")
        .page("c", "c.xhtml")
        .item("img", "p.jpg", "image/jpeg", "");
    let body = "<div><svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" version=\"1.1\" viewBox=\"0 0 20 30\" preserveAspectRatio=\"xMidYMid meet\"><image width=\"20\" height=\"30\" xlink:href=\"p.jpg\"/></svg></div><p>after</p>";
    let files = spec.files(vec![
        f("OEBPS/c.xhtml", xhtml("t", body)),
        f("OEBPS/p.jpg", jpeg(20, 30)),
    ]);
    let book = convert_book(&files);
    assert_eq!(book.flows.len(), 2);
    let svg = String::from_utf8(book.flows[1].clone()).unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" "));
    assert!(svg.contains("preserveAspectRatio=\"xMidYMid meet\""));
    assert!(svg.contains("xlink:href=\"kindle:embed:0001?mime=image/jpeg\""));
    roxmltree::Document::parse(&svg).unwrap();
    assert!(book.docs[0].contains("<img src=\"kindle:flow:0001?mime=image/svg+xml\""));
    assert!(!book.docs[0].contains("<svg"));
}

#[test]
fn internal_links_resolve_to_chunks() {
    let spec = Spec::new("2.0")
        .page("a", "a.xhtml")
        .page("b", "dir/b.xhtml");
    let mut big = String::from("<h1 id=\"start\">B</h1>");
    for i in 0..400 {
        big += &format!(
            "<p>Filler paragraph {i} with some words to make the file big enough to need chunks.</p>"
        );
    }
    big += "<p id=\"deep\">Deep target</p>";
    let files = spec.files(vec![
        f("OEBPS/a.xhtml", xhtml("a", "<p><a href=\"dir/b.xhtml#deep\">deep</a> <a href=\"dir/b.xhtml\">file</a> <a href=\"#me\">self</a> <a href=\"dir/b.xhtml#nothing\">missing anchor</a> <a href=\"http://example.com/\">ext</a> <a href=\"nofile.xhtml\">gone</a> <a href=\"mailto:a@b.c\">mail</a> <span id=\"me\">me</span></p>")),
        f("OEBPS/dir/b.xhtml", xhtml("b", &big)),
    ]);
    let book = convert_book(&files);
    let a = &book.docs[0];
    let mut targets = Vec::new();
    let mut rest = a.as_str();
    while let Some(i) = rest.find("kindle:pos:fid:") {
        let l = &rest[i..i + 34];
        targets.push((
            usize::from_str_radix(&l[15..19], 32).unwrap(),
            usize::from_str_radix(&l[24..34], 32).unwrap(),
        ));
        rest = &rest[i + 34..];
    }
    assert_eq!(targets.len(), 4);
    let snip = |t: (usize, usize)| rebuilt_at(&book, t.0, t.1);
    assert!(
        snip(targets[0]).contains("id=\"deep\""),
        "{}",
        snip(targets[0])
    );
    assert!(snip(targets[1]).starts_with("<body") || book.chunks[targets[1].0].file == 1);
    assert!(snip(targets[2]).contains("id=\"me\""));
    assert_eq!(book.chunks[targets[3].0].file, 1);
    assert!(a.contains("href=\"http://example.com/\""));
    assert!(a.contains("href=\"nofile.xhtml\""));
    assert!(a.contains("href=\"mailto:a@b.c\""));
    assert!(book.chunks.len() > 5);
}

#[test]
fn drm_is_rejected() {
    let spec = Spec::new("2.0").page("c", "c.xhtml");
    let page = f("OEBPS/c.xhtml", xhtml("t", "<p>x</p>"));
    let adept = f(
        "META-INF/encryption.xml",
        "<encryption xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\" xmlns:enc=\"http://www.w3.org/2001/04/xmlenc#\"><enc:EncryptedData><enc:EncryptionMethod Algorithm=\"http://www.w3.org/2001/04/xmlenc#aes128-cbc\"/><enc:CipherData><enc:CipherReference URI=\"OEBPS/c.xhtml\"/></enc:CipherData></enc:EncryptedData></encryption>",
    );
    assert!(matches!(
        convert_err(&spec.files(vec![page.clone(), adept])),
        Error::Drm
    ));
    let rights = f("META-INF/rights.xml", "<rights/>");
    assert!(matches!(
        convert_err(&spec.files(vec![page, rights])),
        Error::Drm
    ));
}

#[test]
fn malformed_inputs_fail_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("out.azw3");
    let meta = Metadata::default();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty.epub", vec![]),
        ("text.epub", b"hello".to_vec()),
        ("trunc.epub", b"PK\x03\x04garbage".to_vec()),
    ];
    for (name, data) in cases {
        let p = dir.path().join(name);
        std::fs::write(&p, data).unwrap();
        assert!(epub_to_azw3(&p, &dst, &meta, None).is_err(), "{name}");
    }
    assert!(matches!(
        epub_to_azw3(&dir.path().join("missing.epub"), &dst, &meta, None),
        Err(Error::Io(_))
    ));
    let no_container = write_epub(dir.path(), "nc.epub", &vec![f("x.txt", "x")]);
    assert!(matches!(
        epub_to_azw3(&no_container, &dst, &meta, None),
        Err(Error::Invalid(_))
    ));
    let empty_spine = Spec::new("2.0").item("a", "a.png", "image/png", "");
    let p = write_epub(
        dir.path(),
        "es.epub",
        &empty_spine.files(vec![f("OEBPS/a.png", png(2, 2, false))]),
    );
    assert!(matches!(
        epub_to_azw3(&p, &dst, &meta, None),
        Err(Error::Invalid(_))
    ));
    let missing_page = Spec::new("2.0").page("c", "c.xhtml");
    let p = write_epub(dir.path(), "mp.epub", &missing_page.files(vec![]));
    assert!(matches!(
        epub_to_azw3(&p, &dst, &meta, None),
        Err(Error::Invalid(_))
    ));
    assert!(!dst.exists());
}

#[test]
fn metadata_overrides_the_epub() {
    let mut spec = Spec::new("2.0").page("c", "c.xhtml");
    spec.metadata += "<dc:publisher>Old Pub</dc:publisher><dc:subject>old</dc:subject>";
    let files = spec.files(vec![f("OEBPS/c.xhtml", xhtml("t", "<p>x</p>"))]);
    let mut meta = Metadata {
        title: Some("Library  Title".into()),
        authors: vec!["First Author".into(), "Second Author".into()],
        publisher: Some("New Pub".into()),
        comments: Some("<p>Summary</p>".into()),
        tags: vec!["one".into(), "two".into()],
        pubdate: Some("2001-02-03".into()),
        languages: vec!["de-CH".into()],
        uuid: Some("abcd".into()),
        ..Default::default()
    };
    meta.identifiers
        .insert("isbn".into(), "9780306406157".into());
    let (bytes, _dir) = convert_files(&files, &meta, None);
    let book = verify::parse(&bytes);
    assert_eq!(book.title, "Library  Title");
    assert_eq!(book.exth_str(503), ["Library Title"]);
    assert_eq!(book.exth_str(100), ["First Author", "Second Author"]);
    assert_eq!(book.exth_str(101), ["New Pub"]);
    assert_eq!(book.exth_str(103), ["<p>Summary</p>"]);
    assert_eq!(book.exth_str(104), ["9780306406157"]);
    assert_eq!(book.exth_str(105), ["one", "two"]);
    assert_eq!(book.exth_str(106), ["2001-02-03T00:00:00+00:00"]);
    assert_eq!(book.exth_str(113), ["abcd"]);
    assert_eq!(book.exth_str(524), ["de-CH"]);
    assert_eq!(book.locale, 8 << 8 | 7);
    assert_eq!(&bytes[..18], b"Library__Title\0\0\0\0");
}

#[test]
fn rtl_books_get_direction_metadata() {
    let mut spec = Spec::new("3.0").page("c", "c.xhtml");
    spec.spine_attrs = "page-progression-direction=\"rtl\"".into();
    let files = spec.files(vec![f(
        "OEBPS/c.xhtml",
        xhtml("t", "<p>\u{5e9}\u{5dc}\u{5d5}\u{5dd}</p>"),
    )]);
    let book = convert_book(&files);
    assert_eq!(book.exth_str(525), ["horizontal-rl"]);
    assert_eq!(book.exth_str(527), ["rtl"]);
}

#[test]
fn output_is_written_atomically() {
    let spec = Spec::new("2.0").page("c", "c.xhtml");
    let files = spec.files(vec![f("OEBPS/c.xhtml", xhtml("t", "<p>x</p>"))]);
    let dir = tempfile::tempdir().unwrap();
    let src = write_epub(dir.path(), "in.epub", &files);
    let dst = dir.path().join("out.azw3");
    std::fs::write(&dst, b"old").unwrap();
    epub_to_azw3(&src, &dst, &Metadata::default(), None).unwrap();
    assert_eq!(&std::fs::read(&dst).unwrap()[60..68], b"BOOKMOBI");
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 2);
}

#[test]
fn can_convert_formats() {
    assert!(can_convert("epub", "azw3"));
    assert!(can_convert("EPUB", "AZW3"));
    assert!(can_convert("kepub", "Azw3"));
    assert!(can_convert(".epub", ".azw3"));
    assert!(!can_convert("azw3", "epub"));
    assert!(!can_convert("pdf", "azw3"));
    assert!(!can_convert("epub", "mobi"));
}

#[test]
fn kobo_spans_are_unwrapped_and_stale_aids_dropped() {
    let spec = Spec::new("3.0").page("c", "c.xhtml");
    let body = "<p><span class=\"koboSpan\" id=\"kobo.1.1\">Hello </span><span class=\"koboSpan\" id=\"kobo.1.2\">world</span></p><p aid=\"zzz\" cid=\"q\">x</p>";
    let book = convert_book(&spec.files(vec![f("OEBPS/c.xhtml", xhtml("t", body))]));
    let doc = &book.docs[0];
    assert!(!doc.contains("koboSpan") && !doc.contains("zzz") && !doc.contains("cid="));
    assert!(doc.contains(">Hello world</p>"));
}

#[test]
fn text_and_attributes_are_escaped() {
    let spec = Spec::new("3.0").page("c", "c.xhtml");
    let body = "<p title='say \"hi\" &amp; &lt;go&gt;&#10;now' lang=\"en\" xml:lang=\"de\" data-x=\"a\tb\">a &lt; b &amp;&amp; c &gt; d ]]&gt; &#x1F600; <![CDATA[<raw> & text]]> &quot;q&quot; &apos;</p><p>&lt;script&gt;alert(1)&lt;/script&gt;</p>";
    let book = convert_book(&spec.files(vec![f("OEBPS/c.xhtml", xhtml("t", body))]));
    let doc = &book.docs[0];
    assert!(
        doc.contains("title=\"say &quot;hi&quot; &amp; &lt;go&gt;&#10;now\""),
        "{doc}"
    );
    assert!(doc.contains("data-x=\"a b\""));
    assert!(doc.contains(" lang=\"de\""));
    assert!(
        doc.contains(
            "a &lt; b &amp;&amp; c &gt; d ]]&gt; \u{1F600} &lt;raw&gt; &amp; text \"q\" '</p>"
        ),
        "{doc}"
    );
    assert!(doc.contains("<p aid=\"") && doc.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
}

#[test]
fn undecodable_cover_override_is_ignored() {
    let mut spec = Spec::new("2.0")
        .page("c", "c.xhtml")
        .item("ci", "cover.jpg", "image/jpeg", "");
    spec.metadata += "<meta name=\"cover\" content=\"ci\"/>";
    let files = spec.files(vec![
        f("OEBPS/c.xhtml", xhtml("t", "<p>text</p>")),
        f("OEBPS/cover.jpg", jpeg(21, 34)),
    ]);
    let (bytes, _dir) = convert_files(&files, &Metadata::default(), Some(b"garbage"));
    let book = verify::parse(&bytes);
    assert_eq!(
        ::image::load_from_memory(&book.resources()[0])
            .unwrap()
            .width(),
        21
    );

    let plain = Spec::new("2.0")
        .page("c", "c.xhtml")
        .files(vec![f("OEBPS/c.xhtml", xhtml("t", "<p>text</p>"))]);
    let (bytes, _dir) = convert_files(&plain, &Metadata::default(), Some(b"garbage"));
    let book = verify::parse(&bytes);
    assert!(book.exth(201).is_empty());
}

#[test]
fn mutated_markup_never_panics() {
    let base = xhtml(
        "Fuzz",
        "<h1 id=\"a\">Title</h1><p class=\"x\">Some <b>bold</b> and <a href=\"#a\">link</a> text &amp; &nbsp; more<br/><img src=\"i.png\" alt=\"x\"/></p><svg xmlns=\"http://www.w3.org/2000/svg\"><image xlink:href=\"i.png\"/></svg><table><tr><td id=\"t\">cell</td></tr></table><style>p { background: url(i.png) } @import \"x.css\";</style><ul><li>one<li>two</ul>",
    );
    let chars: Vec<char> = "<>&/\"'=;!-[]? \n#:\u{e9}\u{4e2d}".chars().collect();
    let mut x = 987654321u64;
    let mut next = move |n: usize| {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((x >> 33) as usize) % n.max(1)
    };
    let dir = tempfile::tempdir().unwrap();
    for round in 0..150 {
        let mut doc: Vec<char> = base.chars().collect();
        for _ in 0..1 + next(6) {
            let at = next(doc.len());
            match next(4) {
                0 => doc.insert(at, chars[next(chars.len())]),
                1 => {
                    let end = (at + 1 + next(8)).min(doc.len());
                    doc.drain(at..end);
                }
                2 => {
                    let end = (at + 1 + next(30)).min(doc.len());
                    let piece: Vec<char> = doc[at..end].to_vec();
                    let to = next(doc.len());
                    doc.splice(to..to, piece);
                }
                _ => doc.truncate(at.max(10)),
            }
        }
        let doc: String = doc.into_iter().collect();
        let css = "p { color: red } @import url(\"a.css\"; @import \"x\u{e9}; url( ";
        let spec = Spec::new("2.0")
            .page("c", "c.xhtml")
            .item("css", "style.css", "text/css", "")
            .item("img", "i.png", "image/png", "");
        let files = spec.files(vec![
            f("OEBPS/c.xhtml", doc),
            f("OEBPS/style.css", css),
            f("OEBPS/i.png", png(3, 3, false)),
        ]);
        let src = write_epub(dir.path(), &format!("m{round}.epub"), &files);
        let dst = dir.path().join("out.azw3");
        if epub_to_azw3(&src, &dst, &Metadata::default(), None).is_ok() {
            verify::parse(&std::fs::read(&dst).unwrap());
        }
    }
}

#[test]
fn mutated_package_never_panics() {
    let mut spec = Spec::new("3.0")
        .item("nav", "nav.xhtml", "application/xhtml+xml", "nav")
        .item("ncx", "toc.ncx", "application/x-dtbncx+xml", "")
        .page("c", "c.xhtml")
        .item("cov", "i.png", "image/png", "cover-image");
    spec.spine_attrs = "toc=\"ncx\"".into();
    spec.guide = "<reference type=\"cover\" title=\"C\" href=\"c.xhtml#x\"/><reference type=\"text\" href=\"c.xhtml\"/>".into();
    let base = spec.files(vec![
        f("OEBPS/nav.xhtml", "<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><body><nav epub:type=\"toc\"><ol><li><a href=\"c.xhtml#x\">X</a><ol><li><span>Y</span></li></ol></li></ol></nav></body></html>"),
        f("OEBPS/toc.ncx", ncx(&point(1, "One", "c.xhtml#x", &point(2, "Two", "c.xhtml", "")))),
        f("OEBPS/c.xhtml", xhtml("t", "<p id=\"x\">text</p>")),
        f("OEBPS/i.png", png(3, 3, false)),
        f("META-INF/encryption.xml", "<encryption xmlns:enc=\"http://www.w3.org/2001/04/xmlenc#\"><enc:EncryptedData><enc:EncryptionMethod Algorithm=\"http://www.idpf.org/2008/embedding\"/><enc:CipherData><enc:CipherReference URI=\"OEBPS/f.ttf\"/></enc:CipherData></enc:EncryptedData></encryption>"),
    ]);
    let chars: Vec<char> = "<>&/\"'=;!-[]? \n#:%\u{e9}".chars().collect();
    let mut x = 5u64;
    let mut next = move |n: usize| {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((x >> 33) as usize) % n.max(1)
    };
    let dir = tempfile::tempdir().unwrap();
    for round in 0..150 {
        let mut files = base.clone();
        for _ in 0..1 + next(3) {
            let idx = next(files.len());
            if files[idx].0.ends_with(".png") {
                continue;
            }
            let mut doc: Vec<char> = String::from_utf8_lossy(&files[idx].1).chars().collect();
            for _ in 0..1 + next(4) {
                let at = next(doc.len());
                match next(3) {
                    0 => doc.insert(at, chars[next(chars.len())]),
                    1 => {
                        let end = (at + 1 + next(12)).min(doc.len());
                        doc.drain(at..end);
                    }
                    _ => doc.truncate(at.max(5)),
                }
            }
            files[idx].1 = doc.into_iter().collect::<String>().into_bytes();
        }
        let src = write_epub(dir.path(), &format!("p{round}.epub"), &files);
        let dst = dir.path().join("out.azw3");
        if epub_to_azw3(&src, &dst, &Metadata::default(), None).is_ok() {
            verify::parse(&std::fs::read(&dst).unwrap());
        }
    }
}
