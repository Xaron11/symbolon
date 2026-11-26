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

### Comparing two locations (M2, 2026-11-24)

Entries are read from the catalog only; no file is touched. Generated data: every 50th file differs, every 100th exists only on one side, every 200th was moved.

| Comparison                                   | Time   |
| -------------------------------------------- | ------ |
| 50k × 2, exact keys                          | 0.35 s |
| 50k × 2, NFC and case-insensitive keys       | 0.40 s |
| 200k × 2, exact keys                         | 1.6 s  |
| 200k × 2: summary and first page of 500 rows | 0.1 s  |

The first 200k × 2 measurement was 4.2 s. Profiling showed index maintenance during the bulk load, a per-file row-assignment pass, and per-file name computation; building indexes after loading, finding path rows by key instead of assigning every file, computing names only for move candidates, and building lookup indexes only when hashing or listing duplicates brought it to 1.6 s.

Since directories and symlinks are compared too ([ADR-0019](adr/0019-directories-and-symlinks-in-comparisons.md), 2026-11-26), the 200k × 2 test also holds each side's 2,000 directories plus 20,000 empty directories on one side, each of which is checked for anything compared beneath it. The comparison takes 2.1 s; the same files without directories took 1.7 s on the same machine. Deriving the kinds of a row with `max()` instead of `DISTINCT` aggregates saved 0.1 s.

### Against the targets in the specification (§11)

- **Walk 200k files in seconds on SSD:** met. Walking alone runs at about 540k entries/s, so 200k files take under half a second; a full eager first scan of 200k small files is about 6–7 s.
- **Diff of two 200k-entry locations in well under a few seconds:** met, 2.1 s with their directories (release build; the acceptance test fails above 3 s).
- **GUI responsive during scans:** measured in M6.

On phones and USB sticks, storage throughput dominates instead; see the next section.

## Phones over ADB (M5, 2026-11-25)

Hashing runs on the phone (the agent, [ADR-0017](adr/0017-android-over-adb.md)); only the index crosses USB. Phones connected over USB to the M1 machine, release builds. Details per device, warnings and the fallback are in [ADR-0003](adr/0003-android-agent-musl-spike.md#m5-results-on-real-phones-2026-11-25).

| Scan                                                   | Galaxy A54        | Pixel 10                       |
| ------------------------------------------------------ | ----------------- | ------------------------------ |
| `DCIM` with the agent, BLAKE3, 2 threads               | 2.4 GiB in 13.7 s | 25.6 GiB in 48 s (542 MiB/s)   |
| `DCIM` with the agent, BLAKE3, 4 threads (the default) | —                 | 25.6 GiB in 30 s (870 MiB/s)   |
| `DCIM` with the fallback (toybox `sha256sum`)          | 55 s              | 1,189 s (22 MiB/s)             |
| Whole `/sdcard` with the agent, BLAKE3, 4 threads      | —                 | 32.4 GiB in 51.6 s (640 MiB/s) |
| Rescan, unchanged                                      | 1.1 s             | —                              |

The hashing thread count is set by [ADR-0018](adr/0018-phone-hashing-threads.md), which has the measurements for 1 to 8 threads.

**Against the milestone target (100 GB phone, bounded by storage speed):** the Pixel holds 97 GB, but only 32.5 GiB of it is readable as files by the shell user. At the whole-`/sdcard` rate, 100 GB takes about two and a half minutes, bounded by the phone's storage. The fallback is 4 to 25 times slower than the agent and is meant for phones the agent cannot run on.
