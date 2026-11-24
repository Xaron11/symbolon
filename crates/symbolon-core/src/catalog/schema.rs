//! Forward-only schema migrations. Migration `n` (1-based) upgrades `user_version` `n - 1` to `n`.
//!
//! Never edit a released migration; add a new one and a fixture (`docs/schema.md`).

pub(crate) const MIGRATIONS: &[&str] = &[V1, V2];

/// Schema version this build creates and understands.
#[expect(
    clippy::cast_possible_truncation,
    reason = "there will never be 2^32 migrations"
)]
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

const V1: &str = r"
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    label TEXT NOT NULL UNIQUE,
    root_kind TEXT NOT NULL CHECK (root_kind IN ('local')),
    root_raw BLOB NOT NULL,
    path_encoding TEXT NOT NULL CHECK (path_encoding IN ('posix', 'wtf8')),
    fingerprint TEXT,
    hash_algo TEXT NOT NULL CHECK (hash_algo IN ('blake3', 'xxh3', 'sha256')),
    hash_policy TEXT NOT NULL CHECK (hash_policy IN ('lazy', 'eager')),
    ignore_patterns TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE scans (
    id INTEGER PRIMARY KEY,
    location_id INTEGER NOT NULL REFERENCES locations (id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    status TEXT NOT NULL
        CHECK (status IN ('running', 'completed', 'cancelled', 'failed', 'interrupted')),
    files INTEGER NOT NULL DEFAULT 0,
    dirs INTEGER NOT NULL DEFAULT 0,
    symlinks INTEGER NOT NULL DEFAULT 0,
    bytes INTEGER NOT NULL DEFAULT 0,
    hashed_files INTEGER NOT NULL DEFAULT 0,
    hashed_bytes INTEGER NOT NULL DEFAULT 0,
    reused_hashes INTEGER NOT NULL DEFAULT 0,
    warnings INTEGER NOT NULL DEFAULT 0,
    error TEXT
) STRICT;

CREATE INDEX scans_location ON scans (location_id, id);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    location_id INTEGER NOT NULL REFERENCES locations (id) ON DELETE CASCADE,
    rel_path_raw BLOB NOT NULL,
    path_key TEXT NOT NULL,
    parent_key TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('file', 'dir', 'symlink', 'other')),
    size INTEGER NOT NULL,
    mtime_ns INTEGER,
    hash_partial BLOB,
    hash_full BLOB,
    seen_scan_id INTEGER NOT NULL,
    flags INTEGER NOT NULL DEFAULT 0,
    UNIQUE (location_id, rel_path_raw)
) STRICT;

CREATE INDEX entries_key ON entries (location_id, path_key);
CREATE INDEX entries_parent ON entries (location_id, parent_key, path_key);
CREATE INDEX entries_size ON entries (size) WHERE kind = 'file';
CREATE INDEX entries_hash_full ON entries (hash_full) WHERE hash_full IS NOT NULL;
CREATE INDEX entries_hash_partial ON entries (hash_partial) WHERE hash_partial IS NOT NULL;

CREATE TABLE scan_warnings (
    id INTEGER PRIMARY KEY,
    scan_id INTEGER NOT NULL REFERENCES scans (id) ON DELETE CASCADE,
    rel_path_raw BLOB,
    kind TEXT NOT NULL,
    message TEXT NOT NULL
) STRICT;

CREATE INDEX scan_warnings_scan ON scan_warnings (scan_id);
";

/// Locations imported from `.symbolon` files (ADR-0013): their files are on another computer.
const V2: &str = r"
CREATE TABLE imported_locations (
    location_id INTEGER PRIMARY KEY REFERENCES locations (id) ON DELETE CASCADE,
    exported_at INTEGER NOT NULL,
    exported_by TEXT NOT NULL,
    imported_at INTEGER NOT NULL
) STRICT;
";
