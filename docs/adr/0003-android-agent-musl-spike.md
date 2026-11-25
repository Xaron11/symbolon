# 0003. Android agent: static musl binary (spike result)

- **Status:** Accepted; decision 3 superseded by [0018](0018-phone-hashing-threads.md)
- **Date:** 2026-11-24

## Context

Spec §6 prefers pushing a static `aarch64-unknown-linux-musl` scanner over ADB and running it via `adb exec-out`, with a switch to `aarch64-linux-android` (NDK) if musl misbehaves on real devices, and a `find` + `sha256sum` fallback. The scanner must work across devices, not only the test phone (for example Pixel 10 with Tensor G5 and optional 16 KiB pages).

The spike ran the PoC `symbolon-agent` (BLAKE3 with NEON, `rayon`, `mimalloc`, `release-agent` profile, 845 KB stripped) built with `cargo-zigbuild` (ADR-0002).

## Spike results (Samsung Galaxy A54, SM-A546B)

Device: Android 16 (SDK 36), kernel 5.15 (android13 GKI), 8 cores, 4 KiB pages, SELinux enforcing, toybox 0.8.12. `/sdcard` is FUSE (`/storage/emulated`, `noexec`).

| Check                                                                   | Result                                                                                                                 |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Push to `/data/local/tmp`, `chmod 755`, exec                            | Works. Runs as `uid=2000(shell)`, context `u:r:shell:s0`.                                                              |
| `/proc/self/status`, `/proc/self/attr/current`, `available_parallelism` | Readable; parallelism = 8.                                                                                             |
| `adb exec-out` NDJSON stream                                            | Clean, no line-ending mangling.                                                                                        |
| Walk `/sdcard` (27,567 files, 5,489 dirs, 17.6 GB)                      | 16.5 s cold, 3.9 s warm. One `EACCES` under `Android/data/<pkg>/…`.                                                    |
| Hash `/sdcard/DCIM` (1,084 files, 2.6 GB), BLAKE3                       | 1 thread 176 MiB/s, **2 threads 238 MiB/s**, 4 threads 227 MiB/s, 8 threads 213 MiB/s.                                 |
| Same tree, `find -print0 \| xargs -0 sha256sum`                         | ≈13.6 s (≈190 MiB/s); `xargs -P 2` ≈12.6 s.                                                                            |
| Unreadable file (EIO at ≈119 MB of a 143 MB video)                      | Agent: per-file `warning`, scan continues. **toybox `sha256sum`: prints a hash of the truncated content and exits 0.** |
| Cleanup                                                                 | Binary removed after the run (`trap … rm`).                                                                            |

No issues specific to musl: no NDK needed, no user/DNS lookups involved, `/proc` access fine, SELinux allows exec from `/data/local/tmp` for the shell domain.

## Decision

1. **Keep the static musl agent** (`aarch64-unknown-linux-musl`, built with zig) as the primary Android path. No NDK.
2. **Portability requirements** for every agent build, checked in CI or the release workflow from M5:
   - fully static, no `PT_INTERP`;
   - `LOAD` segment alignment ≥ 16 KiB (currently 64 KiB), so it runs on 16 KiB-page kernels (Pixel 8 and later in 16 KB mode, future devices);
   - baseline `aarch64` only (NEON is baseline); no CPU feature assumptions beyond ARMv8.0.
3. **I/O parallelism on Android defaults to 2 hashing threads.** Hashing through FUSE is I/O-bound, and more threads were slower. Keep it user-overridable, and measure again on other devices. _(Superseded by [ADR-0018](0018-phone-hashing-threads.md): four threads.)_
4. **The `sha256sum` fallback must not trust toybox's exit status.** It hashes each file as `cat -- "$f" | sha256sum` with `set -o pipefail` (mksh supports it; verified to report the EIO), or an equivalent that detects short reads. A file whose read fails is a warning, never a hash.
5. The agent walks `/sdcard` (FUSE). `/data/media/0` is not readable by the shell user. Paths under `Android/data` and `Android/obb` may be denied per package and are reported as warnings.

## Consequences

- Spec §5.4 is confirmed in practice: per-file errors exist on real phones (failing flash or FUSE), so warnings per entry are required, not optional.
- Not yet verified on: a Pixel (Tensor, 16 KiB page mode, MTE-enabled builds), Android ≤ 10, or devices with `noexec` on `/data/local/tmp`. Run `just android-probe` on a Pixel before closing M5; if exec from `/data/local/tmp` is blocked, the fallback path applies.
- The zig linker prints `ignoring deprecated linker optimization setting "1"`, which surfaces as a `linker_messages` warning. It is harmless; silence it when M5 sets the final agent linker flags.
- Measured throughput (~190–240 MiB/s over FUSE) puts a full 100 GB phone at roughly 7–9 minutes, bounded by storage speed. Re-measure on the real M5 scanner.

## M5 results on real phones (2026-11-25)

The M5 scanner (`AdbSource`, the agent built on core, the toybox fallback) was run through the CLI on both phones over USB. Every scan only read the phone, and nothing was left in `/data/local/tmp` afterwards.

**Pixel 10:** Android 17 (SDK 37), Tensor G5, 8 cores, kernel 6.6 with 4 KiB pages, SELinux enforcing, toybox 0.8.13. `/sdcard` is FUSE (`noexec`) over f2fs. The agent runs from `/data/local/tmp` as `uid=2000(shell)`, context `u:r:shell:s0`, as on the A54.

| Check                                             | Galaxy A54                | Pixel 10                                           |
| ------------------------------------------------- | ------------------------- | -------------------------------------------------- |
| `DCIM` with the agent, BLAKE3, 2 threads          | 2.4 GiB in 13.7 s         | 25.6 GiB in 48 s                                   |
| `DCIM` with the agent, SHA-256, 2 threads         | 2.4 GiB in 12.6 s         | 25.6 GiB in 47 s                                   |
| `DCIM` with the fallback, SHA-256                 | 55 s (≈ 45 MiB/s)         | 1,189 s (≈ 22 MiB/s)                               |
| Fallback digests against the agent's              | identical, 1,086 files    | identical, 11,728 files                            |
| Rescan, unchanged                                 | 1.1 s, all digests reused | —                                                  |
| Unreadable files                                  | 2 videos (EIO): warnings  | 4 `EACCES` under `Android/data`: warnings          |
| Files changed while hashed                        | —                         | 3 under `Android/data`: warnings, digest discarded |
| Whole `/sdcard` with the agent, BLAKE3, 2 threads | —                         | 17,183 files, 32.4 GiB hashed in 80.5 s            |
| Whole `/sdcard` with the agent, BLAKE3, 4 threads | —                         | 17,183 files, 32.4 GiB hashed in 51.6 s            |

The thread count was measured again on the Pixel; the result is [ADR-0018](0018-phone-hashing-threads.md).

The Pixel reports 97 GB in use, but the shell user sees only 32.5 GiB of it as files under `/sdcard`. The rest is app data outside the shell user's reach. A scan of the 100 GB the milestone names could not be run; at the measured rate of about 640 MiB/s for the whole scan it would take about two and a half minutes, bounded by the phone's storage.

Still not verified: a kernel in 16 KiB page mode (the Pixel ran a 4 KiB kernel; the release workflow still checks the agent's segment alignment), MTE-enabled builds, Android 10 or older, and a phone where `/data/local/tmp` is `noexec`. On the Pixel the fallback is about 25 times slower than the agent: it starts processes per file and hashes SHA-256 in toybox, so it suits small folders or phones the agent cannot run on.
