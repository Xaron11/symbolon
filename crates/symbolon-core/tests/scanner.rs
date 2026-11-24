//! `Scanner` end to end: catalog contents, events, warnings, cancel and pause, incremental and
//! resumed scans.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use crossbeam_channel::{Receiver, Sender};
use rusqlite::Connection;
use symbolon_core::catalog::{Catalog, HashPolicy, NewLocation};
use symbolon_core::hash::{HashAlgo, hash_bytes};
use symbolon_core::pathkey::RelPath;
use symbolon_core::scan::{ScanError, ScanEvent, ScanOptions, ScanStatus, ScanSummary, Scanner};
use symbolon_core::source::{
    LocalSource, ReadSeek, Source, SourceEntry, SourceError, WalkItem, WalkOutcome,
};

const TIMEOUT: Duration = Duration::from_secs(10);

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    catalog: Catalog,
}

impl Fixture {
    fn new(policy: HashPolicy) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
        catalog
            .add_location(NewLocation::local("loc", &root).hash_policy(policy))
            .unwrap();
        Self {
            _dir: dir,
            root,
            catalog,
        }
    }

    /// Writes a file with a fixed, distinct mtime so tests never depend on clock resolution.
    fn write(&self, rel: &str, contents: &[u8], mtime_secs: u64) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(mtime_secs))
            .unwrap();
    }

    fn local(&self) -> LocalSource {
        LocalSource::new(&self.root, &[]).unwrap()
    }

    fn scan_with(
        &self,
        source: Arc<dyn Source>,
        options: ScanOptions,
    ) -> (Vec<ScanEvent>, Result<ScanSummary, ScanError>) {
        let handle = Scanner::start_with_source(&self.catalog, "loc", source, options).unwrap();
        let events: Vec<ScanEvent> = handle.events().iter().collect();
        (events, handle.wait())
    }

    fn scan(&self) -> ScanSummary {
        let handle = Scanner::start(&self.catalog, "loc", ScanOptions::default()).unwrap();
        handle.wait().unwrap()
    }

    /// `path_key -> (kind, size, full digest as hex)` for every entry.
    fn entries(&self) -> BTreeMap<String, (String, u64, Option<String>)> {
        let conn = Connection::open(self.catalog.path()).unwrap();
        let mut stmt = conn
            .prepare("SELECT path_key, kind, size, nullif(hex(hash_full), '') FROM entries ORDER BY path_key")
            .unwrap();
        stmt.query_map([], |row| {
            Ok((row.get(0)?, (row.get(1)?, row.get(2)?, row.get(3)?)))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    fn scan_rows(&self) -> Vec<(String, i64)> {
        let conn = Connection::open(self.catalog.path()).unwrap();
        let mut stmt = conn
            .prepare("SELECT status, hashed_files FROM scans ORDER BY id")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }
}

fn blake3_hex(data: &[u8]) -> String {
    hash_bytes(HashAlgo::Blake3, data).to_hex().to_uppercase()
}

fn one_thread() -> ScanOptions {
    ScanOptions::default().threads(NonZeroUsize::MIN)
}

/// Delegates to a `LocalSource`, counting and recording `open` calls.
struct Counting {
    inner: LocalSource,
    opens: AtomicUsize,
    opened: Mutex<Vec<String>>,
}

impl Counting {
    fn new(inner: LocalSource) -> Arc<Self> {
        Arc::new(Self {
            inner,
            opens: AtomicUsize::new(0),
            opened: Mutex::new(Vec::new()),
        })
    }

    fn opened(&self) -> Vec<String> {
        let mut opened = self.opened.lock().unwrap().clone();
        opened.sort();
        opened
    }
}

impl Source for Counting {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        self.inner.walk(visit)
    }
    fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        self.opened
            .lock()
            .unwrap()
            .push(path.key().as_str().to_owned());
        self.inner.open(path)
    }
    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        self.inner.stat(path)
    }
}

/// Reports every `open` on a channel and blocks it until the test lets it go (until released).
struct Gated {
    inner: LocalSource,
    opened: Sender<String>,
    go: Receiver<()>,
    released: AtomicBool,
}

impl Gated {
    fn new(inner: LocalSource) -> (Arc<Self>, Receiver<String>, Sender<()>) {
        let (opened_tx, opened_rx) = crossbeam_channel::unbounded();
        let (go_tx, go_rx) = crossbeam_channel::unbounded();
        let source = Arc::new(Self {
            inner,
            opened: opened_tx,
            go: go_rx,
            released: AtomicBool::new(false),
        });
        (source, opened_rx, go_tx)
    }

    fn release(&self) {
        self.released.store(true, Ordering::SeqCst);
    }
}

impl Source for Gated {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        self.inner.walk(visit)
    }
    fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        self.opened.send(path.key().as_str().to_owned()).unwrap();
        while !self.released.load(Ordering::SeqCst) {
            if self.go.recv_timeout(Duration::from_millis(20)).is_ok() {
                break;
            }
        }
        self.inner.open(path)
    }
    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        self.inner.stat(path)
    }
}

/// Appends to `victim` right before it is opened, as if another program wrote to it.
struct Mutating {
    inner: LocalSource,
    victim: PathBuf,
}

impl Source for Mutating {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        self.inner.walk(visit)
    }
    fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        let native = path.to_native(self.inner.root()).unwrap();
        if native == self.victim {
            fs::OpenOptions::new()
                .append(true)
                .open(&native)?
                .write_all_bytes(b" more")?;
        }
        self.inner.open(path)
    }
    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        self.inner.stat(path)
    }
}

trait WriteAll {
    fn write_all_bytes(self, bytes: &[u8]) -> io::Result<()>;
}

impl WriteAll for fs::File {
    fn write_all_bytes(mut self, bytes: &[u8]) -> io::Result<()> {
        io::Write::write_all(&mut self, bytes)
    }
}

fn many_files(fx: &Fixture, n: u64) {
    for i in 0..n {
        fx.write(
            &format!("f{i:02}.txt"),
            format!("file {i}").as_bytes(),
            1_000 + i,
        );
    }
}

#[test]
fn eager_scan_stores_entries_digests_and_events() {
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("a.txt", b"alpha", 1_000);
    fx.write("dir/b.txt", b"beta", 1_001);
    fx.write("dir/empty", b"", 1_002);

    let (events, summary) = fx.scan_with(Arc::new(fx.local()), ScanOptions::default());
    let summary = summary.unwrap();

    assert_eq!(
        fx.entries(),
        BTreeMap::from([
            (
                "a.txt".into(),
                ("file".into(), 5, Some(blake3_hex(b"alpha")))
            ),
            ("dir".into(), ("dir".into(), 0, None)),
            (
                "dir/b.txt".into(),
                ("file".into(), 4, Some(blake3_hex(b"beta")))
            ),
            (
                "dir/empty".into(),
                ("file".into(), 0, Some(blake3_hex(b"")))
            ),
        ])
    );
    assert_eq!(summary.status, ScanStatus::Completed);
    let c = summary.counts;
    assert_eq!(
        (
            c.files,
            c.dirs,
            c.bytes,
            c.hashed_files,
            c.hashed_bytes,
            c.reused_hashes
        ),
        (3, 1, 9, 3, 9, 0)
    );

    assert!(
        matches!(events.first(), Some(ScanEvent::Started { location, .. }) if location == "loc")
    );
    assert!(matches!(events.last(), Some(ScanEvent::Finished(s)) if *s == summary));
    assert!(events.iter().any(|e| matches!(e, ScanEvent::Progress(_))));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, ScanEvent::FileDiscovered { .. }))
    );
    assert_eq!(fx.scan_rows(), [("completed".into(), 3)]);
}

#[test]
fn lazy_scans_hash_nothing_unless_asked() {
    let fx = Fixture::new(HashPolicy::Lazy);
    fx.write("a.txt", b"alpha", 1_000);
    let summary = fx.scan();
    assert_eq!(summary.counts.hashed_files, 0);
    assert_eq!(fx.entries()["a.txt"].2, None);

    let (_, summary) = fx.scan_with(Arc::new(fx.local()), ScanOptions::default().hash_all(true));
    assert_eq!(summary.unwrap().counts.hashed_files, 1);
    assert_eq!(fx.entries()["a.txt"].2, Some(blake3_hex(b"alpha")));
}

#[test]
fn per_file_events_when_requested() {
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("a", b"1", 1_000);
    fx.write("d/b", b"2", 1_001);
    let (events, _) = fx.scan_with(
        Arc::new(fx.local()),
        ScanOptions::default().file_events(true),
    );
    let discovered = events
        .iter()
        .filter(|e| matches!(e, ScanEvent::FileDiscovered { .. }))
        .count();
    let hashed = events
        .iter()
        .filter(|e| matches!(e, ScanEvent::FileHashed { .. }))
        .count();
    assert_eq!((discovered, hashed), (3, 2));
}

#[test]
fn rescans_read_only_new_and_changed_files() {
    let fx = Fixture::new(HashPolicy::Eager);
    many_files(&fx, 10);

    let first = Counting::new(fx.local());
    fx.scan_with(first.clone(), ScanOptions::default())
        .1
        .unwrap();
    assert_eq!(first.opens.load(Ordering::SeqCst), 10);

    let second = Counting::new(fx.local());
    let summary = fx
        .scan_with(second.clone(), ScanOptions::default())
        .1
        .unwrap();
    assert_eq!(
        second.opens.load(Ordering::SeqCst),
        0,
        "unchanged files must not be read"
    );
    assert_eq!(
        (summary.counts.hashed_files, summary.counts.reused_hashes),
        (0, 10)
    );

    fx.write("f03.txt", b"changed content", 5_000);
    fx.write("f07.txt", b"file 7", 5_001);
    fx.write("new.txt", b"new", 5_002);
    fs::remove_file(fx.root.join("f09.txt")).unwrap();
    let third = Counting::new(fx.local());
    let summary = fx
        .scan_with(third.clone(), ScanOptions::default())
        .1
        .unwrap();
    assert_eq!(third.opened(), ["f03.txt", "f07.txt", "new.txt"]);
    assert_eq!(
        (summary.counts.hashed_files, summary.counts.reused_hashes),
        (3, 7)
    );

    let entries = fx.entries();
    assert!(
        !entries.contains_key("f09.txt"),
        "deleted files leave the catalog"
    );
    assert_eq!(entries["f03.txt"].2, Some(blake3_hex(b"changed content")));
    assert_eq!(entries.len(), 10);
}

#[test]
fn mtime_tolerance_keeps_hashes_of_jittering_timestamps() {
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("a", b"same", 1_000);
    fx.scan();

    fx.write("a", b"same", 1_002);
    let tolerant = Counting::new(fx.local());
    let options = ScanOptions::default().mtime_tolerance(Duration::from_secs(2));
    fx.scan_with(tolerant.clone(), options).1.unwrap();
    assert_eq!(
        tolerant.opens.load(Ordering::SeqCst),
        0,
        "within tolerance: hash kept"
    );

    fx.write("a", b"same", 1_010);
    let exact = Counting::new(fx.local());
    fx.scan_with(exact.clone(), ScanOptions::default())
        .1
        .unwrap();
    assert_eq!(
        exact.opens.load(Ordering::SeqCst),
        1,
        "beyond tolerance: rehashed"
    );
}

#[test]
fn files_changing_while_hashed_become_warnings_without_digests() {
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("stable", b"stable", 1_000);
    fx.write("growing", b"growing", 1_001);
    let source = Arc::new(Mutating {
        inner: fx.local(),
        victim: fs::canonicalize(&fx.root).unwrap().join("growing"),
    });
    let (events, summary) = fx.scan_with(source, ScanOptions::default());
    assert_eq!(summary.unwrap().counts.warnings, 1);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ScanEvent::Warning(w) if w.kind.name() == "changed"))
    );
    let entries = fx.entries();
    assert_eq!(entries["growing"].2, None);
    assert_eq!(entries["stable"].2, Some(blake3_hex(b"stable")));

    let conn = Connection::open(fx.catalog.path()).unwrap();
    let stored: String = conn
        .query_row("SELECT kind FROM scan_warnings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(stored, "changed");
}

#[test]
fn cancelling_keeps_committed_work_and_marks_the_scan() {
    let fx = Fixture::new(HashPolicy::Eager);
    many_files(&fx, 10);
    let (source, opened, go) = Gated::new(fx.local());
    let handle =
        Scanner::start_with_source(&fx.catalog, "loc", source.clone(), one_thread()).unwrap();
    opened.recv_timeout(TIMEOUT).unwrap();
    handle.cancel();
    go.send(()).unwrap();
    let summary = handle.wait().unwrap();
    source.release();

    assert_eq!(summary.status, ScanStatus::Cancelled);
    assert!(summary.counts.hashed_files <= 1);
    assert_eq!(
        fx.entries().len(),
        10,
        "the walk had finished; all entries stay"
    );
    assert_eq!(fx.scan_rows()[0].0, "cancelled");
}

#[test]
fn pausing_stops_work_until_resumed() {
    let fx = Fixture::new(HashPolicy::Eager);
    many_files(&fx, 5);
    let (source, opened, go) = Gated::new(fx.local());
    let handle =
        Scanner::start_with_source(&fx.catalog, "loc", source.clone(), one_thread()).unwrap();
    opened.recv_timeout(TIMEOUT).unwrap();
    handle.pause();
    go.send(()).unwrap();
    assert!(
        opened.recv_timeout(Duration::from_millis(300)).is_err(),
        "no file may be opened while paused"
    );
    handle.resume();
    opened.recv_timeout(TIMEOUT).unwrap();
    source.release();
    let summary = handle.wait().unwrap();
    assert_eq!(
        (summary.status, summary.counts.hashed_files),
        (ScanStatus::Completed, 5)
    );
}

#[test]
fn a_crashed_scan_is_resumed_by_the_next_one() {
    let fx = Fixture::new(HashPolicy::Eager);
    many_files(&fx, 10);

    // Hash a few files, then stop and leave the scan `running`, as a crash would.
    let (source, opened, go) = Gated::new(fx.local());
    let handle =
        Scanner::start_with_source(&fx.catalog, "loc", source.clone(), one_thread()).unwrap();
    for _ in 0..3 {
        opened.recv_timeout(TIMEOUT).unwrap();
        go.send(()).unwrap();
    }
    opened.recv_timeout(TIMEOUT).unwrap();
    handle.cancel();
    go.send(()).unwrap();
    let interrupted = handle.wait().unwrap();
    source.release();
    let done_before = interrupted.counts.hashed_files;
    assert!((3..=4).contains(&done_before));
    Connection::open(fx.catalog.path())
        .unwrap()
        .execute(
            "UPDATE scans SET status = 'running', finished_at = NULL",
            [],
        )
        .unwrap();

    let resumed = Counting::new(fx.local());
    let summary = fx
        .scan_with(resumed.clone(), ScanOptions::default())
        .1
        .unwrap();
    let reopened = u64::try_from(resumed.opens.load(Ordering::SeqCst)).unwrap();
    assert_eq!(
        reopened,
        10 - done_before,
        "already hashed files are not read again"
    );
    assert_eq!(summary.status, ScanStatus::Completed);
    assert_eq!(fx.scan_rows()[0].0, "interrupted");

    // The result equals a clean scan of the same tree.
    let clean = Fixture::new(HashPolicy::Eager);
    many_files(&clean, 10);
    clean.scan();
    assert_eq!(fx.entries(), clean.entries());
}

#[cfg(unix)]
#[test]
fn unreadable_files_become_warnings_and_the_scan_completes() {
    use std::os::unix::fs::PermissionsExt as _;
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("ok", b"ok", 1_000);
    fx.write("secret", b"secret", 1_001);
    let secret = fx.root.join("secret");
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::File::open(&secret).is_ok() {
        return; // running as root
    }
    let summary = fx.scan();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        (
            summary.status,
            summary.counts.hashed_files,
            summary.counts.warnings
        ),
        (ScanStatus::Completed, 1, 1)
    );
    assert_eq!(fx.entries()["secret"].2, None);
}

#[test]
fn an_unavailable_root_fails_the_scan() {
    let fx = Fixture::new(HashPolicy::Eager);
    fs::remove_dir(&fx.root).unwrap();
    let result = Scanner::start(&fx.catalog, "loc", ScanOptions::default())
        .unwrap()
        .wait();
    assert!(matches!(
        result,
        Err(ScanError::Source(SourceError::RootUnavailable { .. }))
    ));
    assert_eq!(fx.scan_rows()[0].0, "failed");
}

#[test]
fn unknown_locations_cannot_be_scanned() {
    let fx = Fixture::new(HashPolicy::Eager);
    assert!(matches!(
        Scanner::start(&fx.catalog, "nope", ScanOptions::default()),
        Err(ScanError::Catalog(_))
    ));
}
