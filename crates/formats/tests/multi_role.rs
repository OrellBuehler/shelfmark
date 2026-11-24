use std::io::Write;

#[test]
fn creator_with_several_roles_is_an_author() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.epub");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.start_file("META-INF/container.xml", stored).unwrap();
    zip.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
    zip.start_file("content.opf", stored).unwrap();
    zip.write_all(br##"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="uid">x</dc:identifier><dc:title>Whale</dc:title><dc:creator id="author">Some Writer</dc:creator><meta property="role" refines="#author" scheme="marc:relators">ann</meta><meta property="role" refines="#author" scheme="marc:relators">aut</meta><dc:creator id="ill">An Illustrator</dc:creator><meta property="role" refines="#ill" scheme="marc:relators">ill</meta></metadata><manifest/><spine/></package>"##).unwrap();
    zip.finish().unwrap();
    let meta = shelfmark_formats::read_metadata(&path).unwrap();
    assert_eq!(meta.authors, vec!["Some Writer".to_string()]);
}
