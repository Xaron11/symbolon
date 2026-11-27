# Architecture

This chapter describes how the pieces fit. Items marked _(planned)_ do not exist yet; the milestone that brings them is in brackets.

## Crates

```mermaid
flowchart LR
    cli["symbolon-cli<br/>(binary: symbolon)"] --> core
    app["symbolon-app<br/>(Tauri 2 + SvelteKit)"] --> core
    agent["symbolon-agent<br/>(static aarch64, on the phone)"] --> core
    core["symbolon-core<br/>(all logic)"]
    ui["ui/ (SvelteKit)"] -. "Tauri commands & events" .-> app
```

`symbolon-core` depends on none of the other crates and needs no async runtime. Everything a user can do is a core API call; the clients only parse input, call core and render output. Platform-specific parts of core (ADB, device identity) sit behind Cargo features so a mobile app could reuse the pure logic ([ADR-0006](adr/0006-core-feature-gates.md)).

## Two halves: `Scanner` and `Query`

Core exposes two independent APIs over one catalog:

```mermaid
flowchart LR
    subgraph writer["Scanner: one per scan"]
        direction TB
        walk[walk] --> reuse[reuse cached hashes] --> hash[hash as needed]
    end
    subgraph readers["Query: concurrent readers"]
        direction TB
        browse[browse / search] ~~~ compare[compare] ~~~ report[report]
    end
    db[("catalog<br/>SQLite, WAL")]
    client[CLI / GUI]
    writer -- "short write transactions" --> db
    db -- "committed data" --> readers
    writer -- "events" --> client
    readers -- "results" --> client
```

- **`Scanner`** scans one location, writes entries in short transactions and streams events over a channel. It supports cancel and pause.
- **`Query`** is read-only. It uses separate SQLite connections in WAL mode, so it can browse partial results while a scan is still writing.
- **`Comparison`** compares two or more sides (locations or subpaths) from the catalog. It copies the compared entries into temporary tables on its own connection, classifies them with SQL, and serves a summary, rows by cursor or by offset in path or size order, a folder tree with totals, and counts by extension and size; `resolve` hashes on demand what decides a row ([ADR-0010](adr/0010-comparison-engine.md), [ADR-0023](adr/0023-offset-reads.md)).
- **`Catalog::export` / `Catalog::import`** move one location between catalogs as a `.symbolon` file; imported locations compare like any other but are never scanned ([ADR-0005](adr/0005-export-format-and-import-conflicts.md), [ADR-0013](adr/0013-imported-locations.md)).

Only one scan of a location runs at a time, across processes, enforced by an OS file lock ([ADR-0012](adr/0012-scan-lock.md)). The `symbolon` CLI is a thin client of these APIs: it parses arguments, calls core, and renders tables, JSON or CSV ([ADR-0011](adr/0011-cli-conventions.md), [Command-line usage](cli.md)).

A client may run both in one process (the GUI) or in different processes (a CLI scan in one terminal, a TUI browsing in another).

## Scan pipeline

```mermaid
flowchart TD
    A[Walk: path, size, mtime, kind<br/>no file reads] --> B{size and mtime<br/>unchanged since last scan?}
    B -- yes --> C[reuse stored hashes]
    B -- no --> D{size unique among<br/>compared locations?}
    D -- yes --> E[skip: cannot match anything]
    D -- no --> F[partial hash: first and last 64 KiB]
    F --> G{partial hashes collide?}
    G -- no --> E
    G -- yes --> H[full hash]
```

With the `eager` policy every file gets a full hash during the scan; with `lazy` the pruning steps run when a comparison needs them. See [Concepts → Hash tiers](concepts.md#hash-tiers).

## Concurrency

- Plain threads, [`rayon`](https://docs.rs/rayon) for parallel hashing and [`crossbeam-channel`](https://docs.rs/crossbeam-channel) for events. No `tokio` in core; the Tauri backend wraps blocking calls with `spawn_blocking`.
- Hashing parallelism is capped per device class: I/O, not CPU, is the bottleneck on HDDs, USB sticks and phones. On Android phones, which thread count is fastest depends on the storage: two on a Galaxy A54, eight on a Pixel 10. Four is close to the best on both ([ADR-0018](adr/0018-phone-hashing-threads.md)).
- One writer connection per scan; readers use their own connections. Write transactions stay short so readers are never blocked for long.
- Scan events are best-effort for slow clients: a full buffer drops progress and per-file events instead of pausing the scan; `Finished` is always delivered, and the catalog is the authoritative record.

## Android

```mermaid
sequenceDiagram
    participant Host as symbolon (host)
    participant ADB as adb
    participant Agent as symbolon-agent (phone)
    Host->>ADB: push static binary to /data/local/tmp
    Host->>ADB: exec-out symbolon-agent /sdcard/…
    ADB->>Agent: run as shell user
    Agent-->>Host: NDJSON events (entries, hashes, warnings)
    Host->>ADB: rm binary
```

Files are hashed on the phone, so only the index crosses USB. If the agent cannot run, a fallback hashes with the phone's own `sha256sum`, guarding against its silent short reads ([ADR-0003](adr/0003-android-agent-musl-spike.md)). The transport sits behind an `AndroidTransport` trait _(planned, M5)_, which tests replace with a mock.

## The desktop app

The app is a thin client of both halves ([ADR-0024](adr/0024-app-core-bridge.md)). Screens talk to a `Backend` interface; its Tauri implementation calls commands that run methods of one long-lived `AppState` on a blocking pool.

```mermaid
flowchart LR
    screens["Screens (Svelte)"] --> api["API layer<br/>Backend, data sources"]
    api -- "commands (bindings.ts)" --> commands["#[tauri::command]<br/>one line each"]
    commands --> state["AppState<br/>catalog, scan registry,<br/>comparison registry"]
    state --> core[symbolon-core]
    state -. "channels: scan progress,<br/>verification, browse notices" .-> api
```

- **Locations and browsing** are direct `Catalog` and `Query` calls.
- **Scans** run as in the CLI; a forwarding thread passes their events to the window's channels, throttled, and keeps the latest one for windows that attach later.
- **Comparisons** stay built for as long as a screen shows them. They appear at once from the catalog and are verified in the background; each hashing round refreshes them and tells the screen.

The TypeScript types and command functions in `ui/src/lib/api/bindings.ts` are generated from the Rust signatures (tauri-specta) and checked for drift by `just test`.

## Catalog

One SQLite database (WAL mode, `rusqlite` with bundled SQLite) holds locations, scans and entries. The schema is a versioned public contract with forward-only migrations ([Catalog schema](schema.md)). Paths are stored as raw bytes plus a normalized key ([ADR-0004](adr/0004-raw-path-encoding.md)).

## Where to look next

- The [specification](start.md) for requirements and milestones.
- The [decision records](adr/index.md) for why things are the way they are.
- The [API reference](development/api.md) for the code itself.
