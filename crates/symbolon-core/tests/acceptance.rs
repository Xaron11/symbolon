//! M1 acceptance at scale: a 100k-file tree is scanned into the catalog, and a rescan reads
//! nothing. Ignored by default because it writes 100k files; `just test-acceptance` runs it.

#![cfg(feature = "local")]

use std::fs;
use std::time::Instant;

use symbolon_core::catalog::{Catalog, HashPolicy, NewLocation};
use symbolon_core::query::{Page, Query};
use symbolon_core::scan::{ScanOptions, ScanStatus, Scanner};

const DIRS: u64 = 1_000;
const FILES_PER_DIR: u64 = 100;

#[test]
#[ignore = "writes 100k files; run with `just test-acceptance`"]
fn scan_and_rescan_a_100k_file_tree() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let started = Instant::now();
    for d in 0..DIRS {
        let sub = root.join(format!("d{d:04}"));
        fs::create_dir_all(&sub).unwrap();
        for f in 0..FILES_PER_DIR {
            // Every 10th file repeats an earlier one's content, so there are duplicates.
            let n = if f % 10 == 9 {
                d * FILES_PER_DIR
            } else {
                d * FILES_PER_DIR + f
            };
            fs::write(
                sub.join(format!("f{f:03}.txt")),
                format!("content of file {n}\n"),
            )
            .unwrap();
        }
    }
    let generated = started.elapsed();

    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    catalog
        .add_location(NewLocation::local("big", &root).hash_policy(HashPolicy::Eager))
        .unwrap();

    let started = Instant::now();
    let first = Scanner::start(&catalog, "big", ScanOptions::default())
        .unwrap()
        .wait()
        .unwrap();
    let first_time = started.elapsed();
    let files = DIRS * FILES_PER_DIR;
    assert_eq!(first.status, ScanStatus::Completed);
    assert_eq!((first.counts.files, first.counts.dirs), (files, DIRS));
    assert_eq!(
        (first.counts.hashed_files, first.counts.warnings),
        (files, 0)
    );

    let query = Query::open(&catalog).unwrap();
    let stats = query.location_stats("big").unwrap();
    assert_eq!((stats.files, stats.verified_files), (files, files));
    let dupes = query.duplicates("big", &Page::first(10)).unwrap();
    assert_eq!(dupes.items.len(), 10);
    assert!(dupes.items.iter().all(|group| group.entries.len() == 11));

    let started = Instant::now();
    let second = Scanner::start(&catalog, "big", ScanOptions::default())
        .unwrap()
        .wait()
        .unwrap();
    let second_time = started.elapsed();
    assert_eq!(
        (second.counts.hashed_files, second.counts.reused_hashes),
        (0, files)
    );

    eprintln!(
        "100k files: generated in {generated:.1?}, first scan {first_time:.1?}, rescan {second_time:.1?}"
    );
}
