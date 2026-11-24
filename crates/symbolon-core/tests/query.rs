//! `Query` against scanned trees, including while a scan is still running.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use symbolon_core::catalog::{Catalog, CatalogError, HashPolicy, NewLocation};
use symbolon_core::pathkey::RelPath;
use symbolon_core::query::{EntryInfo, HashState, Page, PageOf, Query};
use symbolon_core::scan::{ScanOptions, ScanStatus, Scanner};
use symbolon_core::source::{
    LocalSource, ReadSeek, Source, SourceEntry, SourceError, WalkItem, WalkOutcome, WarningKind,
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

    fn write(&self, rel: &str, contents: &[u8]) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn scan(&self) {
        Scanner::start(&self.catalog, "loc", ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
    }

    fn query(&self) -> Query {
        Query::open(&self.catalog).unwrap()
    }
}

fn keys(entries: &[EntryInfo]) -> Vec<String> {
    entries.iter().map(|e| e.key.as_str().to_owned()).collect()
}

/// Follows cursors to the end, asserting every page but the last is full.
fn all_pages(limit: u32, mut fetch: impl FnMut(&Page) -> PageOf<EntryInfo>) -> Vec<String> {
    let mut page = Page::first(limit);
    let mut out = Vec::new();
    loop {
        let result = fetch(&page);
        out.extend(keys(&result.items));
        match result.next {
            Some(cursor) => {
                assert_eq!(result.items.len(), limit as usize);
                page = Page::after(cursor, limit);
            }
            None => return out,
        }
    }
}

#[test]
fn children_are_browsed_page_by_page_in_stable_order() {
    let fx = Fixture::new(HashPolicy::Lazy);
    for i in 0..10 {
        fx.write(&format!("f{i}"), b"x");
    }
    fx.write("dir/inner", b"x");
    fx.scan();
    let q = fx.query();

    let root = all_pages(3, |page| q.children("loc", "", page).unwrap());
    let expected = [
        "dir", "f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9",
    ];
    assert_eq!(root, expected);
    assert_eq!(
        keys(&q.children("loc", "dir", &Page::first(100)).unwrap().items),
        ["dir/inner"]
    );
    assert_eq!(
        q.children("loc", "missing", &Page::first(100))
            .unwrap()
            .items
            .len(),
        0
    );
}

#[test]
fn entries_and_stats_reflect_hash_state() {
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("a.txt", b"alpha");
    fx.write("d/b.txt", b"beta!");
    fx.scan();
    let q = fx.query();

    let stats = q.location_stats("loc").unwrap();
    assert_eq!(
        (stats.files, stats.dirs, stats.bytes, stats.verified_files),
        (2, 1, 10, 2)
    );

    let a = q
        .entry("loc", &RelPath::from_path("a.txt".as_ref()).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!((a.size, a.hash_state), (5, HashState::Verified));
    assert!(a.digest.is_some());
    assert_eq!(
        q.entry("loc", &RelPath::from_path("nope".as_ref()).unwrap())
            .unwrap(),
        None
    );

    let lazy = Fixture::new(HashPolicy::Lazy);
    lazy.write("a.txt", b"alpha");
    lazy.scan();
    let a = lazy
        .query()
        .entry("loc", &RelPath::from_path("a.txt".as_ref()).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!((a.hash_state, a.digest), (HashState::Unhashed, None));
}

#[test]
fn search_matches_substrings_with_literal_wildcards() {
    let fx = Fixture::new(HashPolicy::Lazy);
    for name in [
        "Report_2024.pdf",
        "report-2023.pdf",
        "100%.txt",
        "other.txt",
    ] {
        fx.write(&format!("docs/{name}"), b"x");
    }
    fx.scan();
    let q = fx.query();
    let search = |needle: &str| keys(&q.search("loc", needle, &Page::first(100)).unwrap().items);

    assert_eq!(
        search("report"),
        ["docs/Report_2024.pdf", "docs/report-2023.pdf"]
    );
    assert_eq!(search("t_2"), ["docs/Report_2024.pdf"], "`_` is literal");
    assert_eq!(search("100%"), ["docs/100%25.txt"], "`%` is literal");
    assert_eq!(
        all_pages(1, |page| q.search("loc", ".pdf", page).unwrap()).len(),
        2
    );
}

#[test]
fn duplicates_are_grouped_by_verified_content() {
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("a/photo.jpg", b"same bytes");
    fx.write("b/photo copy.jpg", b"same bytes");
    fx.write("c/again.jpg", b"same bytes");
    fx.write("other", b"pair");
    fx.write("other2", b"pair");
    fx.write("unique", b"unique");
    fx.scan();
    let q = fx.query();

    let mut groups = Vec::new();
    let mut page = Page::first(1);
    loop {
        let result = q.duplicates("loc", &page).unwrap();
        groups.extend(result.items);
        match result.next {
            Some(cursor) => page = Page::after(cursor, 1),
            None => break,
        }
    }
    groups.sort_by_key(|g| g.entries.len());
    assert_eq!(groups.len(), 2);
    assert_eq!(keys(&groups[0].entries), ["other", "other2"]);
    assert_eq!(
        keys(&groups[1].entries),
        ["a/photo.jpg", "b/photo copy.jpg", "c/again.jpg"]
    );
    assert_eq!(groups[1].redundant_bytes(), 2 * "same bytes".len() as u64);
}

/// Fails `open` for one file, producing a scan warning.
struct Failing {
    inner: LocalSource,
}

impl Source for Failing {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        self.inner.walk(visit)
    }
    fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        if path.key().as_str() == "gone" {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        self.inner.open(path)
    }
    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        self.inner.stat(path)
    }
}

#[test]
fn scans_are_listed_newest_first_with_their_warnings() {
    let fx = Fixture::new(HashPolicy::Eager);
    fx.write("ok", b"ok");
    fx.write("gone", b"gone");
    fx.scan();
    let source = Arc::new(Failing {
        inner: LocalSource::new(&fx.root, &[]).unwrap(),
    });
    // The first scan hashed "gone"; touch it so the second scan must hash it again.
    fx.write("gone", b"gone, changed");
    Scanner::start_with_source(&fx.catalog, "loc", source, ScanOptions::default())
        .unwrap()
        .wait()
        .unwrap();

    let q = fx.query();
    let scans = q.scans("loc", &Page::first(10)).unwrap().items;
    assert_eq!(scans.len(), 2);
    assert!(scans[0].id > scans[1].id);
    assert_eq!(scans[0].status, ScanStatus::Completed);
    assert_eq!(scans[0].counts.warnings, 1);

    let warnings = q.warnings(scans[0].id, &Page::first(10)).unwrap().items;
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].kind, WarningKind::Vanished);
    assert_eq!(warnings[0].path.as_ref().unwrap().key().as_str(), "gone");
    assert_eq!(
        q.warnings(scans[1].id, &Page::first(10))
            .unwrap()
            .items
            .len(),
        0
    );
}

#[test]
fn unknown_locations_are_reported() {
    let fx = Fixture::new(HashPolicy::Lazy);
    assert!(matches!(
        fx.query().children("nope", "", &Page::first(10)),
        Err(CatalogError::LocationNotFound(label)) if label == "nope"
    ));
}

/// Blocks every `open` until released, so a scan can be held mid-way.
struct Held {
    inner: LocalSource,
    opened: Sender<()>,
    released: AtomicBool,
}

impl Source for Held {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        self.inner.walk(visit)
    }
    fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        self.opened.send(()).ok();
        while !self.released.load(Ordering::SeqCst) {
            std::thread::park_timeout(Duration::from_millis(10));
        }
        self.inner.open(path)
    }
    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        self.inner.stat(path)
    }
}

#[test]
fn queries_see_partial_results_while_a_scan_runs() {
    let fx = Fixture::new(HashPolicy::Eager);
    for i in 0..300 {
        fx.write(
            &format!("d{}/f{i:03}", i % 7),
            format!("content {i}").as_bytes(),
        );
    }
    let (opened_tx, opened): (Sender<()>, Receiver<()>) = crossbeam_channel::unbounded();
    let source = Arc::new(Held {
        inner: LocalSource::new(&fx.root, &[]).unwrap(),
        opened: opened_tx,
        released: AtomicBool::new(false),
    });
    let options = ScanOptions::default().threads(NonZeroUsize::MIN);
    let handle = Scanner::start_with_source(&fx.catalog, "loc", source.clone(), options).unwrap();
    opened.recv_timeout(TIMEOUT).unwrap();

    // The walk is committed and hashing is held at the first file.
    let q = fx.query();
    let stats = q.location_stats("loc").unwrap();
    assert_eq!((stats.files, stats.dirs, stats.verified_files), (300, 7, 0));
    assert_eq!(
        q.scans("loc", &Page::first(1)).unwrap().items[0].status,
        ScanStatus::Running
    );
    let browsed = all_pages(50, |page| q.children("loc", "d3", page).unwrap());
    assert_eq!(browsed.len(), 43);

    source.released.store(true, Ordering::SeqCst);
    let summary = handle.wait().unwrap();
    assert_eq!(summary.counts.hashed_files, 300);
    assert_eq!(q.location_stats("loc").unwrap().verified_files, 300);
    assert_eq!(
        q.scans("loc", &Page::first(1)).unwrap().items[0].status,
        ScanStatus::Completed
    );
}
