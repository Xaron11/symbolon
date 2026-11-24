# Catalog schema

**Status:** draft. The concrete tables land with M1; this document fixes the rules first.

## Versioning policy

- The schema is a public contract: the CLI, the app, exports and third-party tools read it.
- `PRAGMA user_version` holds the schema version (integer, starting at 1). `PRAGMA application_id = 0x53594D42` identifies Symbolon files (ADR-0005).
- Migrations are forward-only, numbered, and each runs in a single transaction. There are no down-migrations; a backup copy is made before migrating a user's catalog.
- A build refuses to open a catalog with a newer `user_version` than it knows, with a message naming the required version.
- Every released schema version keeps a fixture database in `crates/symbolon-core/tests/fixtures/schema/vN.sqlite`. Tests open each fixture with the current code, migrate it and check the data survived.
- Changing a released migration is not allowed; fix forward with a new one.
- The database runs in WAL mode with short write transactions so `Query` can read while `Scanner` writes.

## Starting point (from `docs/start.md` §4.4, refined in M1)

- `meta(key TEXT PRIMARY KEY, value TEXT)`: `schema_version`, `created_at`, `app_version`, `kind` (`catalog` | `export`), export info.
- `locations(id, label UNIQUE, device_identity, root_path, path_encoding, hash_algo, hash_policy, last_scan_at, …)`
- `scans(id, location_id, started_at, finished_at, status, stats…)`
- `entries(id, location_id, rel_path_raw BLOB, path_key TEXT, size, mtime_ns, kind, hash_partial, hash_full, hash_algo, scanned_scan_id, flags)`
- Indexes: `(location_id, path_key)`, `(size)`, `(hash_full)`, `(hash_partial)`.

Path encoding rules for `rel_path_raw` and `path_key` are in ADR-0004.
