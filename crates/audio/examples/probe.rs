use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use shelfmark_audio::{AudioStore, LoftyReader, NewRoot, scan};

fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("usage: probe <audiobook folder>");
        std::process::exit(2);
    };
    let scratch = std::env::temp_dir().join(format!("shelfmark-probe-{}", std::process::id()));
    let store = AudioStore::open(&scratch).expect("open scratch index");
    let id = store
        .add_root(&NewRoot {
            path: Path::new(&root).to_string_lossy().into_owned(),
            ..Default::default()
        })
        .expect("add root")
        .id;
    for pass in ["cold", "warm"] {
        let started = Instant::now();
        let summary = scan(&store, id, &LoftyReader, &AtomicBool::new(false), &|_| {});
        println!("{pass} scan: {:?} in {:.1?}", summary, started.elapsed());
    }
    let rows = store.list().expect("list");
    let with_asin = rows.iter().filter(|r| r.asin.is_some()).count();
    let with_cover = rows.iter().filter(|r| r.has_cover).count();
    println!(
        "{} audiobooks, {with_asin} with ASIN, {with_cover} with cover",
        rows.len()
    );
    for r in rows.iter() {
        println!(
            "{:<10} {:>6.1}h {:>3} files  {} — {} | read by {}",
            r.asin.as_deref().unwrap_or("-"),
            r.duration_ms as f64 / 3_600_000.0,
            r.file_count,
            r.title,
            r.authors.join(", "),
            r.narrators.join(", ")
        );
    }
    for issue in store.issues().expect("issues") {
        println!("ISSUE {}: {}", issue.path, issue.message);
    }
    drop(store);
    let _ = std::fs::remove_dir_all(&scratch);
}
