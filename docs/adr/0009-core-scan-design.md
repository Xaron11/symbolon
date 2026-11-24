# 0009. Core design for scanning and the catalog (M1)

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

M1 implements local scanning: hashing, paths, the SQLite catalog, `Scanner`, incremental rescans and `Query` while scanning (spec §4–§5). The spec fixes the goals; this records the concrete design choices so later milestones (compare, CLI, GUI, Android) build on stable ground.

## Decision

### Catalog access

- `Catalog` is a cheap, cloneable handle around the database path. It manages locations itself (add, list, remove). Each `Scanner` opens one writer connection; each `Query` opens its own read-only connection. Connections are never shared between threads.
- WAL mode, `synchronous = NORMAL`, foreign keys on, 10-second busy timeout. Write transactions are short batches, so readers never wait long.
- Core takes an explicit catalog path; choosing a default location (XDG data directory, platform equivalents) is the clients' job (CLI in M3).

### Schema v1 choices (details in `docs/schema.md`)

- `STRICT` tables; timestamps in Unix milliseconds, file times in nanoseconds.
- One hash algorithm per location, not per entry: switching algorithms means rehashing the location, and a digest can never be read with the wrong algorithm. In memory, `Digest` carries its algorithm and digests of different algorithms never compare equal.
- Hash state is derived from which digest columns are set, not stored separately.
- Entries keep `parent_key` for cheap tree browsing, and `seen_scan_id` for incremental scans; entries not seen by a completed scan are deleted at its end (history is not kept in M1).
- Location roots are stored as raw bytes in the platform's path encoding. Roots recorded on another platform family load as `LocationRoot::Foreign`: listed and comparable, not scannable.
- Labels: 1–64 characters without `:` (reserved for `label:subpath`), `/`, `\`, control characters or surrounding spaces.
- Migrations back up the catalog with `VACUUM INTO` and run under `BEGIN IMMEDIATE`; one fixture per released schema version is tested.

### Hashing

- Files are hashed by streaming with a 256 KiB buffer, never memory-mapped: a mapped file on removable media that disappears mid-hash kills the process with `SIGBUS` (spec §5.4). Parallelism comes from hashing several files at once.
- A digest is only accepted when exactly the recorded size was read, and the file is re-checked (size, mtime) after hashing; a changed file becomes a warning, not a digest.

### Scan pipeline

1. A new scan row starts as `running`. Any older `running` scan of the same location is marked `interrupted` first.
2. Walk the location (no file reads) and upsert entries in batched transactions. An entry whose kind, size and mtime are unchanged (mtime within a configurable tolerance) keeps its digests; otherwise they are cleared.
3. If the walk completed, delete entries the scan did not see.
4. Policy `eager`: compute full digests for every file without one. Policy `lazy`: hash nothing during the scan; comparisons hash what they need (M2).
5. Hash workers send results to the single writer, which commits them in batches. Hash parallelism is capped per device class (default 2 for removable or slow media, ADR-0003), overridable.
6. Mark the scan `completed` (or `cancelled` / `failed`) with its counters.

Events go over a bounded `crossbeam-channel` (back-pressure instead of unbounded memory): `Started`, `Progress` (throttled), `Warning`, `Finished`, plus per-file `FileDiscovered` / `FileHashed` only when requested, since a 200k-file scan would otherwise flood clients. Cancel and pause are flags checked between files. Everything committed before a cancel, crash or unplug stays valid, so the next scan reuses it: resuming is simply scanning again.

## Consequences

- Clients never touch SQL; `Catalog`, `Scanner` and `Query` are the API.
- Lazy locations have no digests until a comparison needs them, so M1 tests use `eager` for anything involving hashes.
- Dropping history (deleting unseen entries) keeps the catalog small; if scan history is wanted later, it needs a new migration.
