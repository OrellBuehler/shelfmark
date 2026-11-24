use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::json;
use shelfmark_model::{IdentifyQuery, Metadata};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::*;

fn jpeg(tag: u8) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0];
    v.extend(std::iter::repeat_n(tag, 400));
    v
}

fn png(tag: u8) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend(std::iter::repeat_n(tag, 400));
    v
}

fn zip_file(path: &Path, entries: &[(&str, Vec<u8>)], mimetype_first: bool) {
    let mut w = ZipWriter::new(fs::File::create(path).unwrap());
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    if mimetype_first {
        w.start_file("mimetype", stored).unwrap();
        w.write_all(b"application/epub+zip").unwrap();
    }
    for (name, data) in entries {
        w.start_file(*name, deflated).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap();
}

const CONTAINER: &str = r##"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"##;

const OPF2: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="uuid_id" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>The Test Book</dc:title>
    <dc:creator opf:role="aut" opf:file-as="Author, First">First Author</dc:creator>
    <dc:creator opf:role="aut" opf:file-as="Writer, Second">Second Writer</dc:creator>
    <dc:creator opf:role="edt">An Editor</dc:creator>
    <dc:contributor opf:role="bkp">calibre</dc:contributor>
    <dc:identifier id="uuid_id" opf:scheme="uuid">11111111-2222-3333-4444-555555555555</dc:identifier>
    <dc:identifier opf:scheme="calibre">11111111-2222-3333-4444-555555555555</dc:identifier>
    <dc:identifier opf:scheme="ISBN">978-3-16-148410-0</dc:identifier>
    <dc:identifier opf:scheme="MOBI-ASIN">B000FC1PJA</dc:identifier>
    <dc:language>de-CH</dc:language>
    <dc:language>en</dc:language>
    <dc:subject>Fiction</dc:subject>
    <dc:subject>Adventure</dc:subject>
    <dc:publisher>Test &amp; Sons</dc:publisher>
    <dc:date>2008-05-12T10:00:00+00:00</dc:date>
    <dc:description>&lt;p&gt;A &lt;b&gt;fine&lt;/b&gt; book.&lt;/p&gt;</dc:description>
    <meta name="cover" content="cover-img"/>
    <meta name="calibre:series" content="The Series"/>
    <meta name="calibre:series_index" content="2.5"/>
    <meta name="calibre:rating" content="8.0"/>
    <meta name="calibre:title_sort" content="Test Book, The"/>
  </metadata>
  <manifest>
    <item id="cover-img" href="images/cover.jpg" media-type="image/jpeg"/>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"##;

const OPF3: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="pub-id">urn:isbn:9783161484100</dc:identifier>
    <dc:title id="t1">Der Titel</dc:title>
    <meta refines="#t1" property="title-type">main</meta>
    <meta refines="#t1" property="file-as">Titel, Der</meta>
    <dc:creator id="c1">Anna Autorin</dc:creator>
    <meta refines="#c1" property="role" scheme="marc:relators">aut</meta>
    <meta refines="#c1" property="file-as">Autorin, Anna</meta>
    <dc:language>deu</dc:language>
    <dc:date>2019</dc:date>
    <meta property="belongs-to-collection" id="s1">Die Reihe</meta>
    <meta refines="#s1" property="collection-type">series</meta>
    <meta refines="#s1" property="group-position">3</meta>
    <meta property="dcterms:modified">2020-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c" href="Images/Front%20Cover.png" media-type="image/png" properties="cover-image"/>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"##;

fn epub2(dir: &Path) -> PathBuf {
    let p = dir.join("two.epub");
    zip_file(
        &p,
        &[
            ("META-INF/container.xml", CONTAINER.as_bytes().to_vec()),
            ("OEBPS/content.opf", OPF2.as_bytes().to_vec()),
            ("OEBPS/images/cover.jpg", jpeg(1)),
            ("OEBPS/ch1.xhtml", b"<html><body>hi</body></html>".to_vec()),
        ],
        true,
    );
    p
}

fn epub3(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    zip_file(
        &p,
        &[
            ("META-INF/container.xml", CONTAINER.as_bytes().to_vec()),
            ("OEBPS/content.opf", OPF3.as_bytes().to_vec()),
            ("OEBPS/Images/Front Cover.png", png(2)),
            ("OEBPS/ch1.xhtml", b"<html><body>hi</body></html>".to_vec()),
        ],
        true,
    );
    p
}

fn entries(path: &Path) -> Vec<(String, Vec<u8>, CompressionMethod)> {
    let mut z = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut b = Vec::new();
            f.read_to_end(&mut b).unwrap();
            (f.name().unwrap().to_string(), b, f.compression())
        })
        .collect()
}

#[test]
fn epub2_read() {
    let d = tempfile::tempdir().unwrap();
    let p = epub2(d.path());
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("The Test Book"));
    assert_eq!(m.authors, vec!["First Author", "Second Writer"]);
    assert_eq!(
        m.author_sort.as_deref(),
        Some("Author, First & Writer, Second")
    );
    assert_eq!(m.title_sort.as_deref(), Some("Test Book, The"));
    assert_eq!(m.series.as_deref(), Some("The Series"));
    assert_eq!(m.series_index, Some(2.5));
    assert_eq!(m.rating, Some(8));
    assert_eq!(m.tags, vec!["Fiction", "Adventure"]);
    assert_eq!(m.publisher.as_deref(), Some("Test & Sons"));
    assert_eq!(m.pubdate.as_deref(), Some("2008-05-12T10:00:00+00:00"));
    assert_eq!(m.languages, vec!["deu", "eng"]);
    assert_eq!(m.comments.as_deref(), Some("<p>A <b>fine</b> book.</p>"));
    assert_eq!(
        m.uuid.as_deref(),
        Some("11111111-2222-3333-4444-555555555555")
    );
    assert_eq!(
        m.identifiers.get("isbn").map(String::as_str),
        Some("9783161484100")
    );
    assert_eq!(
        m.identifiers.get("mobi-asin").map(String::as_str),
        Some("B000FC1PJA")
    );
    assert!(!m.identifiers.contains_key("calibre"));
    assert_eq!(read_cover(&p).unwrap(), Some(jpeg(1)));
}

#[test]
fn epub3_read() {
    let d = tempfile::tempdir().unwrap();
    let p = epub3(d.path(), "three.epub");
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("Der Titel"));
    assert_eq!(m.title_sort.as_deref(), Some("Titel, Der"));
    assert_eq!(m.authors, vec!["Anna Autorin"]);
    assert_eq!(m.author_sort.as_deref(), Some("Autorin, Anna"));
    assert_eq!(m.series.as_deref(), Some("Die Reihe"));
    assert_eq!(m.series_index, Some(3.0));
    assert_eq!(m.pubdate.as_deref(), Some("2019-01-01"));
    assert_eq!(m.languages, vec!["deu"]);
    assert_eq!(
        m.identifiers.get("isbn").map(String::as_str),
        Some("9783161484100")
    );
    assert_eq!(read_cover(&p).unwrap(), Some(png(2)));
}

#[test]
fn kepub_extension() {
    let d = tempfile::tempdir().unwrap();
    let p = epub3(d.path(), "Der Titel - Anna Autorin.kepub.epub");
    assert_eq!(util::extension(&p), "kepub");
    assert_eq!(
        read_metadata(&p).unwrap().title.as_deref(),
        Some("Der Titel")
    );
    assert!(read_cover(&p).unwrap().is_some());
}

#[test]
fn epub_messy() {
    let d = tempfile::tempdir().unwrap();
    let mut opf = vec![0xEF, 0xBB, 0xBF];
    opf.extend_from_slice(
        "<?xml version='1.0' encoding='iso-8859-1'?>\n<package version='2.0' unique-identifier='bid'>\n<metadata>\n\
<dc:title>Caf\u{e9}&nbsp;Noir &mdash; Part&#160;1</dc:title>\n<dc:creator>J. Doe</dc:creator>\n\
<dc:identifier id='bid'>urn:uuid:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</dc:identifier>\n\
<dc:language>fr_FR</dc:language><dc:date>May 3, 2001</dc:date>\n<dc:subject>a</dc:subject><dc:subject>a</dc:subject>\n\
<unclosed>\n</metadata>\n<manifest><item id='x' href='Text/cover.xhtml' media-type='application/xhtml+xml'/>\n\
<item id='i' href='IMAGES/pic.JPG' media-type='image/jpeg'/></manifest>\n\
<guide><reference type='cover' href='Text/cover.xhtml'/></guide></package>"
            .as_bytes(),
    );
    let p = d.path().join("messy.epub");
    zip_file(
        &p,
        &[
            (
                "META-INF/container.xml",
                CONTAINER
                    .replace("OEBPS/content.opf", "oebps/CONTENT.opf")
                    .into_bytes(),
            ),
            ("OEBPS/content.opf", opf),
            (
                "OEBPS/Text/cover.xhtml",
                br##"<html><body><div><img src="../IMAGES/pic.JPG" alt=""/></div></body>"##
                    .to_vec(),
            ),
            ("OEBPS/IMAGES/pic.JPG", jpeg(7)),
        ],
        false,
    );
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("Caf\u{e9} Noir \u{2014} Part 1"));
    assert_eq!(m.languages, vec!["fra"]);
    assert_eq!(m.pubdate.as_deref(), Some("2001-05-03"));
    assert_eq!(
        m.uuid.as_deref(),
        Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
    );
    assert_eq!(m.tags, vec!["a"]);
    assert_eq!(read_cover(&p).unwrap(), Some(jpeg(7)));
}

#[test]
fn epub_heuristic_cover_and_latin1() {
    let d = tempfile::tempdir().unwrap();
    let opf = "<?xml version=\"1.0\" encoding=\"windows-1252\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"2.0\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>\u{fc}ber alles</dc:title></metadata><manifest><item id=\"a\" href=\"a.png\" media-type=\"image/png\"/><item id=\"b\" href=\"book_cover.png\" media-type=\"image/png\"/></manifest></package>";
    let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(opf);
    let p = d.path().join("h.epub");
    zip_file(
        &p,
        &[
            ("content.opf", bytes.into_owned()),
            ("a.png", png(1)),
            ("book_cover.png", png(9)),
        ],
        true,
    );
    assert_eq!(
        read_metadata(&p).unwrap().title.as_deref(),
        Some("\u{fc}ber alles")
    );
    assert_eq!(read_cover(&p).unwrap(), Some(png(9)));
}

#[test]
fn epub_write_roundtrip_v2() {
    let d = tempfile::tempdir().unwrap();
    let p = epub2(d.path());
    let before = entries(&p);
    let mut m = read_metadata(&p).unwrap();
    m.title = Some("New Title & <More>".into());
    m.title_sort = Some("Title, New".into());
    m.authors = vec!["Zed One".into(), "Amy Two".into(), "Bob Three".into()];
    m.author_sort = Some("One, Zed & Two, Amy & Three, Bob".into());
    m.series = Some("Other Series".into());
    m.series_index = Some(10.0);
    m.tags = vec!["x".into(), "y z".into()];
    m.publisher = Some("Pub".into());
    m.pubdate = Some("1999-12-31".into());
    m.languages = vec!["spa".into()];
    m.rating = Some(6);
    m.comments = Some("<p>c</p> & more".into());
    m.identifiers.clear();
    m.identifiers.insert("isbn".into(), "9780306406157".into());
    m.identifiers.insert("google".into(), "abc123".into());
    let newcover = png(5);
    assert!(write_metadata(&p, &m, Some(&newcover)).unwrap());
    let back = read_metadata(&p).unwrap();
    assert_eq!(back, m);
    assert_eq!(read_cover(&p).unwrap(), Some(newcover));
    let after = entries(&p);
    assert_eq!(after[0].0, "mimetype");
    assert_eq!(after[0].2, CompressionMethod::Stored);
    assert_eq!(after[0].1, b"application/epub+zip");
    for (name, data, _) in &before {
        if name == "mimetype" || name == "OEBPS/content.opf" || name == "OEBPS/images/cover.jpg" {
            continue;
        }
        assert_eq!(
            after.iter().find(|(n, ..)| n == name).map(|e| &e.1),
            Some(data),
            "{name}"
        );
    }
    assert!(!d.path().join(".two.epub.shelfmark.tmp").exists());
    let opf = String::from_utf8(
        after
            .iter()
            .find(|e| e.0 == "OEBPS/content.opf")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    assert!(opf.contains("opf:role=\"aut\""));
    assert!(opf.contains("An Editor"), "non-author creators are kept");
    assert!(opf.contains("calibre</dc:contributor>"));
    assert!(opf.contains("media-type=\"image/png\""));
    let again = read_metadata(&p).unwrap();
    assert_eq!(again, m);
}

#[test]
fn epub_write_roundtrip_v3_add_cover() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("nocover.epub");
    zip_file(
        &p,
        &[
            ("META-INF/container.xml", CONTAINER.as_bytes().to_vec()),
            ("OEBPS/content.opf", OPF3.replace("<item id=\"c\" href=\"Images/Front%20Cover.png\" media-type=\"image/png\" properties=\"cover-image\"/>", "").into_bytes()),
        ],
        true,
    );
    assert_eq!(read_cover(&p).unwrap(), None);
    let mut m = read_metadata(&p).unwrap();
    m.title = Some("Neuer Titel".into());
    m.series = Some("Neue Reihe".into());
    m.series_index = Some(1.5);
    m.authors = vec!["A B".into(), "C D".into()];
    m.author_sort = Some("B, A & D, C".into());
    m.title_sort = Some("Titel, Neuer".into());
    m.uuid = Some("99999999-2222-3333-4444-555555555555".into());
    m.identifiers.insert("amazon".into(), "B00X".into());
    let cover = jpeg(4);
    write_metadata(&p, &m, Some(&cover)).unwrap();
    let back = read_metadata(&p).unwrap();
    assert_eq!(back, m);
    assert_eq!(read_cover(&p).unwrap(), Some(cover));
    let e = entries(&p);
    let opf = String::from_utf8(
        e.iter()
            .find(|e| e.0 == "OEBPS/content.opf")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    assert!(opf.contains("properties=\"cover-image\""));
    assert!(opf.contains("belongs-to-collection"));
    assert!(opf.contains("dcterms:modified"));
    assert!(!opf.contains("opf:role"));
    write_metadata(&p, &m, None).unwrap();
    assert_eq!(read_metadata(&p).unwrap(), m);
}

#[test]
fn epub_write_clears_fields() {
    let d = tempfile::tempdir().unwrap();
    let p = epub2(d.path());
    let mut m = read_metadata(&p).unwrap();
    m.series = None;
    m.series_index = None;
    m.rating = None;
    m.comments = None;
    m.tags.clear();
    m.publisher = None;
    m.pubdate = None;
    m.title_sort = None;
    write_metadata(&p, &m, None).unwrap();
    assert_eq!(read_metadata(&p).unwrap(), m);
}

#[test]
fn epub_write_on_garbage_keeps_file() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("bad.epub");
    fs::write(&p, b"not a zip").unwrap();
    assert!(write_metadata(&p, &Metadata::default(), None).is_err());
    assert_eq!(fs::read(&p).unwrap(), b"not a zip");
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("bad"));
    assert_eq!(read_cover(&p).unwrap(), None);
}

fn minimal_pdf(path: &Path) {
    use lopdf::{Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let content = doc.add_object(Stream::new(dictionary! {}, b"BT ET".to_vec()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content, "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()] });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 },
        ),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let info = doc.add_object(dictionary! { "Title" => Object::string_literal("Old Title"), "Author" => Object::string_literal("A One; B Two"), "Keywords" => Object::string_literal("k1, k2"), "Subject" => Object::string_literal("About it") });
    doc.trailer.set("Info", info);
    doc.save(path).unwrap();
}

#[test]
fn pdf_read_write() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("x.pdf");
    minimal_pdf(&p);
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("Old Title"));
    assert_eq!(m.authors, vec!["A One", "B Two"]);
    assert_eq!(m.tags, vec!["k1", "k2"]);
    assert_eq!(m.comments.as_deref(), Some("About it"));
    assert_eq!(read_cover(&p).unwrap(), None);

    let mut n = Metadata {
        title: Some("Gr\u{fc}\u{df}e \u{4e16}\u{754c}".into()),
        authors: vec!["Zo\u{eb} Q".into(), "Bob".into()],
        tags: vec!["t1".into(), "t2".into()],
        comments: Some("desc <b>".into()),
        publisher: Some("P".into()),
        pubdate: Some("2001-02-03".into()),
        languages: vec!["deu".into()],
        series: Some("S".into()),
        series_index: Some(2.0),
        rating: Some(4),
        uuid: Some("99999999-2222-3333-4444-555555555555".into()),
        ..Default::default()
    };
    n.identifiers.insert("isbn".into(), "9780306406157".into());
    assert!(write_metadata(&p, &n, None).unwrap());
    assert_eq!(read_metadata(&p).unwrap(), n);
    assert!(write_metadata(&p, &n, None).unwrap());
    assert_eq!(read_metadata(&p).unwrap(), n);
    assert!(Document::load(&p).is_ok());
}

use lopdf::Document;

fn put_u32(v: &mut [u8], at: usize, val: u32) {
    v[at..at + 4].copy_from_slice(&val.to_be_bytes());
}

fn minimal_mobi(path: &Path, utf8: bool) {
    let exth_items: Vec<(u32, Vec<u8>)> = vec![
        (100, b"Ann Author & Bob Writer".to_vec()),
        (101, b"Mobi Press".to_vec()),
        (103, b"A description".to_vec()),
        (104, b"978-3-16-148410-0".to_vec()),
        (105, b"Fantasy;Adventure".to_vec()),
        (106, b"2010-06-01T00:00:00+00:00".to_vec()),
        (113, b"B00ASIN123".to_vec()),
        (201, 1u32.to_be_bytes().to_vec()),
        (
            503,
            if utf8 {
                "Der \u{fc}berfall".as_bytes().to_vec()
            } else {
                vec![
                    b'D', b'e', b'r', b' ', 0xFC, b'b', b'e', b'r', b'f', b'a', b'l', b'l',
                ]
            },
        ),
        (524, b"de".to_vec()),
    ];
    let mut exth = b"EXTH".to_vec();
    exth.extend([0u8; 8]);
    for (t, d) in &exth_items {
        exth.extend(t.to_be_bytes());
        exth.extend(((d.len() + 8) as u32).to_be_bytes());
        exth.extend(d);
    }
    let n = exth_items.len() as u32;
    let elen = exth.len() as u32;
    put_u32(&mut exth, 4, elen);
    put_u32(&mut exth, 8, n);
    while !exth.len().is_multiple_of(4) {
        exth.push(0);
    }
    let hlen = 232usize;
    let mut r0 = vec![0u8; 16 + hlen];
    r0[16..20].copy_from_slice(b"MOBI");
    put_u32(&mut r0, 20, hlen as u32);
    put_u32(&mut r0, 28, if utf8 { 65001 } else { 1252 });
    put_u32(&mut r0, 36, 6);
    let name = b"Fallback Name";
    put_u32(&mut r0, 84, (16 + hlen + exth.len()) as u32);
    put_u32(&mut r0, 88, name.len() as u32);
    put_u32(&mut r0, 92, 9);
    put_u32(&mut r0, 108, 2);
    put_u32(&mut r0, 128, 0x40);
    r0.extend(&exth);
    r0.extend(name);
    let recs: Vec<Vec<u8>> = vec![r0, b"text".to_vec(), jpeg(3), jpeg(8)];
    let mut out = vec![0u8; 78 + recs.len() * 8 + 2];
    out[..8].copy_from_slice(b"TestBook");
    out[60..68].copy_from_slice(b"BOOKMOBI");
    out[76..78].copy_from_slice(&(recs.len() as u16).to_be_bytes());
    let mut off = out.len();
    for (i, r) in recs.iter().enumerate() {
        put_u32(&mut out, 78 + i * 8, off as u32);
        off += r.len();
    }
    for r in &recs {
        out.extend(r);
    }
    fs::write(path, out).unwrap();
}

#[test]
fn mobi_read() {
    let d = tempfile::tempdir().unwrap();
    for (ext, utf8) in [("mobi", true), ("azw3", false), ("azw", true)] {
        let p = d.path().join(format!("b.{ext}"));
        minimal_mobi(&p, utf8);
        let m = read_metadata(&p).unwrap();
        assert_eq!(m.title.as_deref(), Some("Der \u{fc}berfall"));
        assert_eq!(m.authors, vec!["Ann Author", "Bob Writer"]);
        assert_eq!(m.publisher.as_deref(), Some("Mobi Press"));
        assert_eq!(m.comments.as_deref(), Some("A description"));
        assert_eq!(
            m.identifiers.get("isbn").map(String::as_str),
            Some("9783161484100")
        );
        assert_eq!(
            m.identifiers.get("mobi-asin").map(String::as_str),
            Some("B00ASIN123")
        );
        assert_eq!(m.tags, vec!["Fantasy", "Adventure"]);
        assert_eq!(m.pubdate.as_deref(), Some("2010-06-01T00:00:00+00:00"));
        assert_eq!(m.languages, vec!["deu"]);
        assert_eq!(read_cover(&p).unwrap(), Some(jpeg(8)));
        assert!(!write_metadata(&p, &m, None).unwrap());
    }
}

#[test]
fn mobi_truncated_does_not_panic() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("b.mobi");
    minimal_mobi(&p, true);
    let full = fs::read(&p).unwrap();
    for cut in [0, 10, 77, 100, 150, 300, 500] {
        fs::write(&p, &full[..cut.min(full.len())]).unwrap();
        let m = read_metadata(&p).unwrap();
        assert_eq!(m.title.as_deref().map(|t| !t.is_empty()), Some(true));
        let _ = read_cover(&p);
    }
}

#[test]
fn fb2_read() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("b.fb2");
    let img = util::base64_encode(&jpeg(6));
    let xml = format!(
        r##"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<description><title-info><genre>sf</genre><author><first-name>Ivan</first-name><middle-name>I.</middle-name><last-name>Ivanov</last-name></author>
<book-title>Kniga</book-title><annotation><p>One</p><p>Two &amp; three</p></annotation><date value="2003-04-05">2003</date><lang>ru</lang>
<sequence name="Cycle" number="4"/><coverpage><image l:href="#cov.jpg"/></coverpage></title-info>
<publish-info><publisher>Izd</publisher><isbn>5-17-000000-0</isbn></publish-info></description>
<body><section><p>text</p></section></body><binary id="cov.jpg" content-type="image/jpeg">{img}</binary></FictionBook>"##
    );
    fs::write(&p, xml).unwrap();
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("Kniga"));
    assert_eq!(m.authors, vec!["Ivan I. Ivanov"]);
    assert_eq!(m.series.as_deref(), Some("Cycle"));
    assert_eq!(m.series_index, Some(4.0));
    assert_eq!(m.languages, vec!["rus"]);
    assert_eq!(m.pubdate.as_deref(), Some("2003-04-05"));
    assert_eq!(m.publisher.as_deref(), Some("Izd"));
    assert_eq!(
        m.comments.as_deref(),
        Some("<p>One</p>\n<p>Two &amp; three</p>")
    );
    assert_eq!(read_cover(&p).unwrap(), Some(jpeg(6)));
}

#[test]
fn cbz_read() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("Some Comic 01.cbz");
    zip_file(
        &p,
        &[
            ("page10.jpg", jpeg(10)),
            ("page2.jpg", jpeg(2)),
            ("page1.jpg", jpeg(1)),
            ("ComicInfo.xml", b"<ComicInfo><Series>Hero</Series><Number>3</Number><Writer>A, B</Writer><Year>2015</Year><Month>7</Month><Genre>Action, Drama</Genre><LanguageISO>en</LanguageISO></ComicInfo>".to_vec()),
        ],
        false,
    );
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("Hero 3"));
    assert_eq!(m.series_index, Some(3.0));
    assert_eq!(m.authors, vec!["A", "B"]);
    assert_eq!(m.pubdate.as_deref(), Some("2015-07-01"));
    assert_eq!(m.tags, vec!["Action", "Drama"]);
    assert_eq!(m.languages, vec!["eng"]);
    assert_eq!(read_cover(&p).unwrap(), Some(jpeg(1)));
    let q = d.path().join("plain.cbz");
    zip_file(&q, &[("b/2.png", png(2)), ("b/10.png", png(10))], false);
    assert_eq!(read_cover(&q).unwrap(), Some(png(2)));
    assert_eq!(read_metadata(&q).unwrap().title.as_deref(), Some("plain"));
}

#[test]
fn filename_fallback() {
    let d = tempfile::tempdir().unwrap();
    let p = d
        .path()
        .join("Great Expectations - Charles Dickens & Co Author.txt");
    fs::write(&p, "hello").unwrap();
    let m = read_metadata(&p).unwrap();
    assert_eq!(m.title.as_deref(), Some("Great Expectations"));
    assert_eq!(m.authors, vec!["Charles Dickens", "Co Author"]);
    let q = d.path().join("no_pattern.weird");
    fs::write(&q, "x").unwrap();
    assert_eq!(
        read_metadata(&q).unwrap().title.as_deref(),
        Some("no_pattern")
    );
    assert!(read_metadata(&d.path().join("missing.epub")).is_err());
    assert!(read_cover(&d.path().join("missing.epub")).is_err());
    assert!(!write_metadata(&p, &m, None).unwrap());
    assert!(BOOK_EXTENSIONS.contains(&"epub") && BOOK_EXTENSIONS.contains(&"azw3"));
}

#[test]
fn dates_and_langs() {
    use util::normalize_date as nd;
    assert_eq!(nd("2008").as_deref(), Some("2008-01-01"));
    assert_eq!(nd("2008-05").as_deref(), Some("2008-05-01"));
    assert_eq!(nd("0101-01-01T00:00:00+00:00"), None);
    assert_eq!(nd("12 March 1999").as_deref(), Some("1999-03-12"));
    assert_eq!(nd("garbage"), None);
    assert_eq!(lang::normalize("de-CH").as_deref(), Some("deu"));
    assert_eq!(lang::normalize("EN").as_deref(), Some("eng"));
    assert_eq!(lang::normalize("ger").as_deref(), Some("deu"));
    assert_eq!(lang::normalize("xx"), None);
    assert_eq!(
        util::parse_series("Discworld ; 4"),
        Some(("Discworld".into(), Some(4.0)))
    );
    assert_eq!(
        util::parse_series("Foo (3)"),
        Some(("Foo".into(), Some(3.0)))
    );
    assert_eq!(
        util::base64_decode(&util::base64_encode(b"hello world!?")),
        b"hello world!?"
    );
}

#[test]
fn xml_robustness() {
    for s in [
        "",
        "<",
        "<a",
        "<a b",
        "<a b=",
        "<a b='",
        "</a>",
        "<!--",
        "<![CDATA[",
        "<?",
        "&",
        "<a>&#xZZ;&#99999999;&amp</a>",
        "<a><b></a></b>",
    ] {
        let _ = xml::parse(s.as_bytes());
    }
    let d = xml::parse(b"<a><b>x &amp; y &bogus; &lt;</b><c/></a>").unwrap();
    assert_eq!(d.root.child("b").unwrap().text(), "x & y &bogus; <");
    assert_eq!(
        d.serialize().lines().last().unwrap(),
        "<a><b>x &amp; y &amp;bogus; &lt;</b><c/></a>"
    );
    let u16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(
            "<a>h\u{e9}</a>"
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes()),
        )
        .collect();
    assert_eq!(xml::parse(&u16).unwrap().root.text(), "h\u{e9}");
}

#[test]
fn google_parse() {
    let j = json!({"items": [
        {"id": "AbC123", "volumeInfo": {
            "title": "Dune", "subtitle": "Book One", "authors": ["Frank Herbert"], "publisher": "Ace",
            "publishedDate": "1990-09", "description": "Spice.", "categories": ["Fiction / Science Fiction"],
            "industryIdentifiers": [{"type": "ISBN_10", "identifier": "0441172717"}, {"type": "ISBN_13", "identifier": "9780441172719"}],
            "language": "en",
            "imageLinks": {"smallThumbnail": "http://books.google.com/books/content?id=AbC123&printsec=frontcover&img=1&zoom=5&edge=curl&source=gbs_api",
                           "thumbnail": "http://books.google.com/books/content?id=AbC123&printsec=frontcover&img=1&zoom=1&edge=curl&source=gbs_api"},
            "canonicalVolumeLink": "https://books.google.com/books/about/Dune.html?id=AbC123"}},
        {"id": "X", "volumeInfo": {"subtitle": "no title"}},
        {"id": "Y", "volumeInfo": {"title": "Bare"}}
    ]});
    let r = sources::parse_google(&j);
    assert_eq!(r.len(), 2);
    let m = &r[0].metadata;
    assert_eq!(m.title.as_deref(), Some("Dune: Book One"));
    assert_eq!(m.authors, vec!["Frank Herbert"]);
    assert_eq!(m.pubdate.as_deref(), Some("1990-09-01"));
    assert_eq!(m.tags, vec!["Fiction", "Science Fiction"]);
    assert_eq!(
        m.identifiers.get("isbn").map(String::as_str),
        Some("9780441172719")
    );
    assert_eq!(
        m.identifiers.get("google").map(String::as_str),
        Some("AbC123")
    );
    assert_eq!(m.languages, vec!["eng"]);
    assert_eq!(
        r[0].cover_url.as_deref(),
        Some(
            "https://books.google.com/books/content?id=AbC123&printsec=frontcover&img=1&zoom=3&source=gbs_api"
        )
    );
    assert_eq!(r[0].source, "Google Books");
    assert!(r[1].url.as_deref().unwrap().contains("id=Y"));
    assert!(sources::parse_google(&json!({"totalItems": 0})).is_empty());
}

#[test]
fn openlibrary_parse() {
    let j = json!({"docs": [{
        "key": "/works/OL893415W", "title": "Dune", "author_name": ["Frank Herbert"], "first_publish_year": 1965,
        "publisher": ["Ace", "Chilton"], "isbn": ["0441172717", "9780441172719"], "language": ["eng", "ger", "fre"],
        "subject": ["Science fiction", "Deserts"], "cover_i": 12345}, {"nokey": true}]});
    let r = sources::parse_ol_search(&j);
    assert_eq!(r.len(), 1);
    let m = &r[0].metadata;
    assert_eq!(m.pubdate.as_deref(), Some("1965-01-01"));
    assert_eq!(m.languages, vec!["eng", "deu", "fra"]);
    assert_eq!(
        m.identifiers.get("isbn").map(String::as_str),
        Some("9780441172719")
    );
    assert_eq!(
        m.identifiers.get("openlibrary").map(String::as_str),
        Some("OL893415W")
    );
    assert_eq!(
        r[0].cover_url.as_deref(),
        Some("https://covers.openlibrary.org/b/id/12345-L.jpg")
    );
    assert_eq!(
        r[0].url.as_deref(),
        Some("https://openlibrary.org/works/OL893415W")
    );

    let ed = json!({"key": "/books/OL7353617M", "title": "Fantastic Mr Fox", "publishers": ["Puffin"], "publish_date": "October 1, 1988",
        "isbn_13": ["9780140328721"], "languages": [{"key": "/languages/eng"}], "covers": [-1, 8739161],
        "series": ["Roald Dahl ; 2"], "works": [{"key": "/works/OL45804W"}]});
    let work =
        json!({"description": {"type": "/type/text", "value": "A fox."}, "subjects": ["Foxes"]});
    let r = sources::parse_ol_edition(
        &ed,
        &["Roald Dahl".to_string()],
        Some(&work),
        "9780140328721",
    )
    .unwrap();
    assert_eq!(r.metadata.comments.as_deref(), Some("A fox."));
    assert_eq!(r.metadata.pubdate.as_deref(), Some("1988-10-01"));
    assert_eq!(r.metadata.series.as_deref(), Some("Roald Dahl"));
    assert_eq!(r.metadata.series_index, Some(2.0));
    assert_eq!(r.metadata.tags, vec!["Foxes"]);
    assert_eq!(
        r.cover_url.as_deref(),
        Some("https://covers.openlibrary.org/b/id/8739161-L.jpg")
    );
}

#[test]
fn ranking() {
    let q = IdentifyQuery {
        title: Some("Dune".into()),
        authors: vec!["Frank Herbert".into()],
        isbn: None,
    };
    let good = Metadata {
        title: Some("Dune".into()),
        authors: vec!["Frank Herbert".into()],
        ..Default::default()
    };
    let bad = Metadata {
        title: Some("Dune Messiah Study Guide".into()),
        authors: vec!["Someone Else".into()],
        ..Default::default()
    };
    assert!(sources::score(&q, &good) > sources::score(&q, &bad));
}

#[tokio::test]
#[ignore]
async fn live_identify_isbn() {
    let q = IdentifyQuery {
        isbn: Some("9780140328721".into()),
        ..Default::default()
    };
    let r = sources::identify(&q).await;
    println!("{r:#?}");
    println!(
        "google results: {}",
        r.iter().filter(|r| r.source == "Google Books").count()
    );
    assert!(r.iter().any(|r| r.source == "Open Library"), "openlibrary");
    for r in &r {
        let url = r.cover_url.as_deref().unwrap_or("");
        if !url.is_empty() {
            let c = sources::fetch_cover(url).await;
            println!("{} cover {url}: {:?}", r.source, c.as_ref().map(Vec::len));
        }
    }
}

#[tokio::test]
#[ignore]
async fn live_identify_title() {
    let q = IdentifyQuery {
        title: Some("The Hobbit".into()),
        authors: vec!["J. R. R. Tolkien".into()],
        isbn: None,
    };
    let r = sources::identify(&q).await;
    println!(
        "{:#?}",
        r.iter()
            .map(|r| (
                &r.source,
                &r.metadata.title,
                &r.metadata.authors,
                &r.cover_url
            ))
            .collect::<Vec<_>>()
    );
    assert!(!r.is_empty());
    assert!(
        r[0].metadata
            .title
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains("hobbit")
    );
}
