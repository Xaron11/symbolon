# Benchmarks

Baseline numbers, so regressions and improvements are visible. Re-measure and update this page when a change affects performance, and always state the machine.

## Running

| Command                                             | Measures                                                            |
| --------------------------------------------------- | ------------------------------------------------------------------- |
| `cargo bench -p symbolon-core --bench hash`         | Hashing throughput per algorithm, from memory                       |
| `cargo bench -p symbolon-core --bench scan`         | Walking, first scans (lazy, eager) and unchanged rescans, 10k files |
| `just test-acceptance` (add `--release` for timing) | Scan and rescan of a 100k-file tree, with assertions                |

Benchmarks create their trees in `TMPDIR`. On many Linux systems `/tmp` is a RAM disk (tmpfs), which measures CPU and SQLite rather than storage; point `TMPDIR` at a directory on the disk you care about. All numbers below are with a warm page cache: the files were just written.

## M1 baseline (2026-11-24)

Machine: Intel Core i7-9750H (6 cores, 12 threads), 2 TB SATA SSD (ext4), Linux 7.3, release build, default `ScanOptions` (4 hashing threads).

### Hashing, from memory, single thread

| Algorithm | 16 MiB input |
| --------- | ------------ |
| BLAKE3    | 3.3 GiB/s    |
| XXH3-128  | 9.3 GiB/s    |
| SHA-256   | 235 MiB/s    |

### Scanning 10k small files (100 directories × 100 files)

| Benchmark         | tmpfs  | SATA SSD | Per file on SSD |
| ----------------- | ------ | -------- | --------------- |
| Walk only         | 15 ms  | 18 ms    | 1.8 µs          |
| First scan, lazy  | 74 ms  | 95 ms    | 9.5 µs          |
| First scan, eager | 109 ms | 165 ms   | 16.5 µs         |
| Rescan, unchanged | 35 ms  | 49 ms    | 4.9 µs          |

A lazy first scan is the walk plus catalog inserts; eager adds hashing. An unchanged rescan only marks entries as seen and reads no file contents.

### 100k files (1,000 directories × 100 files, 10% duplicates)

| Step                                  | SATA SSD |
| ------------------------------------- | -------- |
| First scan, eager (walk, store, hash) | 3.2 s    |
| Rescan, unchanged                     | 0.45 s   |

### Against the targets in the specification (§11)

- **Walk 200k files in seconds on SSD:** met. Walking alone runs at about 540k entries/s, so 200k files take under half a second; a full eager first scan of 200k small files is about 6–7 s.
- **Diff of two 200k-entry locations in well under a few seconds:** measured in M2.
- **GUI responsive during scans:** measured in M6.

On phones and USB sticks, storage throughput dominates instead; see ADR-0003 for measurements over ADB.
