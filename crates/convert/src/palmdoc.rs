const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 10;
const MAX_DIST: usize = 2047;
const HASH_BITS: u32 = 16;

fn hash(p: &[u8]) -> usize {
    let v = (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32;
    (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    let len = data.len();
    let mut out = Vec::with_capacity(len);
    let mut head = vec![-1i32; 1 << HASH_BITS];
    let mut prev = vec![-1i32; len];
    let mut indexed = 0;
    let mut i = 0;
    while i < len {
        let c = data[i];
        if i > 10 && len - i > 10 {
            while indexed < i {
                let h = hash(&data[indexed..]);
                prev[indexed] = head[h];
                head[h] = indexed as i32;
                indexed += 1;
            }
            let cur = &data[i..];
            let mut best = MIN_MATCH - 1;
            let mut dist = 0;
            let mut p = head[hash(cur)];
            while p >= 0 {
                let d = i - p as usize;
                if d > MAX_DIST {
                    break;
                }
                let cand = &data[p as usize..];
                let mut l = 0;
                while l < MAX_MATCH && cand[l] == cur[l] {
                    l += 1;
                }
                l = l.min(d);
                if l > best {
                    best = l;
                    dist = d;
                    if best == MAX_MATCH {
                        break;
                    }
                }
                p = prev[p as usize];
            }
            if best >= MIN_MATCH {
                let compound = (dist << 3) + best - 3;
                out.push((0x80 + (compound >> 8)) as u8);
                out.push((compound & 0xff) as u8);
                i += best;
                continue;
            }
        }
        i += 1;
        if c == 32 && i < len {
            let n = data[i];
            if (0x40..=0x7f).contains(&n) {
                out.push(n ^ 0x80);
                i += 1;
                continue;
            }
        }
        if c == 0 || (c > 8 && c < 0x80) {
            out.push(c);
        } else {
            let mut temp = vec![c];
            let mut j = i;
            while j < len && temp.len() < 8 {
                let c = data[j];
                if c == 0 || (c > 8 && c < 0x80) {
                    break;
                }
                temp.push(c);
                j += 1;
            }
            i += temp.len() - 1;
            out.push(temp.len() as u8);
            out.extend(temp);
        }
    }
    out
}

#[cfg(test)]
pub fn decompress(input: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let c = input[i];
        i += 1;
        match c {
            1..=8 => {
                let n = (c as usize).min(input.len() - i);
                out.extend_from_slice(&input[i..i + n]);
                i += n;
            }
            0 | 9..=0x7f => out.push(c),
            0xc0..=0xff => {
                out.push(b' ');
                out.push(c ^ 0x80);
            }
            _ => {
                if i < input.len() {
                    let v = ((c as usize) << 8) + input[i] as usize;
                    i += 1;
                    let dist = (v & 0x3fff) >> 3;
                    for _ in 0..(v & 7) + 3 {
                        let b = out[out.len() - dist];
                        out.push(b);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_text_and_binary() {
        let mut data = Vec::new();
        for i in 0..400 {
            data.extend_from_slice(
                format!("<p class=\"x\">Hello world {i} and the quick brown fox</p> ").as_bytes(),
            );
        }
        data.extend_from_slice(&[0, 1, 2, 3, 200, 201, 250, 8, 9, 10, 255, 254]);
        data.extend_from_slice("héllo wörld \u{4e2d}\u{6587} ".repeat(30).as_bytes());
        let packed = compress(&data);
        assert!(packed.len() < data.len());
        assert_eq!(decompress(&packed), data);
    }

    #[test]
    fn short_inputs() {
        for data in [
            &b""[..],
            b"a",
            b" a",
            b"abc abc abc abc abc abc",
            &[0xff; 20],
        ] {
            assert_eq!(decompress(&compress(data)), data);
        }
    }
}
