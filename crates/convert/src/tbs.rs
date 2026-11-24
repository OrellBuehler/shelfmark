use crate::util::{encode_tbs, encode_trailing_data};

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Starts,
    Completes,
    Spans,
    Ends,
}

#[derive(Clone)]
struct Entry {
    index: usize,
    depth: usize,
    parent: Option<usize>,
    action: Action,
}

pub struct TocEntry {
    pub index: usize,
    pub offset: usize,
    pub length: usize,
    pub depth: usize,
    pub parent: Option<usize>,
}

type Strand = Vec<(usize, Vec<Entry>)>;

struct NegativeStrandIndex;

fn fill_entry(e: &TocEntry, start_offset: i64, text_record_length: i64) -> Entry {
    let length_offset = start_offset + e.length as i64;
    let action = if start_offset < 0 {
        if length_offset > text_record_length {
            Action::Spans
        } else {
            Action::Ends
        }
    } else if length_offset > text_record_length {
        Action::Starts
    } else {
        Action::Completes
    };
    Entry {
        index: e.index,
        depth: e.depth,
        parent: e.parent,
        action,
    }
}

fn take(entries: &mut Vec<Entry>, index: usize) {
    entries.retain(|e| e.index != index);
}

fn populate_strand(parent: Entry, entries: &mut Vec<Entry>) -> Vec<Entry> {
    let mut ans = vec![parent.clone()];
    let child = entries
        .iter()
        .find(|c| c.parent == Some(parent.index))
        .cloned();
    if let Some(child) = child {
        take(entries, child.index);
        ans.extend(populate_strand(child, entries));
    } else {
        let mut current = parent.index;
        let mut siblings = Vec::new();
        for entry in entries.clone() {
            if entry.depth == parent.depth
                && entry.parent == parent.parent
                && entry.index == current + 1
            {
                current += 1;
                take(entries, entry.index);
                if entries.iter().any(|c| c.parent == Some(entry.index)) {
                    siblings.extend(populate_strand(entry, entries));
                    break;
                }
                siblings.push(entry);
            }
        }
        ans.extend(siblings);
    }
    ans
}

fn separate_strands(mut entries: Vec<Entry>) -> Vec<Strand> {
    let mut ans = Vec::new();
    while !entries.is_empty() {
        let top = entries.remove(0);
        let strand = populate_strand(top, &mut entries);
        let mut layers: Strand = Vec::new();
        for e in strand {
            match layers.iter_mut().find(|(d, _)| *d == e.depth) {
                Some((_, v)) => v.push(e),
                None => layers.push((e.depth, vec![e])),
            }
        }
        ans.push(layers);
    }
    ans
}

fn collect_indexing_data(entries: &[TocEntry], record_lengths: &[usize]) -> Vec<Vec<Strand>> {
    let mut sorted: Vec<&TocEntry> = entries.iter().collect();
    sorted.sort_by_key(|e| e.offset);
    let mut data = Vec::new();
    let mut record_start = 0i64;
    for &len in record_lengths {
        let next_start = record_start + len as i64;
        let mut local = Vec::new();
        for e in &sorted {
            if e.offset as i64 >= next_start {
                break;
            }
            if (e.offset + e.length) as i64 <= record_start {
                continue;
            }
            local.push(fill_entry(e, e.offset as i64 - record_start, len as i64));
        }
        data.push(separate_strands(local));
        record_start += len as i64;
    }
    data
}

type Sequence = (i64, Vec<(u8, u64)>);

fn encode_strands(strands: &[Strand], tbs_type: u64) -> Result<Vec<Sequence>, NegativeStrandIndex> {
    let mut ans: Vec<Sequence> = Vec::new();
    let first_index = strands
        .first()
        .and_then(|s| s.first())
        .map(|(_, e)| e[0].index);
    let mut last_index = 0usize;
    for strand in strands {
        let mut seqs: Vec<Sequence> = Vec::new();
        for (_, entries) in strand {
            let mut extra: Vec<(u8, u64)> = Vec::new();
            if entries.last().unwrap().action == Action::Spans {
                extra.push((0b1, 0));
            }
            if Some(entries[0].index) == first_index {
                extra.push((0b10, tbs_type));
            }
            if entries.len() > 1 {
                extra.push((0b100, entries.len() as u64));
            }
            let mut index = entries[0].index as i64 - entries[0].parent.unwrap_or(0) as i64;
            if !ans.is_empty() && seqs.is_empty() {
                index = last_index as i64 - entries[0].index as i64;
                if index < 0 {
                    if tbs_type == 5 {
                        index = -index;
                    } else {
                        return Err(NegativeStrandIndex);
                    }
                } else {
                    extra.push((0b1000, 1));
                }
            }
            last_index = entries.last().unwrap().index;
            seqs.push((index, extra));
        }
        for i in 0..seqs.len().saturating_sub(1) {
            let next_has = seqs[i + 1].1.iter().any(|(f, _)| *f == 0b1);
            if next_has {
                seqs[i].1.retain(|(f, _)| *f != 0b1);
            }
        }
        ans.extend(seqs);
    }
    Ok(ans)
}

fn sequences_to_bytes(seqs: &[Sequence]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (val, extra)) in seqs.iter().enumerate() {
        out.extend(encode_tbs(*val as u64, extra, if i == 0 { 3 } else { 4 }));
    }
    out
}

fn calculate_all(data: &[Vec<Strand>], tbs_type: u64) -> Result<Vec<Vec<u8>>, NegativeStrandIndex> {
    data.iter()
        .map(|strands| encode_strands(strands, tbs_type).map(|s| sequences_to_bytes(&s)))
        .collect()
}

pub fn apply_trailing_byte_sequences(
    entries: &[TocEntry],
    records: &mut [Vec<u8>],
    record_lengths: &[usize],
) {
    let data = collect_indexing_data(entries, record_lengths);
    let tbs = calculate_all(&data, 8)
        .or_else(|_| calculate_all(&data, 5))
        .unwrap_or_else(|_| vec![Vec::new(); record_lengths.len()]);
    for (i, bytes) in tbs.iter().enumerate() {
        records[i].extend(encode_trailing_data(bytes));
    }
}
