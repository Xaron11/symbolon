//! Every released catalog schema version keeps a fixture in `tests/fixtures/schema/`; each must
//! still open (migrating forward) with its data intact. See `docs/schema.md`.

#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use symbolon_core::catalog::{Catalog, SCHEMA_VERSION};
use symbolon_core::hash::HashAlgo;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema")
}

fn fixture_versions() -> Vec<u32> {
    let mut versions: Vec<u32> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            name.strip_prefix('v')?
                .strip_suffix(".sqlite")?
                .parse()
                .ok()
        })
        .collect();
    versions.sort_unstable();
    versions
}

#[test]
fn current_schema_version_has_a_fixture() {
    assert!(
        fixture_versions().contains(&SCHEMA_VERSION),
        "add tests/fixtures/schema/v{SCHEMA_VERSION}.sqlite with `just schema-fixture`"
    );
}

#[test]
fn every_fixture_opens_and_keeps_its_data() {
    for version in fixture_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        std::fs::copy(fixtures_dir().join(format!("v{version}.sqlite")), &path).unwrap();

        let catalog = Catalog::open(&path).unwrap_or_else(|e| panic!("v{version}: {e}"));
        let location = catalog.location("fixture").unwrap();
        assert_eq!(location.hash_algo, HashAlgo::Blake3, "v{version}");

        let conn = Connection::open(&path).unwrap();
        let user_version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(user_version, SCHEMA_VERSION, "v{version} was not migrated");
        let (entries, hashed): (i64, i64) = conn
            .query_row(
                "SELECT count(*), count(hash_full) FROM entries WHERE location_id = ?1",
                [location.id.get()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((entries, hashed), (3, 1), "v{version} lost entries");

        let backup = dir.path().join(format!("catalog.db.v{version}.bak"));
        assert_eq!(
            backup.exists(),
            version < SCHEMA_VERSION,
            "v{version} backup"
        );
    }
}

/// Writes the fixture for the current schema version. Run once after adding a migration
/// (`just schema-fixture`); never regenerate a released version's fixture.
#[test]
#[ignore = "writes a new fixture file"]
fn write_fixture() {
    let path = fixtures_dir().join(format!("v{SCHEMA_VERSION}.sqlite"));
    assert!(!path.exists(), "{} already exists", path.display());
    Catalog::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "INSERT INTO locations (id, label, root_kind, root_raw, path_encoding, hash_algo, hash_policy, ignore_patterns, created_at)
             VALUES (1, 'fixture', 'local', CAST('/fixture/photos' AS BLOB), 'posix', 'blake3', 'eager', '*.tmp', 1795000000000);
         INSERT INTO scans (id, location_id, started_at, finished_at, status, files, dirs, bytes, hashed_files, hashed_bytes)
             VALUES (1, 1, 1795000000000, 1795000001000, 'completed', 2, 1, 2048, 1, 1024);
         INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, hash_partial, hash_full, seen_scan_id)
             VALUES
             (1, CAST('2024' AS BLOB), '2024', '', 'dir', 0, 1795000000000000000, NULL, NULL, 1),
             (1, CAST('2024/a.jpg' AS BLOB), '2024/a.jpg', '2024', 'file', 1024, 1795000000000000000,
                 zeroblob(32), x'6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85', 1),
             (1, x'322f62ff2e6a7067', '2/b%FF.jpg', '2', 'file', 1024, 1795000000000000000, NULL, NULL, 1);
         INSERT INTO scan_warnings (scan_id, rel_path_raw, kind, message)
             VALUES (1, CAST('2024/locked' AS BLOB), 'permission', 'Permission denied (os error 13)');
         PRAGMA journal_mode = DELETE;
         VACUUM;",
    )
    .unwrap();
}
