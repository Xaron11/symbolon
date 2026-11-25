# 0018. Four hashing threads on phones

- **Status:** Accepted
- **Date:** 2026-11-25

## Context

[ADR-0003](0003-android-agent-musl-spike.md) decision 3 set the agent's default to two hashing threads, because more threads were slower on the Galaxy A54 and hashing through FUSE looked I/O-bound. It asked to measure again on other devices.

The M5 tests on a Pixel 10 (Tensor G5) gave the opposite result. BLAKE3 over `/sdcard/DCIM` (11,728 files, 25.6 GiB), with the agent run directly, timed over USB:

| Threads | Galaxy A54 (spike, 2.6 GB) | Pixel 10 (25.6 GiB)  |
| ------- | -------------------------- | -------------------- |
| 1       | 176 MiB/s                  | 247 MiB/s (106 s)    |
| 2       | **238 MiB/s**              | 542 MiB/s (48 s)     |
| 4       | 227 MiB/s                  | 870 MiB/s (30 s)     |
| 8       | 213 MiB/s                  | **1.2 GiB/s** (22 s) |

A second two-thread run on the Pixel took 49 s, so caching did not skew the results; the tree is larger than the phone's RAM. All thread counts produced the same digests.

The best thread count depends on the phone's storage, and the agent cannot tell in advance which kind it is on.

## Decision

1. **The agent hashes with four threads by default**: `AdbOptions::default().threads` and the agent's `--threads` default. On the A54 this costs about 5% against two threads; on the Pixel 10 it is 1.6 times faster.
2. The thread count stays a parameter (`AdbOptions::threads`, `agent hash --threads`). It is not adapted per device yet.
3. This supersedes ADR-0003 decision 3. The rest of ADR-0003 stands.

## Consequences

- A whole-`/sdcard` scan of the Pixel 10 (17,183 files, 32.4 GiB) went from 80.5 s to 51.6 s, so 100 GB takes about two and a half minutes instead of about four. Phones like the A54 take slightly longer than before.
- Eight threads would be faster still on fast storage but costs about 10% on slow storage and keeps more cores busy, which means more heat and battery drain during a scan.
- Revisit with measurements from more phones, or if scans become long enough to justify picking the thread count per device, for example by timing the first files hashed.
