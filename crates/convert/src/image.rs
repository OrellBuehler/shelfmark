use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader};

pub const PLACEHOLDER_GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\xf0\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

const MAX_SIDE: u32 = 4000;
const MAX_PIXELS: u64 = 12_000_000;
const MAX_BYTES: usize = 10 * 1024 * 1024;
const MAX_DECODE_PIXELS: u64 = 120_000_000;
const FIT_SIDE: u32 = 3000;
const THUMB_SIZE: (u32, u32) = (180, 240);
const THUMB_MAX_BYTES: usize = 16 * 1024;
const QUALITY: u8 = 90;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Jpeg,
    Png,
    Gif,
    Webp,
    Other,
}

pub fn sniff(data: &[u8]) -> Kind {
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Kind::Jpeg
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Kind::Png
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Kind::Gif
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Kind::Webp
    } else {
        Kind::Other
    }
}

pub fn mime_for(kind: Kind) -> &'static str {
    match kind {
        Kind::Jpeg => "image/jpeg",
        Kind::Png => "image/png",
        Kind::Gif => "image/gif",
        _ => "image/jpeg",
    }
}

struct JpegInfo {
    jfif: bool,
    exif: bool,
    components: u8,
}

fn jpeg_info(data: &[u8]) -> JpegInfo {
    let mut info = JpegInfo {
        jfif: false,
        exif: false,
        components: 3,
    };
    let mut i = 2;
    while i + 4 <= data.len() {
        if data[i] != 0xff {
            break;
        }
        let marker = data[i + 1];
        if marker == 0xff {
            i += 1;
            continue;
        }
        if marker == 0xd8 || (0xd0..=0xd7).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        let body = data.get(i + 4..i + 2 + len).unwrap_or(&[]);
        match marker {
            0xe0 if body.starts_with(b"JFIF\0") => info.jfif = true,
            0xe1 if body.starts_with(b"Exif\0") => info.exif = true,
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                info.components = body.get(5).copied().unwrap_or(3);
                break;
            }
            0xda => break,
            _ => {}
        }
        i += 2 + len;
    }
    info
}

fn decode(data: &[u8]) -> Option<DynamicImage> {
    let (w, h) = dimensions(data)?;
    if w as u64 * h as u64 > MAX_DECODE_PIXELS {
        return None;
    }
    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Some(img)
}

fn dimensions(data: &[u8]) -> Option<(u32, u32)> {
    ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

fn flatten(img: DynamicImage) -> DynamicImage {
    if !img.color().has_alpha() {
        return img;
    }
    let rgba = img.to_rgba8();
    let mut out = image::RgbImage::new(rgba.width(), rgba.height());
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
        out.put_pixel(x, y, image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
    }
    DynamicImage::ImageRgb8(out)
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let enc = JpegEncoder::new_with_quality(&mut out, quality);
    let img = match img {
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_) => img.clone(),
        other if other.color().channel_count() <= 2 => DynamicImage::ImageLuma8(other.to_luma8()),
        other => DynamicImage::ImageRgb8(other.to_rgb8()),
    };
    img.write_with_encoder(enc).ok()?;
    Some(out)
}

fn encode_png(img: &DynamicImage) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

fn fit(img: DynamicImage, max_side: u32) -> DynamicImage {
    if img.width().max(img.height()) <= max_side {
        img
    } else {
        img.resize(max_side, max_side, FilterType::Lanczos3)
    }
}

fn oversized(data: &[u8]) -> bool {
    match dimensions(data) {
        Some((w, h)) => {
            w.max(h) > MAX_SIDE || w as u64 * h as u64 > MAX_PIXELS || data.len() > MAX_BYTES
        }
        None => false,
    }
}

fn reencode(img: DynamicImage) -> Option<(Vec<u8>, Kind)> {
    if img.color().has_alpha() {
        let has_transparency = img.to_rgba8().pixels().any(|p| p[3] < 255);
        if has_transparency {
            return encode_png(&img).map(|d| (d, Kind::Png));
        }
    }
    encode_jpeg(&flatten(img), QUALITY).map(|d| (d, Kind::Jpeg))
}

pub fn process(data: Vec<u8>) -> Option<(Vec<u8>, Kind)> {
    let kind = sniff(&data);
    match kind {
        Kind::Jpeg => {
            let info = jpeg_info(&data);
            if oversized(&data) {
                let img = fit(decode(&data)?, FIT_SIDE);
                return encode_jpeg(&flatten(img), QUALITY).map(|d| (d, Kind::Jpeg));
            }
            if !info.jfif || info.exif || info.components == 4 {
                let img = decode(&data)?;
                return encode_jpeg(&flatten(img), QUALITY).map(|d| (d, Kind::Jpeg));
            }
            Some((data, Kind::Jpeg))
        }
        Kind::Png | Kind::Gif => {
            if oversized(&data) {
                return reencode(fit(decode(&data)?, FIT_SIDE));
            }
            Some((data, kind))
        }
        Kind::Webp => reencode(fit(decode(&data)?, FIT_SIDE)),
        Kind::Other => None,
    }
}

pub fn process_cover(data: &[u8]) -> Option<Vec<u8>> {
    if sniff(data) == Kind::Jpeg {
        let info = jpeg_info(data);
        if info.jfif && !info.exif && info.components != 4 && !oversized(data) {
            return Some(data.to_vec());
        }
    }
    let img = fit(flatten(decode(data)?), FIT_SIDE);
    encode_jpeg(&img, QUALITY)
}

pub fn thumbnail(cover: &[u8]) -> Option<Vec<u8>> {
    let img = flatten(decode(cover)?);
    let img = if img.width() > THUMB_SIZE.0 || img.height() > THUMB_SIZE.1 {
        img.resize(THUMB_SIZE.0, THUMB_SIZE.1, FilterType::Lanczos3)
    } else {
        img
    };
    let mut quality = QUALITY;
    loop {
        let data = encode_jpeg(&img, quality)?;
        if data.len() <= THUMB_MAX_BYTES || quality <= 5 {
            return Some(data);
        }
        quality -= 5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn test_png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let img = if alpha {
            DynamicImage::ImageRgba8(image::RgbaImage::from_fn(w, h, |x, y| {
                image::Rgba([
                    (x % 255) as u8,
                    (y % 255) as u8,
                    90,
                    if x < 2 { 0 } else { 255 },
                ])
            }))
        } else {
            DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
                image::Rgb([(x % 255) as u8, (y % 255) as u8, 90])
            }))
        };
        encode_png(&img).unwrap()
    }

    pub fn test_jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 90])
        }));
        encode_jpeg(&img, 80).unwrap()
    }

    #[test]
    fn placeholder_is_a_valid_gif() {
        let img = image::load_from_memory(PLACEHOLDER_GIF).unwrap();
        assert_eq!((img.width(), img.height()), (1, 1));
    }

    #[test]
    fn jpeg_passes_through() {
        let jpeg = test_jpeg(64, 48);
        assert!(jpeg_info(&jpeg).jfif);
        let (out, kind) = process(jpeg.clone()).unwrap();
        assert_eq!(kind, Kind::Jpeg);
        assert_eq!(out, jpeg);
    }

    #[test]
    fn png_passes_through_and_big_images_shrink() {
        let png = test_png(40, 30, true);
        let (out, kind) = process(png.clone()).unwrap();
        assert_eq!((kind, out), (Kind::Png, png));
        let big = test_png(4200, 100, false);
        let (out, kind) = process(big).unwrap();
        assert_eq!(kind, Kind::Jpeg);
        assert_eq!(image::load_from_memory(&out).unwrap().width(), FIT_SIDE);
    }

    #[test]
    fn cover_becomes_jpeg_and_thumbnail_is_small() {
        let cover = process_cover(&test_png(600, 900, true)).unwrap();
        assert_eq!(sniff(&cover), Kind::Jpeg);
        let thumb = thumbnail(&cover).unwrap();
        assert!(thumb.len() <= THUMB_MAX_BYTES);
        let t = image::load_from_memory(&thumb).unwrap();
        assert!(t.width() <= 180 && t.height() <= 240);
    }

    #[test]
    fn unknown_data_is_dropped() {
        assert!(process(b"not an image".to_vec()).is_none());
    }
}
