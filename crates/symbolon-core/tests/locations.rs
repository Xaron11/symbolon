//! Device fingerprints and finding moved locations (ADR-0016), with a fake mount table: a
//! "pendrive" is a directory that the fake mounts at one place or another.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use symbolon_core::catalog::{Catalog, HashPolicy, NewLocation, Whereabouts};
use symbolon_core::compare::{CompareOptions, Comparison, Side};
use symbolon_core::identity::{Mount, Volumes};
use symbolon_core::scan::{ScanError, ScanOptions, Scanner};

/// A mount table the test changes as it "plugs" devices in.
#[derive(Default)]
struct FakeMounts(Mutex<Vec<Mount>>);

impl FakeMounts {
    fn set(&self, mounts: &[(Option<&str>, &Path)]) {
        *self.0.lock().unwrap() = mounts
            .iter()
            .map(|(uuid, at)| Mount {
                uuid: uuid.map(str::to_owned),
                root: b"/".to_vec(),
                mount_point: fs::canonicalize(at).unwrap(),
            })
            .collect();
    }
}

impl Volumes for FakeMounts {
    fn mounts(&self) -> io::Result<Vec<Mount>> {
        Ok(self.0.lock().unwrap().clone())
    }
}

struct Machine {
    dir: tempfile::TempDir,
    mounts: Arc<FakeMounts>,
    catalog: Catalog,
}

impl Machine {
    /// Everything lives under `system` (a filesystem with UUID `disk`) until a test mounts
    /// something else.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mounts = Arc::new(FakeMounts::default());
        let catalog = Catalog::open(dir.path().join("catalog.db"))
            .unwrap()
            .with_volumes(Arc::clone(&mounts) as Arc<dyn Volumes>);
        let machine = Self {
            dir,
            mounts,
            catalog,
        };
        machine.mounts.set(&[(Some("disk"), machine.dir.path())]);
        machine
    }

    fn path(&self, rel: &str) -> PathBuf {
        let path = self.dir.path().join(rel);
        fs::create_dir_all(&path).unwrap();
        fs::canonicalize(path).unwrap()
    }

    /// A pendrive `uuid` with `files`, mounted at `at`.
    fn plug(&self, uuid: &str, at: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = self.path(at);
        for (name, contents) in files {
            fs::write(root.join(name), contents).unwrap();
        }
        self.mounts
            .set(&[(Some("disk"), self.dir.path()), (Some(uuid), &root)]);
        root
    }

    fn scan(&self, label: &str) -> Result<(), ScanError> {
        Scanner::start(&self.catalog, label, ScanOptions::default())?
            .wait()
            .map(drop)
    }

    fn files(&self, label: &str) -> i64 {
        rusqlite::Connection::open(self.catalog.path())
            .unwrap()
            .query_row(
                "SELECT count(*) FROM entries e JOIN locations l ON l.id = e.location_id \
                 WHERE l.label = ?1 AND e.kind = 'file'",
                [label],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn fingerprint(&self, label: &str) -> Option<String> {
        self.catalog.location(label).unwrap().fingerprint
    }
}

#[test]
fn adding_records_the_fingerprint_of_the_root() {
    let m = Machine::new();
    let usb = m.plug("usb-1", "media/usb", &[("a.jpg", "a")]);
    fs::create_dir(usb.join("DCIM")).unwrap();
    m.catalog
        .add_location(NewLocation::local("photos", usb.join("DCIM")))
        .unwrap();
    assert_eq!(m.fingerprint("photos").as_deref(), Some("fs:usb-1:/DCIM"));

    // A filesystem without a UUID gives none.
    let tmp = m.path("tmp");
    m.mounts.set(&[(Some("disk"), m.dir.path()), (None, &tmp)]);
    m.catalog
        .add_location(NewLocation::local("scratch", &tmp))
        .unwrap();
    assert_eq!(m.fingerprint("scratch"), None);
    assert_eq!(m.catalog.locate("scratch").unwrap(), Whereabouts::Here(tmp));
}

#[test]
fn every_row_of_the_whereabouts_table() {
    let m = Machine::new();
    let first = m.plug("usb-1", "media/a", &[("a.jpg", "a")]);
    m.catalog
        .add_location(NewLocation::local("photos", &first))
        .unwrap();
    assert_eq!(
        m.catalog.locate("photos").unwrap(),
        Whereabouts::Here(first.clone())
    );

    // Replugged elsewhere; the old mount point is gone.
    fs::remove_file(first.join("a.jpg")).unwrap();
    fs::remove_dir(&first).unwrap();
    let second = m.plug("usb-1", "media/b", &[]);
    assert_eq!(
        m.catalog.locate("photos").unwrap(),
        Whereabouts::Moved {
            from: first.clone(),
            to: second.clone()
        }
    );

    // Two copies of the same filesystem are plugged in.
    let third = m.path("media/c");
    m.mounts.set(&[
        (Some("disk"), m.dir.path()),
        (Some("usb-1"), &second),
        (Some("usb-1"), &third),
    ]);
    assert_eq!(
        m.catalog.locate("photos").unwrap(),
        Whereabouts::Ambiguous {
            root: first.clone(),
            candidates: vec![second, third]
        }
    );

    // Unplugged: the stored root is missing and the device is nowhere.
    m.mounts.set(&[(Some("disk"), m.dir.path())]);
    assert_eq!(
        m.catalog.locate("photos").unwrap(),
        Whereabouts::Missing(first.clone())
    );

    // Unplugged, but the empty mount point is still there: it is on the system disk.
    fs::create_dir(&first).unwrap();
    match m.catalog.locate("photos").unwrap() {
        Whereabouts::OtherDevice {
            root,
            expected,
            found,
        } => {
            assert_eq!(root, first);
            assert_eq!(expected.as_str(), "fs:usb-1:/");
            assert!(found.unwrap().as_str().starts_with("fs:disk:/"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_empty_mount_point_is_not_scanned_as_the_location() {
    let m = Machine::new();
    let usb = m.plug("usb-1", "media/usb", &[("a.jpg", "a"), ("b.jpg", "b")]);
    m.catalog
        .add_location(NewLocation::local("photos", &usb))
        .unwrap();
    m.scan("photos").unwrap();
    assert_eq!(m.files("photos"), 2);

    // The pendrive is unplugged; its mount point stays behind, empty.
    for name in ["a.jpg", "b.jpg"] {
        fs::remove_file(usb.join(name)).unwrap();
    }
    m.mounts.set(&[(Some("disk"), m.dir.path())]);
    assert!(matches!(
        m.scan("photos"),
        Err(ScanError::OtherDevice { .. })
    ));
    assert_eq!(m.files("photos"), 2, "nothing recorded as deleted");
}

#[test]
fn a_moved_location_is_scanned_after_relocating() {
    let m = Machine::new();
    let first = m.plug("usb-1", "media/a", &[("a.jpg", "a")]);
    m.catalog
        .add_location(NewLocation::local("photos", &first))
        .unwrap();
    m.scan("photos").unwrap();

    fs::remove_file(first.join("a.jpg")).unwrap();
    fs::remove_dir(&first).unwrap();
    let second = m.plug("usb-1", "media/b", &[("a.jpg", "a"), ("new.jpg", "n")]);
    match m.scan("photos") {
        Err(ScanError::Moved { from, to, .. }) => assert_eq!((from, to), (first, second.clone())),
        other => panic!("{other:?}"),
    }
    m.catalog.relocate("photos", &second).unwrap();
    assert_eq!(
        m.catalog.locate("photos").unwrap(),
        Whereabouts::Here(second)
    );
    m.scan("photos").unwrap();
    assert_eq!(m.files("photos"), 2);
}

#[test]
fn relocating_accepts_another_device_at_the_root() {
    let m = Machine::new();
    let usb = m.plug("usb-1", "media/usb", &[("a.jpg", "a")]);
    m.catalog
        .add_location(NewLocation::local("photos", &usb))
        .unwrap();
    m.plug("usb-2", "media/usb", &[]);
    assert!(matches!(
        m.catalog.locate("photos").unwrap(),
        Whereabouts::OtherDevice { .. }
    ));
    let location = m.catalog.relocate("photos", &usb).unwrap();
    assert_eq!(location.fingerprint.as_deref(), Some("fs:usb-2:/"));
    m.scan("photos").unwrap();
}

#[test]
fn a_scan_records_a_missing_fingerprint_but_never_replaces_one() {
    let m = Machine::new();
    let usb = m.path("media/usb");
    m.mounts.set(&[(None, &usb)]);
    m.catalog
        .add_location(NewLocation::local("photos", &usb))
        .unwrap();
    assert_eq!(m.fingerprint("photos"), None);

    m.mounts.set(&[(Some("usb-1"), &usb)]);
    m.scan("photos").unwrap();
    assert_eq!(m.fingerprint("photos").as_deref(), Some("fs:usb-1:/"));

    m.mounts.set(&[(Some("usb-2"), &usb)]);
    assert!(matches!(
        m.scan("photos"),
        Err(ScanError::OtherDevice { .. })
    ));
    assert_eq!(m.fingerprint("photos").as_deref(), Some("fs:usb-1:/"));
}

#[test]
fn comparisons_never_hash_a_location_whose_device_is_elsewhere() {
    let m = Machine::new();
    let usb = m.plug("usb-1", "media/usb", &[("same.txt", "aaaa")]);
    let local = m.path("local");
    fs::write(local.join("same.txt"), "aaaa").unwrap();
    for (label, root) in [("usb", &usb), ("local", &local)] {
        m.catalog
            .add_location(NewLocation::local(label, root).hash_policy(HashPolicy::Lazy))
            .unwrap();
        m.scan(label).unwrap();
    }

    // Another device now sits at the pendrive's mount point.
    m.plug("usb-2", "media/usb", &[]);
    let mut cmp = Comparison::new(
        &m.catalog,
        &[Side::location("usb"), Side::location("local")],
        CompareOptions::default(),
    )
    .unwrap();
    let summary = cmp
        .resolve(&ScanOptions::default(), |_| ControlFlow::Continue(()))
        .unwrap();
    assert_eq!(summary.unreachable, ["usb"]);
    let hashed: Option<Vec<u8>> = rusqlite::Connection::open(m.catalog.path())
        .unwrap()
        .query_row(
            "SELECT e.hash_full FROM entries e JOIN locations l ON l.id = e.location_id \
             WHERE l.label = 'usb' AND e.kind = 'file'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hashed, None);
}
