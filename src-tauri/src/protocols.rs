use std::borrow::Cow;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Cursor;
use std::path::Path;
use std::time::UNIX_EPOCH;

use tauri::http::{Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::state::AppState;

type Body = Cow<'static, [u8]>;

fn respond(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "private, max-age=31536000, immutable")
        .body(Cow::Owned(body))
        .unwrap()
}

fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Borrowed(&[][..]))
        .unwrap()
}

fn decoded_path(request: &Request<Vec<u8>>) -> String {
    let raw = request.uri().path().trim_start_matches('/');
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && raw.is_char_boundary(i + 3)
            && let Ok(b) = u8::from_str_radix(&raw[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_width(request: &Request<Vec<u8>>) -> u32 {
    request
        .uri()
        .query()
        .and_then(|q| {
            q.split('&')
                .find_map(|kv| kv.strip_prefix("w=").and_then(|w| w.parse::<u32>().ok()))
        })
        .unwrap_or(400)
        .clamp(32, 2000)
}

fn thumbnail(state: &AppState, cover: &Path, width: u32) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(cover).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = DefaultHasher::new();
    (cover, mtime, meta.len(), width).hash(&mut hasher);
    let cached = state
        .cache_dir
        .join(format!("{:016x}.jpg", hasher.finish()));
    if let Ok(bytes) = std::fs::read(&cached) {
        return Some(bytes);
    }
    let original = std::fs::read(cover).ok()?;
    let Ok(img) = image::load_from_memory(&original) else {
        return Some(original);
    };
    if img.width() <= width {
        return Some(original);
    }
    let height = (img.height() as u64 * width as u64 / img.width() as u64).max(1) as u32;
    let thumb = img
        .resize_exact(width, height, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(Cursor::new(&mut out), 85)
        .encode_image(&thumb)
        .ok()?;
    let _ = std::fs::create_dir_all(&state.cache_dir);
    let _ = std::fs::write(&cached, &out);
    Some(out)
}

pub fn cover<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let width = query_width(&request);
        let response = decoded_path(&request)
            .parse::<i64>()
            .ok()
            .and_then(|id| state.current().ok()?.library.cover_path(id).ok().flatten())
            .and_then(|path| thumbnail(&state, &path, width))
            .map(|bytes| respond(StatusCode::OK, "image/jpeg", bytes))
            .unwrap_or_else(not_found);
        responder.respond(response);
    });
}

fn mime(format: &str) -> &'static str {
    match format.to_ascii_uppercase().as_str() {
        "EPUB" | "KEPUB" => "application/epub+zip",
        "PDF" => "application/pdf",
        "MOBI" | "AZW" | "AZW3" => "application/x-mobipocket-ebook",
        "CBZ" => "application/vnd.comicbook+zip",
        "FB2" => "application/x-fictionbook+xml",
        _ => "application/octet-stream",
    }
}

pub fn bookfile<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let path = decoded_path(&request);
        let response = path
            .split_once('/')
            .and_then(|(id, format)| {
                let id = id.parse::<i64>().ok()?;
                let file = state.current().ok()?.library.format_path(id, format).ok()?;
                let bytes = std::fs::read(file).ok()?;
                let mut response = respond(StatusCode::OK, mime(format), bytes);
                response
                    .headers_mut()
                    .insert("Cache-Control", "no-store".parse().unwrap());
                Some(response)
            })
            .unwrap_or_else(not_found);
        responder.respond(response);
    });
}
