use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use shelfmark_model::BookId;

use crate::dates;
use crate::error::{Error, Result};
use crate::record::Record;
use crate::text::{self, PathRules};

pub const COVER_FILE_NAME: &str = "cover.jpg";
pub const TRASH_DIR_NAME: &str = ".caltrash";

pub struct Ctx<'a> {
    pub dir: &'a Path,
    pub rules: PathRules,
}

impl Ctx<'_> {
    pub fn join(&self, rel: &str) -> PathBuf {
        let mut p = self.dir.to_path_buf();
        for seg in rel.split('/').filter(|s| !s.is_empty()) {
            p.push(seg);
        }
        p
    }

    pub fn trash(&self) -> PathBuf {
        self.dir.join(TRASH_DIR_NAME)
    }

    pub fn ensure_trash(&self) -> Result<()> {
        fs::create_dir_all(self.trash().join("b"))?;
        fs::create_dir_all(self.trash().join("f"))?;
        Ok(())
    }
}

pub fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

pub fn move_path(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    if src.is_dir() {
        if let Err(e) = copy_dir_all(src, dst) {
            let _ = fs::remove_dir_all(dst);
            return Err(e.into());
        }
        fs::remove_dir_all(src)?;
    } else {
        fs::copy(src, dst)?;
        fs::remove_file(src)?;
    }
    Ok(())
}

pub fn remove_dir_if_empty(path: &Path) {
    let _ = fs::remove_dir(path);
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp-shelfmark");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn find_format_file(dir: &Path, name: &str, fmt: &str) -> Option<PathBuf> {
    let ext = fmt.to_lowercase();
    let exact = dir.join(format!("{name}.{ext}"));
    if exact.is_file() {
        return Some(exact);
    }
    let suffix = format!(".{ext}");
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let fname = entry.file_name().to_string_lossy().to_lowercase();
        if fname.ends_with(&suffix) && !fname.starts_with("._") && entry.path().is_file() {
            return Some(entry.path());
        }
    }
    None
}

pub struct BookRow {
    pub title: String,
    pub path: String,
    pub first_author: String,
    pub formats: Vec<(String, String)>,
}

pub fn book_row(conn: &Connection, id: BookId) -> Result<BookRow> {
    let (title, path): (String, String) = conn
        .query_row("SELECT title, path FROM books WHERE id=?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?
        .ok_or(Error::BookNotFound(id))?;
    let first_author: Option<String> = conn
        .query_row(
            "SELECT a.name FROM books_authors_link l JOIN authors a ON a.id=l.author WHERE l.book=?1 ORDER BY l.id LIMIT 1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    let mut stmt = conn.prepare("SELECT format, name FROM data WHERE book=?1 ORDER BY id")?;
    let formats = stmt
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?.to_uppercase(),
                r.get::<_, String>(1)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(BookRow {
        title,
        path,
        first_author: first_author
            .map(|a| a.replace('|', ","))
            .unwrap_or_else(|| "Unknown".to_string()),
        formats,
    })
}

pub fn update_path(ctx: &Ctx, conn: &Connection, id: BookId) -> Result<()> {
    let row = book_row(conn, id)?;
    let path = text::construct_path_name(ctx.rules.limit, id, &row.title, &row.first_author);
    let extlen = row
        .formats
        .iter()
        .map(|f| f.0.len())
        .max()
        .map_or(10, |l| l + 1);
    let fname = text::construct_file_name(ctx.rules, &row.title, &row.first_author, extlen);

    let save = |conn: &Connection| -> Result<()> {
        conn.execute("UPDATE data SET name=?1 WHERE book=?2", params![fname, id])?;
        conn.execute("UPDATE books SET path=?1 WHERE id=?2", params![path, id])?;
        Ok(())
    };

    if row.path.is_empty() {
        return save(conn);
    }

    let spath = ctx.join(&row.path);
    let tpath = ctx.join(&path);

    let rename_formats = |base: &Path, from_names: &[(String, String)]| -> Result<()> {
        for (fmt, name) in from_names {
            if *name == fname {
                continue;
            }
            if let Some(cur) = find_format_file(base, name, fmt) {
                let new = base.join(format!("{fname}.{}", fmt.to_lowercase()));
                if cur != new {
                    fs::rename(&cur, &new)?;
                }
            }
        }
        Ok(())
    };

    if path == row.path {
        if row.formats.iter().any(|f| f.1 != fname) {
            rename_formats(&spath, &row.formats)?;
            save(conn)?;
        }
        return Ok(());
    }

    if spath.exists() && tpath.exists() && same_dir(&spath, &tpath) {
        rename_formats(&spath, &row.formats)?;
        let mut cur = ctx.dir.to_path_buf();
        for (old, new) in row.path.split('/').zip(path.split('/')) {
            if old != new && old.to_lowercase() == new.to_lowercase() {
                let _ = fs::rename(cur.join(old), cur.join(new));
            }
            cur.push(new);
        }
        return save(conn);
    }

    if spath.exists() {
        if tpath.exists() {
            fs::remove_dir_all(&tpath)?;
        }
        move_path(&spath, &tpath)?;
        if let Err(e) = rename_formats(&tpath, &row.formats) {
            let _ = move_path(&tpath, &spath);
            return Err(e);
        }
        if let Some(parent) = spath.parent() {
            remove_dir_if_empty(parent);
        }
    }
    save(conn)
}

pub fn add_format_file(
    ctx: &Ctx,
    conn: &Connection,
    id: BookId,
    src: &Path,
    fmt: &str,
) -> Result<()> {
    if !src.is_file() {
        return Err(Error::FileMissing(src.to_path_buf()));
    }
    let mut row = book_row(conn, id)?;
    if row.path.is_empty() {
        update_path(ctx, conn, id)?;
        row = book_row(conn, id)?;
    }
    let fmt = fmt.to_uppercase();
    let fname = text::construct_file_name(ctx.rules, &row.title, &row.first_author, fmt.len() + 1);
    let dir = ctx.join(&row.path);
    fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{fname}.{}", fmt.to_lowercase()));
    if let Some((_, old_name)) = row.formats.iter().find(|f| f.0 == fmt)
        && *old_name != fname
    {
        let old = dir.join(format!("{old_name}.{}", fmt.to_lowercase()));
        if old.is_file() && old != dest {
            let _ = fs::rename(&old, &dest);
        }
    }
    if !(dest.exists() && same_dir(src, &dest)) {
        let tmp = dest.with_extension("tmp-shelfmark");
        fs::copy(src, &tmp)?;
        fs::rename(&tmp, &dest)?;
    }
    let size = fs::metadata(&dest)?.len() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO data(book, format, uncompressed_size, name) VALUES (?1, ?2, ?3, ?4)",
        params![id, fmt, size, fname],
    )?;
    mark_page_rescan(conn, id);
    Ok(())
}

pub fn mark_page_rescan(conn: &Connection, id: BookId) {
    let _ = conn.execute(
        "UPDATE books_pages_link SET needs_scan=1 WHERE book=?1",
        [id],
    );
}

pub fn remove_format_file(ctx: &Ctx, conn: &Connection, id: BookId, fmt: &str) -> Result<()> {
    let row = book_row(conn, id)?;
    let fmt = fmt.to_uppercase();
    let Some((_, name)) = row.formats.iter().find(|f| f.0 == fmt) else {
        return Err(Error::FormatNotFound { id, format: fmt });
    };
    if !row.path.is_empty() {
        let dir = ctx.join(&row.path);
        if let Some(file) = find_format_file(&dir, name, &fmt) {
            ctx.ensure_trash()?;
            let dest = ctx.trash().join("f").join(id.to_string());
            fs::create_dir_all(&dest)?;
            let authors: Vec<String> = {
                let mut stmt =
                    conn.prepare("SELECT a.name FROM books_authors_link l JOIN authors a ON a.id=l.author WHERE l.book=?1 ORDER BY l.id")?;
                stmt.query_map([id], |r| r.get::<_, String>(0))?
                    .map(|n| n.map(|n| n.replace('|', ",")))
                    .collect::<rusqlite::Result<_>>()?
            };
            let meta = serde_json::json!({ "title": row.title, "authors": authors });
            atomic_write(&dest.join("metadata.json"), meta.to_string().as_bytes())?;
            move_path(&file, &dest.join(fmt.to_lowercase()))?;
        }
    }
    conn.execute(
        "DELETE FROM data WHERE book=?1 AND format=?2",
        params![id, fmt],
    )?;
    mark_page_rescan(conn, id);
    Ok(())
}

pub fn encode_cover(bytes: &[u8]) -> Result<Vec<u8>> {
    use image::ImageFormat;
    let fmt = image::guess_format(bytes)?;
    let img = image::load_from_memory(bytes)?;
    if fmt == ImageFormat::Jpeg {
        return Ok(bytes.to_vec());
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = u32::from(p[3]);
        let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
        rgb.put_pixel(x, y, image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode_image(&rgb)?;
    Ok(out)
}

pub fn write_cover(ctx: &Ctx, conn: &Connection, id: BookId, jpeg: &[u8]) -> Result<()> {
    let mut row = book_row(conn, id)?;
    if row.path.is_empty() {
        update_path(ctx, conn, id)?;
        row = book_row(conn, id)?;
    }
    let dir = ctx.join(&row.path);
    fs::create_dir_all(&dir)?;
    atomic_write(&dir.join(COVER_FILE_NAME), jpeg)?;
    conn.execute("UPDATE books SET has_cover=1 WHERE id=?1", [id])?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 32 && !matches!(c, '\n' | '\r' | '\t') => {}
            c => out.push(c),
        }
    }
    out
}

pub fn opf(rec: &Record) -> String {
    let mut x = String::new();
    x.push_str("<?xml version='1.0' encoding='utf-8'?>\n");
    x.push_str("<package xmlns=\"http://www.idpf.org/2007/opf\" unique-identifier=\"uuid_id\" version=\"2.0\">\n");
    x.push_str("  <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:opf=\"http://www.idpf.org/2007/opf\">\n");
    x.push_str(&format!(
        "    <dc:identifier opf:scheme=\"calibre\" id=\"calibre_id\">{}</dc:identifier>\n",
        rec.id
    ));
    x.push_str(&format!(
        "    <dc:identifier opf:scheme=\"uuid\" id=\"uuid_id\">{}</dc:identifier>\n",
        xml_escape(&rec.uuid)
    ));
    x.push_str(&format!(
        "    <dc:title>{}</dc:title>\n",
        xml_escape(&rec.title)
    ));
    for a in &rec.authors {
        let file_as = if rec.author_sort.is_empty() {
            String::new()
        } else {
            format!(" opf:file-as=\"{}\"", xml_escape(&rec.author_sort))
        };
        x.push_str(&format!(
            "    <dc:creator opf:role=\"aut\"{file_as}>{}</dc:creator>\n",
            xml_escape(a)
        ));
    }
    x.push_str(
        "    <dc:contributor opf:file-as=\"calibre\" opf:role=\"bkp\">shelfmark</dc:contributor>\n",
    );
    if let Some(p) = &rec.pubdate {
        x.push_str(&format!(
            "    <dc:date>{}</dc:date>\n",
            p.format("%Y-%m-%dT%H:%M:%S+00:00")
        ));
    }
    for l in &rec.languages {
        x.push_str(&format!(
            "    <dc:language>{}</dc:language>\n",
            xml_escape(l)
        ));
    }
    if let Some(c) = &rec.comments {
        x.push_str(&format!(
            "    <dc:description>{}</dc:description>\n",
            xml_escape(c)
        ));
    }
    if let Some(p) = &rec.publisher {
        x.push_str(&format!(
            "    <dc:publisher>{}</dc:publisher>\n",
            xml_escape(p)
        ));
    }
    for (k, v) in &rec.identifiers {
        x.push_str(&format!(
            "    <dc:identifier opf:scheme=\"{}\">{}</dc:identifier>\n",
            xml_escape(&k.to_uppercase()),
            xml_escape(v)
        ));
    }
    for t in &rec.tags {
        x.push_str(&format!("    <dc:subject>{}</dc:subject>\n", xml_escape(t)));
    }
    if let Some(s) = &rec.series {
        x.push_str(&format!(
            "    <meta name=\"calibre:series\" content=\"{}\"/>\n",
            xml_escape(s)
        ));
        x.push_str(&format!(
            "    <meta name=\"calibre:series_index\" content=\"{}\"/>\n",
            rec.series_index
        ));
    }
    x.push_str(&format!(
        "    <meta name=\"calibre:title_sort\" content=\"{}\"/>\n",
        xml_escape(&rec.sort)
    ));
    if rec.rating > 0 {
        x.push_str(&format!(
            "    <meta name=\"calibre:rating\" content=\"{}.0\"/>\n",
            rec.rating
        ));
    }
    if let Some(t) = &rec.timestamp {
        x.push_str(&format!(
            "    <meta name=\"calibre:timestamp\" content=\"{}\"/>\n",
            dates::iso(t)
        ));
    }
    x.push_str("  </metadata>\n  <guide>\n");
    if rec.has_cover {
        x.push_str("    <reference type=\"cover\" title=\"Cover\" href=\"cover.jpg\"/>\n");
    }
    x.push_str("  </guide>\n</package>\n");
    x
}

pub fn trash_book(ctx: &Ctx, rec: &Record) -> Result<Option<(PathBuf, PathBuf)>> {
    if rec.path.is_empty() {
        return Ok(None);
    }
    let src = ctx.join(&rec.path);
    if !src.is_dir() {
        return Ok(None);
    }
    ctx.ensure_trash()?;
    let dest = ctx.trash().join("b").join(rec.id.to_string());
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    move_path(&src, &dest)?;
    atomic_write(&dest.join("metadata.opf"), opf(rec).as_bytes())?;
    if let Some(parent) = src.parent() {
        remove_dir_if_empty(parent);
    }
    Ok(Some((src, dest)))
}
