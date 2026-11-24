//! Walking and scanning a generated 10k-file tree. Set `TMPDIR` to choose the filesystem.

#![expect(clippy::expect_used, reason = "benchmark setup fails loudly by design")]

use std::fs;
use std::hint::black_box;
use std::ops::ControlFlow;
use std::path::Path;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use symbolon_core::catalog::{Catalog, HashPolicy, NewLocation};
use symbolon_core::scan::{ScanOptions, Scanner};
use symbolon_core::source::{LocalSource, Source};

const DIRS: u64 = 100;
const FILES_PER_DIR: u64 = 100;
const FILES: u64 = DIRS * FILES_PER_DIR;

fn tree(root: &Path) {
    for d in 0..DIRS {
        let dir = root.join(format!("d{d:03}"));
        fs::create_dir_all(&dir).expect("create bench dir");
        for f in 0..FILES_PER_DIR {
            fs::write(dir.join(format!("f{f:03}")), format!("file {d}/{f}\n"))
                .expect("write bench file");
        }
    }
}

fn catalog_with(root: &Path, dir: &Path, policy: HashPolicy) -> Catalog {
    let catalog = Catalog::open(dir.join("catalog.db")).expect("open catalog");
    catalog
        .add_location(NewLocation::local("bench", root).hash_policy(policy))
        .expect("add location");
    catalog
}

fn scan(catalog: &Catalog) {
    let summary = Scanner::start(catalog, "bench", ScanOptions::default())
        .expect("start scan")
        .wait()
        .expect("scan");
    black_box(summary);
}

fn benches(c: &mut Criterion) {
    let work = tempfile::tempdir().expect("tempdir");
    let root = work.path().join("root");
    tree(&root);

    let mut group = c.benchmark_group("scan_10k_files");
    group.sample_size(10);
    group.throughput(Throughput::Elements(FILES));

    let source = LocalSource::new(&root, &[]).expect("source");
    group.bench_function("walk", |b| {
        b.iter(|| {
            let mut n = 0_u64;
            source
                .walk(&mut |_| {
                    n += 1;
                    ControlFlow::Continue(())
                })
                .expect("walk");
            black_box(n)
        });
    });

    for (name, policy) in [
        ("first_scan_lazy", HashPolicy::Lazy),
        ("first_scan_eager", HashPolicy::Eager),
    ] {
        group.bench_function(name, |b| {
            b.iter_batched(
                || {
                    let dir = tempfile::tempdir().expect("tempdir");
                    let catalog = catalog_with(&root, dir.path(), policy);
                    (dir, catalog)
                },
                |(_dir, catalog)| scan(&catalog),
                BatchSize::PerIteration,
            );
        });
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let catalog = catalog_with(&root, dir.path(), HashPolicy::Eager);
    scan(&catalog);
    group.bench_function("rescan_unchanged", |b| b.iter(|| scan(&catalog)));
    group.finish();
}

criterion_group!(scan_benches, benches);
criterion_main!(scan_benches);
