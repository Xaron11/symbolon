# Catalog schema

The catalog is one SQLite database. Its schema is a public contract: the CLI, the app, `.symbolon` exports and third-party tools read it. This page describes the current version (**v4**) and the rules for changing it. The DDL lives in `crates/symbolon-core/src/catalog/schema.rs`.

## Versioning policy

- `PRAGMA user_version` holds the schema version (`SCHEMA_VERSION` in code). `PRAGMA application_id = 0x53594D42` (ASCII `SYMB`) identifies Symbolon files ([ADR-0005](adr/0005-export-format-and-import-conflicts.md)).
- Migrations are forward-only and numbered; each runs in one `BEGIN IMMEDIATE` transaction, so two processes opening the same old catalog cannot both migrate it.
- Before migrating an existing catalog, Symbolon writes a copy next to it: `<file>.v<old version>.bak` (`VACUUM INTO`).
- A build refuses a catalog with a newer `user_version` than it knows, and any SQLite file without Symbolon's `application_id`.
- Every released schema version has a fixture, `crates/symbolon-core/tests/fixtures/schema/vN.sqlite`. Tests open each fixture with the current code and check that its data survives migration. After adding a migration, run `just schema-fixture` once; never regenerate a released fixture.
- Never edit a released migration; fix forward with a new one.
- The database runs in WAL mode with `synchronous = NORMAL` and short write transactions, so `Query` reads committed data while `Scanner` writes.

## Conventions

- Tables are `STRICT`. Timestamps are milliseconds since the Unix epoch (`*_at`); file modification times are nanoseconds (`mtime_ns`).
- Paths are stored as raw bytes plus a matching key ([ADR-0004](adr/0004-raw-path-encoding.md)). Relative paths use `/` between components; the location root is the empty path.
- Digests are raw bytes. The algorithm is a property of the location, not of each digest: every digest of a location uses `locations.hash_algo`.
- Enumerations are lowercase text with a `CHECK` constraint.

## Tables (v4)

### `meta`

Key/value facts about the file: `kind` (`catalog` or `export`), `created_at`, `created_by` (app version). Exports add `exported_at`, `exported_by`, and the exported location's `label`, `hash_algo`, `path_encoding` and `fingerprint` (when known).

### `locations`

| Column            | Type    | Notes                                                                                                                                                                                                                                             |
| ----------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`              | INTEGER | Primary key.                                                                                                                                                                                                                                      |
| `label`           | TEXT    | Unique; 1–64 characters, no `:`, `/`, `\`, control characters or surrounding spaces. The location's identity.                                                                                                                                     |
| `root_kind`       | TEXT    | `local`. Imported locations have an `imported_locations` row, and phone locations an `adb_locations` row.                                                                                                                                         |
| `root_raw`        | BLOB    | Absolute root path in `path_encoding`.                                                                                                                                                                                                            |
| `path_encoding`   | TEXT    | `posix` or `wtf8`; applies to the root and all entry paths.                                                                                                                                                                                       |
| `fingerprint`     | TEXT    | Best-effort device identity: `fs:<filesystem UUID>:<path inside the filesystem>` ([ADR-0016](adr/0016-device-fingerprints.md)), or `adb:<ro.serialno>:<path>` for phones ([ADR-0017](adr/0017-android-over-adb.md)); NULL when none was detected. |
| `hash_algo`       | TEXT    | `blake3`, `xxh3` or `sha256`.                                                                                                                                                                                                                     |
| `hash_policy`     | TEXT    | `lazy` or `eager`.                                                                                                                                                                                                                                |
| `ignore_patterns` | TEXT    | `.gitignore`-style patterns, one per line.                                                                                                                                                                                                        |
| `created_at`      | INTEGER |                                                                                                                                                                                                                                                   |

### `imported_locations` (v2)

Locations created by importing a `.symbolon` file ([ADR-0013](adr/0013-imported-locations.md)); their files are on another computer, so they are compared but never scanned. `location_id` (primary key, cascades on delete), `exported_at`, `exported_by` (app version that wrote the file), `imported_at`.

### `adb_locations` (v3)

Locations on Android phones ([ADR-0017](adr/0017-android-over-adb.md)). They have two columns: `location_id`, the primary key, which cascades on delete, and `serial`, the connection serial that `adb -s` takes. The path on the phone is in `locations.root_raw`, with `posix` encoding. Such a location is shown as `adb:<serial>:<path>`.

### `scans`

One row per scan of a location. `status` is `running`, `completed`, `cancelled`, `failed` or `interrupted` (a `running` scan found by a later scan, after a crash or unplug). Counters: `files`, `dirs`, `symlinks`, `bytes`, `hashed_files`, `hashed_bytes`, `reused_hashes`, `warnings`; `error` holds the reason for `failed`. Deleting a location deletes its scans.

### `entries`

One row per file, directory, symlink or other object of a location.

| Column         | Type    | Notes                                                                                                                                                                                                  |
| -------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`           | INTEGER | Primary key.                                                                                                                                                                                           |
| `location_id`  | INTEGER | References `locations`; cascades on delete.                                                                                                                                                            |
| `rel_path_raw` | BLOB    | Raw relative path; unique per location.                                                                                                                                                                |
| `path_key`     | TEXT    | Matching key of `rel_path_raw`.                                                                                                                                                                        |
| `parent_key`   | TEXT    | `path_key` of the parent directory (`''` at the root), for tree browsing.                                                                                                                              |
| `kind`         | TEXT    | `file`, `dir`, `symlink` or `other`.                                                                                                                                                                   |
| `size`         | INTEGER | Bytes; 0 for directories.                                                                                                                                                                              |
| `mtime_ns`     | INTEGER | Modification time; nullable when unavailable.                                                                                                                                                          |
| `hash_partial` | BLOB    | Partial digest (size + first and last 64 KiB); nullable. Pruning only.                                                                                                                                 |
| `hash_full`    | BLOB    | Full-content digest; nullable.                                                                                                                                                                         |
| `seen_scan_id` | INTEGER | Last scan that saw the entry.                                                                                                                                                                          |
| `flags`        | INTEGER | Reserved, 0.                                                                                                                                                                                           |
| `link_target`  | BLOB    | Symlink target as raw bytes in `path_encoding`, exactly as `readlink` returns it ([ADR-0019](adr/0019-directories-and-symlinks-in-comparisons.md)); NULL for other kinds or when unknown. Added in v4. |

The hash state shown to users is derived: `verified` when `hash_full` is set, `partial` when only `hash_partial` is, `unhashed` otherwise. Hashes are cleared whenever `size` or `mtime_ns` change. Every scan rereads each symlink's `link_target`, because retargeting a link need not change its mtime. Entries not seen by a completed scan are deleted at its end.

Indexes: `(location_id, rel_path_raw)` unique, `(location_id, path_key)`, `(location_id, parent_key, path_key)`, `(size)` for files, `(hash_full)` and `(hash_partial)` where not null.

### `scan_warnings`

Per-entry problems found during a scan (permission denied, vanished, changed while hashing, I/O errors): `scan_id`, `rel_path_raw` (nullable for scan-wide warnings), `kind`, `message`.

## `.symbolon` export files

An export is a database with the same schema, `meta.kind = 'export'` and exactly one location: its newest completed scan and its entries, all marked as seen by that scan. It uses `journal_mode = DELETE`, so it is one self-contained file. `Catalog::export` writes it and `Catalog::import` reads it; the conflict rules are in [ADR-0005](adr/0005-export-format-and-import-conflicts.md) and [ADR-0013](adr/0013-imported-locations.md). Exports contain full paths and hashes of private files: treat them as sensitive.

An export comes from another catalog, so `Catalog::import` does not trust it. It opens the file read-only, with SQLite's defensive mode on and the schema untrusted. The header's `hash_algo` and `path_encoding` must match the location row. Every entry is checked before anything is written:

- `rel_path_raw` must be a valid relative path in its encoding. For `wtf8`, this also means no `\` or `:`, which on Windows would be a separator, a drive or a stream.
- `path_key` and `parent_key` must be the keys derived from it.
- Each digest must have its algorithm's length.
- `link_target` may be set only on `symlink` entries, holds at most 4,096 bytes and no NUL, and must be valid in its encoding. Exports from schema v3 have no `link_target` and import with unknown targets.

A file that fails a check is refused with `TransferError::InvalidEntry`.
