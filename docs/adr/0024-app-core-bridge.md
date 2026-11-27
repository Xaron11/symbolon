# 0024. The app–core bridge

- **Status:** Accepted
- **Date:** 2026-11-26

## Context

The desktop GUI ([ADR-0021](0021-gui-clients.md)) has to run the whole read-only workflow: manage locations, scan with live progress while browsing what has been found, and compare locations with drill-down into results of hundreds of thousands of rows (spec §9, §13). Tauri commands must stay thin wrappers over `symbolon-core`, core must stay free of Tauri and async runtimes ([ADR-0006](0006-core-feature-gates.md)), and screens reach the backend only through one frontend API layer (ADR-0021 §2). The data components read offset-based sources ([ADR-0022](0022-design-system.md) §6, [ADR-0023](0023-offset-reads.md)).

A comparison of 200k × 2 entries takes about 2 s to build; rebuilding it per command or per scroll is not an option. Comparisons can also have unverified rows that only hashing settles, which can take minutes over USB.

## Decision

1. **A stateful bridge.** `symbolon-app` keeps one `AppState` for the life of the window: the catalog, a registry of running scans by location label, and a registry of open comparisons by id. Commands are `async` `#[tauri::command]` functions of one line each that run an `AppState` method on Tauri's blocking pool, so catalog, device and file system work never blocks the window (Tauri runs synchronous commands on the main thread). Registries are plain Rust and are tested without Tauri. Starting a scan reserves its label, then probes the device outside the registry's lock; removing or renaming a location checks for a scan and acts under that lock, so neither can slip between the check and the start.
2. **One catalog with the CLI.** The app opens the same file as `symbolon` (`SYMBOLON_CATALOG`, else `<local data dir>/symbolon/catalog.db`) and finds the Android agent the same way, through helpers in `symbolon_core::catalog`.
3. **Streams through channels.** Scan progress, background verification and browse notices travel over Tauri channels passed to the command that starts them. Progress is sent at most every 100 ms per scan or comparison; browse notices at most once a second while a scan writes, and once when it ends. A gone receiver never stops the work: a window that reloads re-attaches to running scans with `scan_attach`. Tauri cannot tell the backend that a channel's page went away, so subscriptions are released explicitly: `browse_subscribe` returns an id for `browse_unsubscribe`.
4. **Show first, verify after.** `compare_open` builds the comparison and returns its totals at once, then verifies it on its own thread with a second `Comparison` over the same sides. After each hashing round the comparison the screen reads from refreshes, and the screen hears `ResultsChanged` with new totals. Verify after a cancel takes over the run that is winding down instead of being lost. Closing a comparison cancels its verification. Comparisons do not survive a reload: their ids are lost with the page, so the frontend closes all open comparisons when it starts and reopens the one in the URL. Quitting the app cancels running scans and waits up to 3 s for them to record themselves as cancelled.
5. **Typed bindings from Rust.** tauri-specta generates `ui/src/lib/api/bindings.ts` (types and one function per command) from the Rust command signatures. Its 2.x line is still a release candidate, so `tauri-specta`, `specta` and `specta-typescript` are pinned with `=` and upgraded only in their own PRs. The file is committed so frontend checks never need a Rust build; a Rust test fails when it is stale and `just bindings` rewrites it. 64-bit numbers cross as JavaScript numbers (exact to 2^53). Paths cross twice: as a lossy display string, and as the path key that is sent back, so no path round-trips through display text ([ADR-0004](0004-raw-path-encoding.md)).
6. **One error type.** Every command fails with `{ kind, message }`; `kind` is a closed list (`NotFound`, `AlreadyExists`, `AlreadyScanning`, `Moved`, `Ambiguous`, `OtherDevice`, `Unavailable`, `DeviceNotReady`, `AlgorithmMismatch`, `NotScannable`, `InvalidInput`, `Io`, `Internal`) mapped from core errors in one module, so screens can react to it.
7. **The frontend API layer.** `src/lib/api/` defines a `Backend` interface with a Tauri implementation and a memory implementation for tests and the gallery; screens import only from there (ESLint rejects Tauri and bindings imports elsewhere). Data sources over it feed the existing tables and trees.
8. **Live data sources.** This extends ADR-0022 §6: a `DataSource` or `TreeSource` may offer `subscribe(onChange)`. Tables and trees then re-read what they show, keeping scroll position, open folders and the active row; selection, being by position, is cleared.

## Consequences

- Scrolling, sorting and drilling down read pages of a comparison that stays built; only verification rounds rebuild it, a handful of times per comparison.
- The app and the CLI see each other's locations and scans; one location is never scanned twice at once across them ([ADR-0012](0012-scan-lock.md)).
- Pinned release candidates need deliberate upgrades; switching to another generator later touches only the bindings file and the API layer's Tauri implementation.
- A future HTTP backend for the web UI ([ADR-0021](0021-gui-clients.md)) implements the same `Backend` interface over the same `AppState` methods.
- Revisit if verification of very large comparisons makes the per-round refresh too slow, or if a client other than the Svelte one needs these types.
