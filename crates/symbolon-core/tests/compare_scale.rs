//! Spec §11: comparing two 200k-entry locations from the catalog in well under a few seconds.
//! Ignored by default (it builds a large catalog); `just test-acceptance` runs it.

#![expect(clippy::expect_used, reason = "test setup fails loudly by design")]

use std::time::Instant;

use rusqlite::{Connection, params};
use symbolon_core::catalog::{Catalog, NewLocation};
use symbolon_core::compare::{Category, CompareOptions, Comparison, RowFilter, Side};
use symbolon_core::hash::{HashAlgo, hash_bytes};
use symbolon_core::pathkey::RelPath;
use symbolon_core::query::Page;

const FILES: u64 = 200_000;
const PER_DIR: u64 = 100;
/// Empty directories only on a: each one is checked for anything beneath it.
const EMPTY_DIRS: u64 = 20_000;

/// Writes `FILES` file entries per location straight into the catalog, plus their directories:
/// on b, every 50th file has different content, every 100th exists only on one side, and every
/// 200th was moved; a also has `EMPTY_DIRS` empty directories.
fn populate(dir: &std::path::Path) -> Catalog {
    let catalog = Catalog::open(dir.join("catalog.db")).expect("open");
    let mut conn = Connection::open(catalog.path()).expect("connect");
    for label in ["a", "b"] {
        let root = dir.join(label);
        std::fs::create_dir(&root).expect("root");
        let location = catalog
            .add_location(NewLocation::local(label, &root))
            .expect("location");
        let tx = conn.transaction().expect("tx");
        let mut dirs = std::collections::BTreeSet::new();
        if label == "a" {
            dirs.extend((0..EMPTY_DIRS).map(|i| format!("empty{i:05}")));
        }
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, hash_full, seen_scan_id)
                     VALUES (?1, ?2, ?3, ?4, 'file', ?5, 0, ?6, 1)",
                )
                .expect("prepare");
            for i in 0..FILES {
                let (mut path, mut content) = (
                    format!("d{:05}/f{i:07}.jpg", i / PER_DIR),
                    format!("content {i}"),
                );
                if label == "b" {
                    if i % 200 == 0 {
                        path = format!("moved/f{i:07}.jpg");
                    } else if i % 100 == 0 {
                        path = format!("only_b/f{i:07}.jpg");
                        content = format!("new {i}");
                    } else if i % 50 == 0 {
                        content = format!("changed {i}");
                    }
                }
                let rel = RelPath::from_path(path.as_ref()).expect("path");
                let key = rel.key();
                dirs.insert(key.parent().to_owned());
                let digest = hash_bytes(HashAlgo::Blake3, content.as_bytes());
                insert
                    .execute(params![
                        location.id.get(),
                        rel.as_bytes(),
                        key.as_str(),
                        key.parent(),
                        content.len() as u64,
                        digest.as_bytes()
                    ])
                    .expect("insert");
            }
            let mut insert_dir = tx
                .prepare(
                    "INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, seen_scan_id)
                     VALUES (?1, CAST(?2 AS BLOB), ?2, '', 'dir', 0, 0, 1)",
                )
                .expect("prepare");
            for dir in &dirs {
                insert_dir
                    .execute(params![location.id.get(), dir])
                    .expect("insert dir");
            }
        }
        tx.commit().expect("commit");
    }
    catalog
}

#[test]
#[ignore = "builds two 200k-entry locations; run with `just test-acceptance`"]
fn compares_two_200k_entry_locations_quickly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let started = Instant::now();
    let catalog = populate(dir.path());
    let populated = started.elapsed();

    let started = Instant::now();
    let cmp = Comparison::new(
        &catalog,
        &[Side::location("a"), Side::location("b")],
        CompareOptions::default(),
    )
    .expect("compare");
    let compared = started.elapsed();

    let started = Instant::now();
    let summary = cmp.summary().expect("summary");
    let page = cmp
        .rows(
            &RowFilter::categories([Category::Different]),
            &Page::first(500),
        )
        .expect("rows");
    let read = started.elapsed();

    assert_eq!(summary.entries(), 2 * FILES + EMPTY_DIRS);
    assert_eq!(summary.get(Category::Moved).rows, FILES / 200);
    assert_eq!(
        summary.get(Category::OnlyIn).rows,
        2 * (FILES / 100 - FILES / 200) + EMPTY_DIRS
    );
    assert_eq!(
        summary.get(Category::Different).rows,
        FILES / 50 - FILES / 100
    );
    assert_eq!(page.items.len(), 500);
    // Timing only means something in optimized builds; CI runs this instrumented for coverage.
    if !cfg!(debug_assertions) {
        assert!(compared.as_secs_f64() < 3.0, "comparison took {compared:?}");
    }
    eprintln!(
        "200k × 2: catalog built in {populated:.1?}, compared in {compared:.2?}, summary + first page in {read:.0?}"
    );
}
