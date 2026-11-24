//! Hashing throughput per algorithm, from memory (no I/O).

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use symbolon_core::hash::{HashAlgo, hash_reader};

fn throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_reader");
    for size in [64 * 1024_usize, 16 * 1024 * 1024] {
        let data: Vec<u8> = (0..=250u8).cycle().take(size).collect();
        group.throughput(Throughput::Bytes(data.len() as u64));
        for algo in HashAlgo::ALL {
            group.bench_with_input(BenchmarkId::new(algo.name(), size), &data, |b, data| {
                b.iter(|| hash_reader(algo, black_box(data.as_slice()), data.len() as u64));
            });
        }
    }
    group.finish();
}

criterion_group!(benches, throughput);
criterion_main!(benches);
