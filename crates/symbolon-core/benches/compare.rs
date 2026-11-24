//! Comparing two 50k-entry locations from the catalog (the 200k acceptance test covers scale).

#![expect(clippy::expect_used, reason = "benchmark setup fails loudly by design")]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rusqlite::{Connection, params};
use symbolon_core::catalog::{Catalog, NewLocation};
use symbolon_core::compare::{CompareOptions, Comparison, Side};
use symbolon_core::hash::{HashAlgo, hash_bytes};
use symbolon_core::pathkey::{KeyOptions, RelPath};

const FILES: u64 = 50_000;

/// Two locations; on b every 50th file differs, every 100th is only on b, every 200th moved.
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
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, hash_full, seen_scan_id)
                     VALUES (?1, ?2, ?3, ?4, 'file', ?5, 0, ?6, 1)",
                )
                .expect("prepare");
            for i in 0..FILES {
                let (mut path, mut content) = (
                    format!("d{:04}/F{i:06}.jpg", i / 100),
                    format!("content {i}"),
                );
                if label == "b" {
                    if i % 200 == 0 {
                        path = format!("moved/F{i:06}.jpg");
                    } else if i % 100 == 0 {
                        content = format!("new {i}");
                        path = format!("only_b/F{i:06}.jpg");
                    } else if i % 50 == 0 {
                        content = format!("changed {i}");
                    }
                }
                let rel = RelPath::from_path(path.as_ref()).expect("path");
                let key = rel.key();
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
        }
        tx.commit().expect("commit");
    }
    catalog
}

fn benches(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    let catalog = populate(dir.path());
    let sides = [Side::location("a"), Side::location("b")];
    let mut group = c.benchmark_group("compare_50k_x2");
    group.sample_size(10);
    group.throughput(Throughput::Elements(2 * FILES));
    group.bench_function("exact_keys", |b| {
        b.iter(|| {
            black_box(
                Comparison::new(&catalog, &sides, CompareOptions::default()).expect("compare"),
            )
        });
    });
    let normalized = CompareOptions::default().keys(KeyOptions {
        nfc: true,
        case_insensitive: true,
    });
    group.bench_function("nfc_case_insensitive", |b| {
        b.iter(|| {
            black_box(Comparison::new(&catalog, &sides, normalized.clone()).expect("compare"))
        });
    });
    group.finish();
}

criterion_group!(compare_benches, benches);
criterion_main!(compare_benches);
