use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

pub fn is_font(data: &[u8]) -> bool {
    matches!(
        data.get(..4),
        Some(b"\0\x01\0\0" | b"true" | b"ttcf" | b"OTTO")
    )
}

fn key_for(data: &[u8]) -> [u8; 20] {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data.iter().step_by(7) {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    let mut key = [0u8; 20];
    for k in key.iter_mut() {
        h = h
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *k = (h >> 33) as u8;
    }
    key
}

pub fn font_record(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(data).unwrap();
    let mut packed = enc.finish().unwrap();
    let mut flags = 0b1u32;
    let mut xor_key: Vec<u8> = Vec::new();
    if packed.len() >= 1040 {
        flags |= 0b10;
        let key = key_for(data);
        for (i, b) in packed.iter_mut().take(1040).enumerate() {
            *b ^= key[i % key.len()];
        }
        xor_key = key.to_vec();
    }
    let key_start = 24u32;
    let data_start = key_start + xor_key.len() as u32;
    let mut out = b"FONT".to_vec();
    for v in [
        data.len() as u32,
        flags,
        data_start,
        xor_key.len() as u32,
        key_start,
    ] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend(xor_key);
    out.extend(packed);
    out
}
