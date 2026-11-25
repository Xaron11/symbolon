//! Export and import of locations (ADR-0005, ADR-0013).

#![cfg(feature = "local")]
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly by design"
)]

use std::fs;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;
use symbolon_core::catalog::{
    Catalog, CatalogError, HashPolicy, ImportAction, ImportConflict, ImportOptions, LocationRoot,
    NewLocation, SCHEMA_VERSION, TransferError,
};
use symbolon_core::compare::{Category, CompareOptions, Comparison, Side};
use symbolon_core::pathkey::{PathEncoding, RelPath};
use symbolon_core::scan::{ScanError, ScanOptions, Scanner};
use symbolon_core::source::{ReadSeek, Source, SourceEntry, SourceError, WalkItem, WalkOutcome};

struct Machine {
    dir: tempfile::TempDir,
    catalog: Catalog,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
        Self { dir, catalog }
    }

    /// Adds and scans a location with `files`.
    fn location(&self, label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = self.dir.path().join(label);
        fs::create_dir_all(&root).unwrap();
        for (path, contents) in files {
            fs::write(root.join(path), contents).unwrap();
        }
        self.catalog
            .add_location(NewLocation::local(label, &root).hash_policy(HashPolicy::Eager))
            .unwrap();
        self.scan(label);
        root
    }

    fn scan(&self, label: &str) {
        Scanner::start(&self.catalog, label, ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
    }

    fn export(&self, label: &str) -> PathBuf {
        let path = self.dir.path().join(format!("{label}.symbolon"));
        self.catalog.export(label, &path).unwrap();
        path
    }

    fn set_fingerprint(&self, label: &str, fingerprint: &str) {
        Connection::open(self.catalog.path())
            .unwrap()
            .execute(
                "UPDATE locations SET fingerprint = ?2 WHERE label = ?1",
                [label, fingerprint],
            )
            .unwrap();
    }

    /// `(path_key, size, full digest)` of the location's entries.
    fn entries(&self, label: &str) -> Vec<(String, i64, Option<Vec<u8>>)> {
        let conn = Connection::open(self.catalog.path()).unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT path_key, size, hash_full FROM entries JOIN locations l ON l.id = location_id \
                 WHERE l.label = ?1 ORDER BY path_key",
            )
            .unwrap();
        stmt.query_map([label], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }
}

const FILES: &[(&str, &str)] = &[("a.jpg", "aaaa"), ("b.jpg", "bb")];

fn conflict(result: Result<impl std::fmt::Debug, TransferError>) -> ImportConflict {
    match result {
        Err(TransferError::Conflict(conflict)) => conflict,
        other => panic!("expected a conflict, got {other:?}"),
    }
}

#[test]
fn export_and_import_round_trip() {
    let laptop = Machine::new();
    let root = laptop.location("photos", FILES);
    let file = laptop.export("photos");
    assert!(!file.with_extension("symbolon.partial").exists());

    let desktop = Machine::new();
    let summary = desktop
        .catalog
        .import(&file, &ImportOptions::default())
        .unwrap();
    assert_eq!(
        (summary.label.as_str(), summary.action, summary.entries),
        ("photos", ImportAction::Added, 2)
    );
    assert_eq!(desktop.entries("photos"), laptop.entries("photos"));

    let imported = desktop.catalog.location("photos").unwrap();
    assert!(
        matches!(imported.root, LocationRoot::Imported { .. }),
        "{:?}",
        imported.root
    );
    assert_eq!(
        imported.root.display(),
        LocationRoot::Local(fs::canonicalize(&root).unwrap()).display()
    );
    assert!(matches!(
        Scanner::start(&desktop.catalog, "photos", ScanOptions::default()),
        Err(ScanError::NotScannable(_))
    ));

    // The import compares like any location: here against a local copy of the same files.
    desktop.location("copy", FILES);
    let cmp = Comparison::new(
        &desktop.catalog,
        &[Side::location("photos"), Side::location("copy")],
        CompareOptions::default(),
    )
    .unwrap();
    let summary = cmp.summary().unwrap();
    assert_eq!(summary.get(Category::Identical).rows, 2);
    assert_eq!(summary.files(), 4);
}

#[test]
fn the_same_location_is_replaced_by_newer_scans_only() {
    let laptop = Machine::new();
    let root = laptop.location("photos", FILES);
    let old = laptop.export("photos");
    let old_copy = laptop.dir.path().join("old.symbolon");
    fs::copy(&old, &old_copy).unwrap();
    fs::write(root.join("c.jpg"), "new").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    laptop.scan("photos");
    let new = laptop.export("photos");

    let desktop = Machine::new();
    desktop
        .catalog
        .import(&old_copy, &ImportOptions::default())
        .unwrap();
    let replaced = desktop
        .catalog
        .import(&new, &ImportOptions::default())
        .unwrap();
    assert_eq!(
        (replaced.action, replaced.entries),
        (ImportAction::Replaced, 3)
    );
    assert_eq!(desktop.entries("photos"), laptop.entries("photos"));

    // Importing the same file again is fine; an older one needs `force`.
    desktop
        .catalog
        .import(&new, &ImportOptions::default())
        .unwrap();
    assert!(matches!(
        conflict(desktop.catalog.import(&old_copy, &ImportOptions::default())),
        ImportConflict::OlderScan { .. }
    ));
    assert_eq!(desktop.entries("photos").len(), 3, "nothing changed");
    let forced = desktop
        .catalog
        .import(&old_copy, &ImportOptions::default().force(true))
        .unwrap();
    assert_eq!(forced.entries, 2);
}

#[test]
fn a_label_of_another_device_needs_a_choice() {
    let laptop = Machine::new();
    laptop.location("photos", FILES);
    laptop.set_fingerprint("photos", "disk-1");
    let file = laptop.export("photos");

    let desktop = Machine::new();
    desktop.location("photos", &[("other.txt", "x")]);
    desktop.set_fingerprint("photos", "disk-2");
    assert_eq!(
        conflict(desktop.catalog.import(&file, &ImportOptions::default())),
        ImportConflict::LabelTaken {
            label: "photos".into()
        }
    );

    let renamed = desktop
        .catalog
        .import(&file, &ImportOptions::default().as_label("laptop-photos"))
        .unwrap();
    assert_eq!(
        (renamed.label.as_str(), renamed.action),
        ("laptop-photos", ImportAction::Added)
    );
    assert_eq!(desktop.entries("photos").len(), 1, "untouched");

    let replaced = desktop
        .catalog
        .import(&file, &ImportOptions::default().replace(true))
        .unwrap();
    assert_eq!(replaced.action, ImportAction::Replaced);
    assert_eq!(desktop.entries("photos"), laptop.entries("photos"));
    assert_eq!(
        desktop
            .catalog
            .location("photos")
            .unwrap()
            .fingerprint
            .as_deref(),
        Some("disk-1")
    );
}

#[test]
fn a_device_saved_under_another_label_needs_a_choice() {
    // The desktop saw the disk first; the laptop's scan is newer.
    let desktop = Machine::new();
    desktop.location("usb", &[("old.txt", "x")]);
    desktop.set_fingerprint("usb", "disk-1");
    std::thread::sleep(std::time::Duration::from_millis(5));

    let laptop = Machine::new();
    laptop.location("photos", FILES);
    laptop.set_fingerprint("photos", "disk-1");
    let file = laptop.export("photos");
    assert_eq!(
        conflict(desktop.catalog.import(&file, &ImportOptions::default())),
        ImportConflict::SameDevice {
            label: "photos".into(),
            existing: "usb".into()
        }
    );

    let merged = desktop
        .catalog
        .import(&file, &ImportOptions::default().merge_into("usb"))
        .unwrap();
    assert_eq!(
        (merged.label.as_str(), merged.action),
        ("usb", ImportAction::Replaced)
    );
    assert_eq!(desktop.entries("usb"), laptop.entries("photos"));

    let separate = desktop
        .catalog
        .import(&file, &ImportOptions::default().as_label("photos"))
        .unwrap();
    assert_eq!(separate.action, ImportAction::Added);
}

/// Neither side has a fingerprint before M4, so "both absent" says nothing about whether a
/// scanned local location and the file are the same device (#81).
#[test]
fn a_scanned_local_location_is_only_replaced_by_choice() {
    let here = Machine::new();
    here.location("photos", &[("a.jpg", "aaaa"), ("b.jpg", "bbbb")]);
    let before = here.entries("photos");
    std::thread::sleep(std::time::Duration::from_millis(5));

    // Another computer's newer scan under the same label claims both files are equal.
    let other = Machine::new();
    other.location("photos", &[("a.jpg", "aaaa"), ("b.jpg", "aaaa")]);
    let file = other.export("photos");

    assert_eq!(
        conflict(here.catalog.import(&file, &ImportOptions::default())),
        ImportConflict::LocalData {
            label: "photos".into()
        }
    );
    assert_eq!(here.entries("photos"), before, "untouched");

    // Replacing on purpose turns it into an imported location, so a later scan cannot keep
    // the file's digests for the local files.
    let replaced = here
        .catalog
        .import(&file, &ImportOptions::default().replace(true))
        .unwrap();
    assert_eq!(replaced.action, ImportAction::Replaced);
    assert_eq!(here.entries("photos"), other.entries("photos"));
    assert!(matches!(
        here.catalog.location("photos").unwrap().root,
        LocationRoot::Imported { .. }
    ));
    assert!(matches!(
        Scanner::start(&here.catalog, "photos", ScanOptions::default()),
        Err(ScanError::NotScannable(_))
    ));
}

#[test]
fn merging_into_a_local_location_makes_it_imported() {
    let here = Machine::new();
    here.location("usb", &[("old.txt", "x")]);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let other = Machine::new();
    other.location("photos", FILES);
    let file = other.export("photos");

    here.catalog
        .import(&file, &ImportOptions::default().merge_into("usb"))
        .unwrap();
    assert_eq!(here.entries("usb"), other.entries("photos"));
    assert!(matches!(
        Scanner::start(&here.catalog, "usb", ScanOptions::default()),
        Err(ScanError::NotScannable(_))
    ));
}

#[test]
fn other_files_are_refused() {
    let machine = Machine::new();
    machine.location("photos", FILES);
    let text = machine.dir.path().join("notes.symbolon");
    fs::write(&text, "not a database").unwrap();
    assert!(matches!(
        machine.catalog.import(&text, &ImportOptions::default()),
        Err(TransferError::NotAnExport(_))
    ));
    assert!(
        matches!(
            machine
                .catalog
                .import(machine.catalog.path(), &ImportOptions::default()),
            Err(TransferError::NotAnExport(_))
        ),
        "a whole catalog is not an export"
    );

    let newer = machine.export("photos");
    Connection::open(&newer)
        .unwrap()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    assert!(matches!(
        machine.catalog.import(&newer, &ImportOptions::default()),
        Err(TransferError::Catalog(CatalogError::TooNew { .. }))
    ));
}

#[test]
fn exports_of_an_older_schema_are_migrated_in_a_copy() {
    // A v1 export: the v1 catalog fixture marked as an export.
    let machine = Machine::new();
    let file = machine.dir.path().join("v1.symbolon");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema/v1.sqlite"),
        &file,
    )
    .unwrap();
    let conn = Connection::open(&file).unwrap();
    conn.execute_batch(
        "UPDATE meta SET value = 'export' WHERE key = 'kind';
         INSERT INTO meta (key, value) VALUES ('label', 'fixture'), ('hash_algo', 'blake3'),
             ('path_encoding', 'posix'), ('exported_at', '1795000000000'), ('exported_by', '0.2.0');
         INSERT INTO scans (location_id, started_at, finished_at, status) VALUES (1, 1795000000000, 1795000001000, 'completed');",
    )
    .unwrap();
    drop(conn);
    let before = fs::read(&file).unwrap();

    let summary = machine
        .catalog
        .import(&file, &ImportOptions::default())
        .unwrap();
    assert_eq!((summary.label.as_str(), summary.entries), ("fixture", 3));
    assert_eq!(
        fs::read(&file).unwrap(),
        before,
        "the file is never modified"
    );
    let leftovers: Vec<_> = fs::read_dir(machine.dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|name| name.to_string_lossy().starts_with("v1.symbolon."))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn failed_exports_leave_nothing_behind() {
    let machine = Machine::new();
    let target = machine.dir.path().join("out.symbolon");
    assert!(matches!(
        machine.catalog.export("nope", &target),
        Err(TransferError::Catalog(CatalogError::LocationNotFound(_)))
    ));
    let root = machine.dir.path().join("never");
    fs::create_dir(&root).unwrap();
    machine
        .catalog
        .add_location(NewLocation::local("never", &root))
        .unwrap();
    assert!(matches!(
        machine.catalog.export("never", &target),
        Err(TransferError::NeverScanned(_))
    ));
    let names: Vec<_> = fs::read_dir(machine.dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("out"))
        .collect();
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn paths_of_another_encoding_are_not_merged() {
    let laptop = Machine::new();
    laptop.location("photos", FILES);
    let file = laptop.export("photos");
    let other = match PathEncoding::native() {
        PathEncoding::Posix => "wtf8",
        PathEncoding::Wtf8 => "posix",
    };
    let conn = Connection::open(&file).unwrap();
    conn.execute("UPDATE locations SET path_encoding = ?1", [other])
        .unwrap();
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'path_encoding'",
        [other],
    )
    .unwrap();
    drop(conn);
    let desktop = Machine::new();
    desktop.location("photos", FILES);
    assert!(matches!(
        desktop
            .catalog
            .import(&file, &ImportOptions::default().replace(true)),
        Err(TransferError::EncodingMismatch { .. })
    ));
}

/// A walk that never ends, so its scan holds the location's lock.
struct Stuck;

impl Source for Stuck {
    fn walk(
        &self,
        _visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        loop {
            std::thread::park();
        }
    }
    fn open(&self, _path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn stat(&self, _path: &RelPath) -> io::Result<SourceEntry> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[test]
fn a_location_being_scanned_is_not_replaced() {
    let laptop = Machine::new();
    laptop.location("photos", FILES);
    let file = laptop.export("photos");
    let desktop = Machine::new();
    desktop.location("photos", FILES);
    let _running = Scanner::start_with_source(
        &desktop.catalog,
        "photos",
        Arc::new(Stuck),
        ScanOptions::default(),
    )
    .unwrap();
    assert!(matches!(
        desktop
            .catalog
            .import(&file, &ImportOptions::default().replace(true)),
        Err(TransferError::Busy(_))
    ));
}
