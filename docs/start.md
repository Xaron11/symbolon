# Symbolon - Starting point

You are the lead engineer bootstrapping **Symbolon**, a private (potentially public later) Rust project. Read this whole document before writing code. Treat it as a living spec: where it conflicts with reality (library limits, platform quirks, a better design), propose the change, record it as an ADR in `docs/adr/`, and proceed. Do not silently deviate.

The name comes from the ancient Greek _symbolon_: a token broken in two, each party keeping a half, later matched to prove identity. The tool answers one question: **do these locations match, and if not, how exactly do they differ?**

---

## 1. Problem statement

A tool that takes **two or more locations** (directory, mounted disk, pendrive, Android phone over ADB), scans them, hashes file contents, stores the results in a local catalog, and reports for any set of locations:

- which files are **identical** across locations
- which files are **missing** in which location (only-in-X)
- which files have the **same path but different content**
- which files were **moved/renamed** (same content hash, different path)
- which files are **duplicated** within a single location

Differentiators over existing tools (czkawka, jdupes, FreeFileSync, rclone check, hashdeep):

1. N-way comparison (2+ locations) in a single report, with all five categories above.
2. **Persistent catalog**: compare while a device is disconnected; rescans reuse cached hashes.
3. **Android scanning without copying data** (hash on-device, only the index crosses USB).
4. **Usable while scanning**: browse partial results of a running scan.
5. Core is a reusable library; CLI, GUI and a future TUI/mobile app are thin clients.

Typical scale: up to ~100 GB per location, 20k–200k files, mostly documents, images, videos. Design for 1M+ files without falling over.

## 2. Non-goals (v1)

- Dedicated Android app (keep the core mobile-ready, do not build it).
- Cloud remotes (S3, R2, etc.), network shares beyond what the OS mounts.
- Bidirectional sync semantics (conflict resolution, merge). Write operations are explicit, user-reviewed actions, not continuous sync.
- Byte-for-byte verification of hash-equal files (optional later flag).

## 3. Decisions already made

| Area            | Decision                                                                                                                                                                                                                                                                                                                      |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Language        | Rust, stable toolchain, edition 2021 or newer                                                                                                                                                                                                                                                                                 |
| GUI             | Tauri 2 + SvelteKit (static adapter, SSR disabled), Chart.js for pie/bar/doughnut charts                                                                                                                                                                                                                                      |
| Repo layout     | Cargo workspace: `symbolon-core` (lib), `symbolon-cli`, `symbolon-app` (Tauri)                                                                                                                                                                                                                                                |
| Storage         | SQLite (WAL) as the single source of truth, one global catalog DB with saved `locations`; per-location export/import as a standalone `.symbolon` file (a SQLite file with a Symbolon-specific `application_id` pragma)                                                                                                        |
| SQLite build    | `rusqlite` with the `bundled` feature, so the SQLite version and features are identical on Nix, Arch and Windows                                                                                                                                                                                                              |
| Default hash    | BLAKE3, configurable (`blake3`, `xxh3`, `sha256`), algorithm stored per location                                                                                                                                                                                                                                              |
| Hash strategy   | Tiered: size → partial hash → full hash; lazy by default, optional "hash everything now" per location                                                                                                                                                                                                                         |
| Android v1      | No app. Push a static `aarch64` scanner binary via `adb` (start with `aarch64-unknown-linux-musl`), fallback to `sha256sum` over `adb exec-out`                                                                                                                                                                               |
| Device identity | User-assigned location label is the primary identity; automatic fingerprint (filesystem UUID, ADB serial) is a best-effort hint, and the user is asked to confirm when it is missing or ambiguous                                                                                                                             |
| ADB             | Spawn system `adb` behind an `AndroidTransport` trait; `adb_client` crate can be a later alternative implementation                                                                                                                                                                                                           |
| Path matching   | Relative to location root; optional case-insensitive and Unicode-NFC-normalized key; raw bytes always preserved                                                                                                                                                                                                               |
| Write ops       | Read-only first. Writes are plan → review → execute, with quarantine instead of delete, post-copy hash verification, and a journal                                                                                                                                                                                            |
| Platforms       | Linux + Android first-class. Windows must not be made impossible (no Unix-only assumptions outside platform modules); full Windows support is a later milestone                                                                                                                                                               |
| License         | `MIT OR Apache-2.0`, at the user's option, the Rust ecosystem default ([ADR-0020](adr/0020-mit-or-apache-license.md); releases up to v0.7.0 were `GPL-3.0-or-later`). Enforce the dependency license policy with `cargo-deny`. No DCO and no CLA: contributions are accepted under the same dual license (inbound = outbound) |
| Nix             | `flake.nix` with `devShell`, `.envrc` (`use flake`), and a package output for the CLI                                                                                                                                                                                                                                         |
| Repo            | Private GitHub repo under `Xaron11`, structured so it can become public (no secrets, no hardcoded personal paths)                                                                                                                                                                                                             |

## 4. Architecture

### 4.1 Crates

```text
symbolon/
├── Cargo.toml                  # workspace
├── crates/
│   ├── symbolon-core/          # all logic, no UI, no Tauri, no async runtime requirement
│   ├── symbolon-cli/           # clap-based CLI, binary name: symbolon
│   └── symbolon-app/           # Tauri 2 backend; frontend in ./ui (SvelteKit)
├── docs/
│   ├── adr/                    # architecture decision records
│   └── schema.md               # catalog schema + versioning policy
├── flake.nix
├── .envrc
├── justfile
├── deny.toml
├── rust-toolchain.toml
└── .github/                    # workflows, PR template
```

Dependency rule: `cli` and `app` depend on `core`; `core` depends on neither. Heavy or platform-specific parts of `core` (ADB transport, device identity per OS) live behind modules/features so a mobile or WASM-ish consumer can use the pure parts.

### 4.2 Core public API (two independent halves)

1. **`Scanner`**: runs a scan of one location and streams events (`Started`, `FileDiscovered`, `FileHashed`, `Progress { files, bytes, rate }`, `Warning`, `Finished`) over a channel. Supports cancel and pause. Writes to the catalog as it goes.
2. **`Query`**: read-only access to scanned data: list locations, browse a tree, search, group by hash, run comparisons, produce reports. Must work while a scan is writing (WAL, short transactions, separate read connections).

A GUI, CLI or TUI is a client of both halves and may use them in the same process or in different processes. This is the property that makes "browse while scanning" and a future TUI cheap.

Concurrency model: threads + `rayon` + `crossbeam-channel`. Do not force `tokio` on library consumers. Tauri commands may wrap core calls with `spawn_blocking`.

### 4.3 Modules inside `symbolon-core` (suggested, adjust via ADR)

- `source`: `Source` trait (`walk`, `open`, `stat`) with `LocalSource` and `AdbSource`
- `transport::android`: `AndroidTransport` trait, `SystemAdb` implementation, device discovery, scanner push/exec, `sha256sum` fallback parser
- `scan`: walker, scheduler, hash pipeline, incremental logic
- `hash`: `HashAlgo` enum, streaming hashers, partial-hash helper
- `catalog`: SQLite schema, migrations, DAOs, export/import (internal storage layer; the public API is `Scanner` for writing scans and `Query` for reading, and clients should not touch `catalog` directly)
- `identity`: stable device/volume identity per platform
- `compare`: N-way classification engine
- `plan`: action planning (copy, move, quarantine) as pure data
- `exec`: plan executor with journal and verification (feature-gated, M7)
- `report`: CSV/JSON/HTML exporters
- `pathkey`: normalization (NFC, optional case fold), raw-bytes handling

### 4.4 Catalog schema (starting point, refine in `docs/schema.md`)

- `meta(schema_version, created_at, app_version)`
- `locations(id, label, device_identity, root_path, hash_algo, hash_policy, last_scan_at, ...)`
- `scans(id, location_id, started_at, finished_at, status, stats...)`
- `entries(id, location_id, rel_path_raw BLOB, path_key TEXT, size, mtime_ns, kind, hash_partial, hash_full, hash_algo, scanned_scan_id, flags)`
- Indexes: `(location_id, path_key)`, `(size)`, `(hash_full)`, `(hash_partial)`

Rules:

- The schema is a public contract. Version it from day one with forward-only migrations and tests that open a database created by every previous schema version.
- Store paths as raw bytes (Linux paths are not guaranteed UTF-8) plus a normalized `path_key` for matching. Never lossily convert the raw path.
- Per-location export is a self-contained SQLite file with the `.symbolon` extension, importable into another catalog with conflict handling (same fingerprint, different label, etc.). No separate manifest: the `meta` table (schema version, app version, export info) serves as one. Optional zstd compression (`.symbolon.zst`) can be added later without changing the format.

## 5. Scanning and hashing

### 5.1 Pipeline

1. **Walk** the location: collect `rel_path`, size, mtime, kind. No file reads. This is fast and gives instant partial results to the UI.
2. **Reuse** cached hashes where `size` and `mtime` are unchanged from the previous scan of the same location (mtime tolerance is configurable: FAT has 2 s resolution, Android mtimes can be unreliable).
3. **Prune**: a file whose size is unique across all locations being compared cannot match anything and does not need hashing for comparison purposes.
4. **Partial hash** (size + first and last 64 KiB) for size-collision candidates. This is a pruning filter only. **Never declare two files identical from a partial hash.**
5. **Full hash** for remaining candidates.
6. Mark entries with their hash state so the UI can show "unhashed / partial / verified".

### 5.2 Hash policy per location

- `lazy` (default for always-connected devices): hash only when needed by a comparison.
- `eager` (for removable media you will not have later): hash everything during the scan so the catalog is fully comparable offline.
- Rescans never start from scratch: unchanged files keep their hashes, only new or changed files are read.

### 5.3 Algorithm handling

- `HashAlgo` is an enum stored per location. Comparing locations with different algorithms must **refuse with a clear error** (or offer to rehash), never silently mismatch.
- BLAKE3 default (use the `blake3` crate with `rayon` and `mmap` features; verify current API for multithreaded hashing of large files). On HDD, USB sticks and phone storage, I/O is the bottleneck, so cap parallelism per device class (auto-detect rotational/removable where possible, user-overridable).
- Android fallback uses SHA-256 because that is what toybox provides; document that such locations are only comparable with other SHA-256 locations or after rehash.

### 5.4 Robustness

- Do not follow symlinks by default; record them as entries of kind `symlink`. Detect and avoid cycles.
- Handle unreadable files, permission errors, files changing during hashing, and vanishing devices. Record warnings per entry, never abort the whole scan for one file.
- Scans must be resumable after crash or unplug.
- Ignore patterns (`.gitignore` style) per location.

## 6. Android (v1, no app)

Goal: hash on the device, transfer only the index.

1. Discover devices via `adb devices -l`; handle unauthorized/offline states with clear messages.
2. **Preferred path:** cross-compile the `symbolon-core` scanner as a static `aarch64-unknown-linux-musl` binary (no NDK needed, simpler in Nix; use `mimalloc`, because musl's default allocator is slow under multithreading; NEON is baseline on aarch64 so BLAKE3 gets SIMD), `adb push` to `/data/local/tmp`, `chmod +x`, run through `adb exec-out`, stream line-delimited JSON events back. This gives BLAKE3, multithreading and one code path everywhere. Run a short spike on a real device (Samsung A54) first. If the musl binary has problems (SELinux, missing `/proc` entries, user/DNS lookups, anything odd), switch to `aarch64-linux-android` via the NDK and record the decision in an ADR.
3. **Fallback:** `adb exec-out` running `find` + `sha256sum` (use `-print0`/`xargs -0` or equivalent to survive odd file names). Use `exec-out`, not `shell`, to avoid line-ending mangling.
4. Verify per device, and do not assume: execution from `/data/local/tmp` may be blocked on some ROMs (SELinux/noexec), the shell user may lack access to some paths (`Android/data` is typically restricted without root), toybox applets differ across Android versions.
5. Device identity for Android: the user-assigned label is the primary identity. The ADB serial plus root path is only a best-effort fingerprint (the serial can change over wireless ADB). When it does not match a saved location, ask the user to confirm or pick the location instead of failing.
6. Keep the transport behind `AndroidTransport` so `adb_client` or a future companion app can replace it.

Mobile app constraint: keep `symbolon-core` free of desktop-only assumptions so a future app can reuse it (candidate routes: UniFFI/JNI with a native Kotlin UI, Tauri 2 mobile, or Flutter via `flutter_rust_bridge`). Do not choose now; do not block any of them.

## 7. Comparison engine

Input: 2 or more locations (each = location + subpath) from the catalog.
Output per entry/group:

- **Identical**: same path key and same full hash in all compared locations.
- **Different content**: same path key, different size or hash.
- **Only in X**: present in a subset of locations, with the exact set of locations shown (for N > 2 this is a presence matrix, not a binary).
- **Moved/renamed**: same full hash, different path across locations. Distinguish "moved" (same filename, different directory) from "renamed" (different filename).
- **Duplicates within a location**: same hash, multiple paths in one location.

Rules:

- Path equality uses `path_key`, with the case-insensitive and NFC options configurable per comparison.
- Same path + different size is classified as different content without hashing.
- Same path + same size + no hash yet triggers hashing (lazy policy) or an explicit "assume identical by size+mtime" fast mode that the report labels clearly as unverified.
- Comparison is a pure function of catalog state: deterministic, testable, streamable for huge result sets.
- Results are paginated and queryable (filter by category, extension, size, path prefix). Never load 200k rows into memory or the webview at once.

## 8. Write operations (designed early, implemented last)

Read-only is the priority. When writes arrive (M7), they must be safe by construction:

1. **Plan, never act directly.** Comparison results produce an `ActionPlan` (data): copy A→B, move within a location, quarantine. The plan is serializable and reviewable.
2. **Dry-run by default.** CLI requires an explicit `--apply`. GUI requires a review screen showing every action, bytes affected, and conflicts.
3. **No deletion.** "Delete" means move to a per-location quarantine/trash directory with a manifest, restorable.
4. **Verify after copy**: hash the destination and compare before marking success. Write to a temp name and atomically rename. Never overwrite without an explicit per-action opt-in.
5. **Journal**: every action is recorded (planned, started, verified, failed) in the catalog so interrupted operations can be resumed or rolled back.
6. **Preconditions**: require explicit user confirmation that the connected device is the intended location when the automatic fingerprint is missing or does not match the catalog (wrong drive plugged in), and refuse to act on entries that are stale compared to disk (re-stat before acting).
7. Never follow symlinks out of the location root; reject paths escaping the root (`..`, absolute paths).
8. Android writes (via `adb push/pull`) come after local writes are solid.

## 9. GUI (Tauri 2 + SvelteKit)

- Static SvelteKit build, SSR disabled, TypeScript strict. Tauri commands are thin wrappers over `symbolon-core`; business logic never lives in the frontend.
- Core views:
  - **Locations**: saved locations, connection status, last scan, hash policy, scan/rescan buttons.
  - **Scan progress**: live files/bytes/rate, per-location, cancel/pause; results browsable while scanning.
  - **Compare**: pick 2+ locations, summary dashboard (Chart.js pie/doughnut of identical / different / only-in-X / moved / duplicates, bar charts by size or extension), then expandable tree and list drill-down per category.
  - **Plan review** (M7).
- Large lists use virtualization and server-side (Rust) pagination. Streaming events via Tauri events/channels.
- Keep the UI minimal-bloat: no heavy component framework unless an ADR justifies it.
- Design for dark mode and keyboard navigation. Windows-compatible styling.

## 10. CLI

Binary `symbolon` (optional alias `sbn`). Suggested surface, adjust as needed:

```text
symbolon location add <label> <path|adb:serial:/sdcard/DCIM> [--policy lazy|eager] [--algo blake3]
symbolon location list
symbolon scan <label>... [--full]
symbolon diff <label>[:subpath] <label>[:subpath]... [--case-insensitive] [--format table|json|csv|html] [--only identical|different|missing|moved|dupes]
symbolon dupes <label>
symbolon export <label> -o file.symbolon
symbolon import file.symbolon
symbolon plan ...   # M7
```

Requirements: stable exit codes, `--json` for scripting, progress on stderr, results on stdout, shell completions, man page generation.

## 11. Quality bar

- **Safety:** `#![forbid(unsafe_code)]` in `core` unless an ADR justifies an exception. No `unwrap`/`expect` in non-test code paths that handle I/O. Typed errors (`thiserror` in libs, `anyhow` in binaries).
- **Style:** `rustfmt`, `clippy -D warnings`, no AI-style obvious comments, comments only for non-obvious "why". All code, identifiers and comments in English.
- **Tests:**
  - unit tests for hashing, path keys, classification
  - property tests (`proptest`) for the comparison engine invariants (e.g. identical ⇒ no other category, symmetry)
  - integration tests with generated directory trees (tempdirs) covering moves, renames, dupes, unicode, case collisions, symlinks, permission errors
  - catalog migration tests across schema versions
  - a mock `AndroidTransport` so Android logic is testable without a device
  - benchmarks (`criterion`) for hashing throughput and diff on 1M synthetic entries
- **Performance targets (verify, adjust via ADR):** walk 200k files in seconds on SSD, diff of two 200k-entry locations in well under a few seconds from catalog, GUI stays responsive during scans.
- **Security/privacy caveats to document:** catalogs contain full file paths and hashes of personal data, so treat exported files as sensitive; hashes can reveal that a known file exists; ADB grants broad device access, so never auto-authorize or persist keys beyond the system's `adb`; the pushed binary must be removed after use unless the user opts to keep it.

## 12. Repository and tooling setup

- `flake.nix`: `devShell` with Rust toolchain (pinned via `rust-toolchain.toml`), Tauri 2 Linux dependencies, Node/pnpm, `just`, `cargo-deny`, `cargo-nextest`, `adb`; plus a `packages.default` building the CLI. `.envrc` with `use flake`.
- `justfile`: `build`, `test`, `lint`, `fmt`, `deny`, `ui-dev`, `app-dev`, `bench`, `build-agent` (cross-compile).
- **CI (GitHub Actions):** fmt, clippy, nextest, `cargo-deny`, `nix build`; matrix Linux + Windows for `core` and `cli`; frontend lint/typecheck/build; cache with `Swatinem/rust-cache`. Release workflow producing CLI binaries and the Android scanner artifact on tags.
- **Conventions:** Conventional Commits, `CHANGELOG.md` (Keep a Changelog format, or `git-cliff`), `.github/pull_request_template.md` (summary, motivation, testing, screenshots for UI, schema/migration impact, security impact), `CONTRIBUTING.md` short, `docs/adr/0000-template.md`.
- Licensing:
  - `LICENSE-MIT` and `LICENSE-APACHE`, `license = "MIT OR Apache-2.0"` in every `Cargo.toml`, and the `license` field in `ui/package.json` ([ADR-0020](adr/0020-mit-or-apache-license.md); originally GPL-3.0-or-later). No per-file license headers (they can be added later with the `reuse` tool if REUSE compliance is ever needed).
  - `deny.toml` allowlist limited to licenses that let the binaries be distributed under MIT OR Apache-2.0 (for example MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, 0BSD). Reject GPL, proprietary and unknown licenses. Run `cargo deny check licenses` in CI and also check the npm tree (for example `license-checker`) for the frontend.
  - No DCO and no CLA. `CONTRIBUTING.md` states that contributions are dual licensed under `MIT OR Apache-2.0`. Do not add a sign-off check or a CLA unless I ask.
  - Release workflow: published binaries (including the Android scanner) link to the exact source tag and ship both license files.
  - A third-party tool bundled in releases (for example Android `platform-tools`/`adb`) is allowed only if its license permits redistribution; record it in `THIRD_PARTY.md`.
- Do not commit secrets, personal paths, or real device serials. Use fixtures.

## 13. Milestones and acceptance criteria

Work milestone by milestone. At the end of each, stop, summarize what was done, list open questions and risks, and wait for confirmation before the next.

**M0. Scaffolding.** Workspace, flake/devShell, CI, `deny.toml`, license files, ADR template, ADR-0001 (stack and layout), the name `symbolon` has been reported available, so only re-check crates.io before the first publish.
_Done when:_ `nix develop` works, `just test lint` pass, CI green on an empty-but-real skeleton.

**M1. Core scan (local).** `LocalSource`, walker, hash pipeline (BLAKE3 + enum), SQLite catalog with migrations, `Scanner` with events/cancel, incremental rescans.
_Done when:_ scanning a 100k-file tree populates the catalog; a second scan only reads changed files (test proves it); queries work during a running scan.

**M2. Compare engine.** N-way classification, path-key options, hash tiers with size pruning, pagination.
_Done when:_ integration and property tests cover all categories including moved vs renamed and case/Unicode edge cases; wrong-algorithm comparison is refused.

**M3. CLI.** Location management, scan, diff, dupes, JSON/CSV output, export/import.
_Done when:_ a full workflow (add two locations, scan, diff, export one) works end-to-end from the shell; CLI integration tests cover it.

**M4. Locations and device identity.** User-assigned location labels as the primary identity, with an automatic best-effort fingerprint (filesystem UUID on Linux, ADB serial for Android) used to recognize a device at a different mount point and to ask the user to confirm when it is ambiguous; eager vs lazy policy.
_Done when:_ replugging a pendrive at another mount point resolves to the same saved location, or prompts for confirmation when the fingerprint is ambiguous.

**M5. Android over ADB.** `AndroidTransport`, device discovery, pushed static scanner, `sha256sum` fallback, mock-based tests.
_Done when:_ a real phone location can be scanned without copying files, scanning 100 GB is bounded by device storage speed (measure and record), failures degrade to the fallback with clear messages.

**M6. GUI.** Tauri app with Locations, Scan progress (browse while scanning), Compare dashboard with charts, expandable tree/list.
_Done when:_ GUI can run the full read-only workflow on Linux; large result sets stay smooth (virtualized, paginated).

**M7. Safe write operations.** Action plan, review UI, executor with quarantine, post-copy verification, journal, resume.
_Done when:_ fault-injection tests (kill mid-copy, wrong drive, stale entry) never lose or corrupt data and the journal recovers cleanly.

**M8. Reports and polish.** HTML report, shell completions, Windows build in CI, docs, benchmark results in `docs/`.

## 14. How to work

- Start with M0. Before writing code, **ask me any blocking questions** (closed questions with a recommended default), then proceed.
- Prefer small, reviewable commits with Conventional Commit messages.
- When you hit a decision that this document does not settle, choose the simplest option that keeps the architecture flexible, record it in an ADR, and mention it in the milestone summary.
- State uncertainty explicitly. Do not invent library APIs: check the current docs of `blake3`, `rusqlite`, `tauri` v2, `clap` and others before relying on them.
- If you use a happy-path/PoC approach anywhere, label it `PoC` in code and in the summary, and add a follow-up task.

## 15. Open questions to resolve during M0

1. How to build the static musl Android scanner in the flake (toolchain overlay, linker), and the result of the real-device spike (musl vs `aarch64-linux-android`).
2. Raw path encoding on Windows (WTF-8 is the likely choice) and how it maps to the catalog's raw-bytes path column.
3. The `application_id` value for `.symbolon` files and the import conflict policy (same fingerprint with a different label, same label with a different fingerprint).
4. Which parts of `symbolon-core` need feature gates so a future mobile consumer can use the pure logic without the ADB and desktop identity modules.
