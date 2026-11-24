use std::collections::HashMap;

use crate::dom;
use crate::index::{Cncx, IndexEntry, NCX_TAGS, build_index};
use crate::markup::{self, Ctx, Doc};
use crate::skeleton;
use crate::tbs::{self, TocEntry};

const PALMDOC_OUT: &str = "3c70e36c6173733d2278223e48656c6c6ff76f726c6420302cf46865f17569636be2726f776ee66f78ea756d7073ef76657280fa6c617a79e46f673c2f703e0a82678267826231826782678267826782678267826782623282678267826782678267826782678262338267826782678267826782678267826234826782678267826782678267826782623582678267826782678267826782678262368267826782678267826782678267826237826782678267826782678267826782623882678267826782678267826782678262398267826782678267826782678267956397cf826f826f826f826f826f826f826c97d7826f826f826f826f0a00050102c8c908096802c3a9824202c3b6824908e697a5e69cace8aa019ee880c780c780c780c706e69cace8aa9e20";

const TBS_RECORDS: [&str; 5] = [
    "8288b084",
    "838880b40286",
    "8288c0bc0286",
    "938880b085",
    "9288b084",
];

const NCX_RECORDS: [&str; 3] = [
    "494e4458000000c0000000000000000000000002000000f8000000010000fde9ffffffff000000060000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c0000000000000000054414758000000300000000101010100020102000301040004010800150110001601200017014000060280000000000102303500060000004944585400f00000",
    "494e4458000000c00000000000000001000000000000011000000006ffffffffffffffff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000023030ef8046a88080838480800230318f46a817b882808187023032ef5de02ef084808585828e0230339fe4278886818083950230349f27ec1ebc898180849c0230359f65c827888c818285a30000004944585400c000cd00d900e700f30100",
    "81418142814382413182413282433100",
];

const CHUNK_TEXT_LEN: usize = 77004;
const CHUNK_TEXT_FNV: u64 = 18297996174062437624;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, &x| {
        (h ^ x as u64).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn palmdoc_matches_calibre() {
    let mut data: Vec<u8> = (0..12)
        .map(|i| {
            format!(
                "<p class=\"x\">Hello world {i}, the quick brown fox jumps over the lazy dog</p>\n"
            )
        })
        .collect::<String>()
        .into_bytes();
    data.extend_from_slice(&[0, 1, 2, 200, 201, 8, 9]);
    data.extend_from_slice(
        "h\u{e9}llo w\u{f6}rld \u{65e5}\u{672c}\u{8a9e} "
            .repeat(3)
            .as_bytes(),
    );
    assert_eq!(data.len(), 993);
    assert_eq!(hex(&crate::palmdoc::compress(&data)), PALMDOC_OUT);
}

type TocRow = (
    &'static str,
    usize,
    Option<usize>,
    Option<usize>,
    Option<usize>,
    usize,
    usize,
    usize,
    usize,
);

const TOC: [TocRow; 6] = [
    ("A", 0, None, Some(3), Some(4), 0, 9000, 0, 0),
    ("B", 0, None, None, None, 9000, 3000, 1, 7),
    ("C", 0, None, Some(5), Some(5), 12000, 6000, 2, 14),
    ("A1", 1, Some(0), None, None, 100, 5000, 3, 21),
    ("A2", 1, Some(0), None, None, 5100, 3900, 4, 28),
    ("C1", 1, Some(2), None, None, 13000, 5000, 5, 35),
];

#[test]
fn trailing_byte_sequences_match_calibre() {
    let entries: Vec<TocEntry> = TOC
        .iter()
        .enumerate()
        .map(|(index, t)| TocEntry {
            index,
            offset: t.5,
            length: t.6,
            depth: t.1,
            parent: t.2,
        })
        .collect();
    let mut records = vec![Vec::new(); 5];
    tbs::apply_trailing_byte_sequences(&entries, &mut records, &[4096, 4096, 4096, 4096, 1700]);
    let got: Vec<String> = records.iter().map(|r| hex(r)).collect();
    assert_eq!(got, TBS_RECORDS);
}

#[test]
fn ncx_index_matches_calibre() {
    let cncx = Cncx::new(TOC.iter().map(|t| t.0));
    let entries: Vec<IndexEntry> = TOC
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut tags: Vec<(&'static str, Vec<u64>)> = vec![
                ("offset", vec![t.5 as u64]),
                ("length", vec![t.6 as u64]),
                ("label", vec![cncx.get(t.0)]),
                ("depth", vec![t.1 as u64]),
            ];
            if let Some(p) = t.2 {
                tags.push(("parent", vec![p as u64]));
            }
            if let (Some(a), Some(b)) = (t.3, t.4) {
                tags.push(("first_child", vec![a as u64]));
                tags.push(("last_child", vec![b as u64]));
            }
            tags.push(("pos_fid", vec![t.7 as u64, t.8 as u64]));
            IndexEntry {
                lead: format!("{i:02X}").into_bytes(),
                tags,
            }
        })
        .collect();
    let records = build_index(NCX_TAGS, &entries, &cncx).unwrap();
    let got: Vec<String> = records.iter().map(|r| hex(r)).collect();
    assert_eq!(got, NCX_RECORDS);
}

fn reference_doc(i: usize) -> String {
    let mut body = format!("<div id=\"d{i}\">");
    for k in 0..60 {
        body += &format!("<p>Para {i}-{k} {}</p>\n", "word ".repeat(40));
    }
    body += &format!("<section id=\"s{i}\"><h2>Sec</h2>");
    for k in 0..30 {
        body += &format!(
            "<p id=\"q{i}_{k}\">Quote {k} {}</p>",
            "\u{fc}n\u{ef} ".repeat(30)
        );
    }
    body += "</section></div><p>";
    body += &"\u{e9}".repeat(9000);
    body += "</p><p>tail</p>";
    format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"en\">\n<head><title>T{i}</title></head>\n<body>{body}</body>\n</html>"
    )
}

#[test]
fn chunker_matches_calibre() {
    let mut ctx = Ctx::default();
    let mut docs: Vec<Doc> = (0..2)
        .map(|i| Doc {
            href: format!("d{i}.xhtml"),
            root: markup::prepare(dom::parse(reference_doc(i).as_bytes())),
        })
        .collect();
    markup::insert_aid_attributes(&mut ctx, &mut docs);
    let mut roots: Vec<_> = docs.into_iter().map(|d| d.root).collect();
    let chunked = skeleton::chunk(&mut roots, &HashMap::new()).unwrap();
    let skel: Vec<(usize, usize, usize, usize)> = chunked
        .skel_table
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.chunk_count, s.start_pos, s.length))
        .collect();
    assert_eq!(skel, [(0, 7, 0, 197), (1, 7, 38390, 205)]);
    let chunks: Vec<(usize, &str, usize, usize, usize, usize)> = chunked
        .chunk_table
        .iter()
        .map(|c| {
            (
                c.insert_pos,
                c.selector.as_str(),
                c.file_number,
                c.sequence_number,
                c.start_pos,
                c.length,
            )
        })
        .collect();
    let expected: Vec<(usize, &str, usize, usize, usize, usize)> = vec![
        (159, "P-//*[@aid='1']", 0, 0, 0, 8132),
        (8291, "P-//*[@aid='1']", 0, 1, 8132, 5448),
        (13739, "P-//*[@aid='1']", 0, 2, 13580, 6517),
        (20274, "P-//*[@aid='2U']", 0, 3, 20097, 8192),
        (28466, "P-//*[@aid='2U']", 0, 4, 28289, 8230),
        (36696, "P-//*[@aid='2U']", 0, 5, 36519, 1654),
        (38354, "S-//*[@aid='2U']", 0, 6, 38173, 20),
        (38555, "P-//*[@aid='UGI1']", 1, 7, 0, 8005),
        (46560, "P-//*[@aid='UGI1']", 1, 8, 8005, 5725),
        (52285, "P-//*[@aid='UGI1']", 1, 9, 13730, 6581),
        (58886, "P-//*[@aid='UGKU']", 1, 10, 20311, 8192),
        (67078, "P-//*[@aid='UGKU']", 1, 11, 28503, 8230),
        (75308, "P-//*[@aid='UGKU']", 1, 12, 36733, 1654),
        (76966, "S-//*[@aid='UGKU']", 1, 13, 38387, 22),
    ];
    assert_eq!(chunks, expected);
    assert_eq!(chunked.text.len(), CHUNK_TEXT_LEN);
    assert_eq!(fnv(&chunked.text), CHUNK_TEXT_FNV);
    for (aid, (seq, off, abs)) in [
        ("0", (0, 0, 124)),
        ("1", (0, 0, 138)),
        ("3", (0, 225, 384)),
        ("UGI0", (7, 0, 38514)),
        ("UGI5", (7, 684, 39239)),
    ] {
        assert_eq!(chunked.aid_offsets[aid], (seq, off, abs), "aid {aid}");
    }
}
