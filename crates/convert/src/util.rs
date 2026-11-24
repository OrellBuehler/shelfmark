pub const RECORD_SIZE: usize = 0x1000;

pub fn encint(mut value: u64, forward: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        bytes.push((value & 0x7f) as u8);
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    let first = if forward { 0 } else { bytes.len() - 1 };
    bytes[first] |= 0x80;
    bytes.reverse();
    bytes
}

pub fn to_base(mut num: u64, base: u64, min_digits: usize) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut out = Vec::new();
    while num > 0 {
        out.push(DIGITS[(num % base) as usize]);
        num /= base;
    }
    while out.len() < min_digits.max(1) {
        out.push(b'0');
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

pub fn align_block(mut raw: Vec<u8>) -> Vec<u8> {
    let extra = raw.len() % 4;
    if extra != 0 {
        raw.resize(raw.len() + 4 - extra, 0);
    }
    raw
}

pub fn encode_trailing_data(raw: &[u8]) -> Vec<u8> {
    let mut lsize = 1;
    let encoded = loop {
        let encoded = encint((raw.len() + lsize) as u64, false);
        if encoded.len() == lsize {
            break encoded;
        }
        lsize += 1;
    };
    let mut out = raw.to_vec();
    out.extend(encoded);
    out
}

pub fn encode_tbs(val: u64, extra: &[(u8, u64)], flag_size: u32) -> Vec<u8> {
    let flags = extra.iter().fold(0u64, |acc, (flag, _)| acc | *flag as u64);
    let mut ans = val << flag_size;
    for i in 0..flag_size {
        ans |= flags & (1 << i);
    }
    let mut out = encint(ans, true);
    let get = |flag: u8| extra.iter().find(|(f, _)| *f == flag).map(|(_, v)| *v);
    if let Some(v) = get(0b10) {
        out.extend(encint(v, true));
    }
    if let Some(v) = get(0b100) {
        out.push(v as u8);
    }
    if let Some(v) = get(0b1) {
        out.extend(encint(v, true));
    }
    out
}

pub fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}

pub fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

pub fn has_scheme(href: &str) -> bool {
    match href.find(':') {
        Some(i) if i > 0 => {
            let scheme = &href[..i];
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        }
        _ => false,
    }
}

pub fn dir_of(path: &str) -> &str {
    path.rfind('/').map(|i| &path[..i]).unwrap_or("")
}

pub fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub fn resolve_href(base_doc: &str, href: &str) -> Option<(String, String)> {
    if has_scheme(href) {
        return None;
    }
    let (path, frag) = match href.split_once('#') {
        Some((p, f)) => (p, f),
        None => (href, ""),
    };
    let path = path.split_once('?').map(|(p, _)| p).unwrap_or(path);
    let path = percent_decode(path);
    let frag = percent_decode(frag);
    let full = if path.is_empty() {
        base_doc.to_string()
    } else if let Some(abs) = path.strip_prefix('/') {
        normalize_path(abs)
    } else {
        normalize_path(&format!("{}/{}", dir_of(base_doc), path))
    };
    Some((full, frag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encint_matches_calibre() {
        assert_eq!(encint(0x11111, true), [0x04, 0x22, 0x91]);
        assert_eq!(encint(0x11111, false), [0x84, 0x22, 0x11]);
        assert_eq!(encint(0, true), [0x80]);
        assert_eq!(encint(127, true), [0xff]);
        assert_eq!(encint(128, true), [0x01, 0x80]);
        assert_eq!(encint(128, false), [0x81, 0x00]);
    }

    #[test]
    fn base32() {
        assert_eq!(to_base(0, 32, 0), "0");
        assert_eq!(to_base(0, 32, 4), "0000");
        assert_eq!(to_base(1_000_000, 32, 4), "UGI0");
        assert_eq!(to_base(12345, 32, 4), "0C1P");
        assert_eq!(to_base(31, 32, 4), "000V");
        assert_eq!(to_base(1023, 32, 4), "00VV");
        assert_eq!(to_base(1024, 32, 4), "0100");
        assert_eq!(to_base(5, 32, 10), "0000000005");
    }

    #[test]
    fn trailing_data() {
        assert_eq!(encode_trailing_data(&[]), [0x81]);
        assert_eq!(encode_trailing_data(&[1, 2, 3]), [1, 2, 3, 0x84]);
        let enc = encode_trailing_data(&[0; 200]);
        assert_eq!(&enc[200..], [0x81, 0x4a]);
        let enc = encode_trailing_data(&vec![0; 16400]);
        assert_eq!(&enc[16400..], [0x81, 0x00, 0x13]);
    }

    #[test]
    fn tbs_encoding() {
        assert_eq!(encode_tbs(0, &[(0b10, 8)], 3), [0x82, 0x88]);
        assert_eq!(
            encode_tbs(3, &[(0b100, 4), (0b1, 0)], 4),
            [0xb5, 0x04, 0x80]
        );
        assert_eq!(encode_tbs(5, &[(0b1000, 1)], 4), [0xd8]);
        assert_eq!(
            encode_tbs(300, &[(0b10, 3), (0b1, 70000)], 4),
            [0x25, 0xc3, 0x83, 0x04, 0x22, 0xf0]
        );
    }
}
