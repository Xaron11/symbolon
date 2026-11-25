//! Hashing on the source's side (ADR-0017): scans hand their jobs to a [`RemoteHasher`] and
//! never open files themselves. The "remote" here is a local directory hashed through
//! `hash_requests`, which is what the Android agent runs on the phone.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use symbolon_core::catalog::{Catalog, HashPolicy, NewLocation};
use symbolon_core::hash::{HashAlgo, hash_reader, partial_hash};
use symbolon_core::pathkey::{PathEncoding, RelPath};
use symbolon_core::scan::{ScanOptions, Scanner, hash_requests};
use symbolon_core::source::{
    HashReply, HashRequest, LocalSource, ReadSeek, RemoteHasher, Source, SourceEntry, SourceError,
    WalkItem, WalkOutcome,
};

/// A source whose files can only be hashed where they are.
struct Remote {
    inner: LocalSource,
    algos: Vec<HashAlgo>,
    partial: bool,
    /// Fail after this many replies, like an unplugged phone.
    fail_after: Option<usize>,
    opens: AtomicUsize,
}

impl Remote {
    fn new(root: &Path) -> Self {
        Self {
            inner: LocalSource::new(root, &[]).unwrap(),
            algos: HashAlgo::ALL.to_vec(),
            partial: true,
            fail_after: None,
            opens: AtomicUsize::new(0),
        }
    }
}

impl Source for Remote {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        self.inner.walk(visit)
    }

    fn open(&self, _path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::other("contents never leave the remote"))
    }

    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        self.inner.stat(path)
    }

    fn remote_hasher(&self) -> Option<&dyn RemoteHasher> {
        Some(self)
    }
}

impl RemoteHasher for Remote {
    fn supports(&self, algo: HashAlgo) -> bool {
        self.algos.contains(&algo)
    }

    fn partial_digests(&self) -> bool {
        self.partial
    }

    fn hash(
        &self,
        algo: HashAlgo,
        requests: &[HashRequest],
        each: &mut dyn FnMut(usize, HashReply) -> ControlFlow<()>,
    ) -> Result<(), SourceError> {
        let mut replies = Vec::new();
        hash_requests(
            &self.inner,
            algo,
            requests,
            NonZeroUsize::MIN,
            |i, reply| {
                replies.push((i, reply));
            },
        )
        .unwrap();
        for (n, (i, reply)) in replies.into_iter().enumerate() {
            if self.fail_after == Some(n) {
                return Err(SourceError::Remote("the phone was unplugged".into()));
            }
            if each(i, reply).is_break() {
                break;
            }
        }
        Ok(())
    }
}

fn tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.jpg"), "aaaa").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/b.jpg"), vec![7_u8; 300_000]).unwrap();
    dir
}

/// `(path key, full digest)` of each stored file.
type Digests = Vec<(String, Option<Vec<u8>>)>;

/// Scans `remote` as an eager location.
fn scan(root: &Path, remote: Remote, algo: HashAlgo) -> (Arc<Remote>, Digests) {
    let catalog_dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(catalog_dir.path().join("c.db")).unwrap();
    catalog
        .add_location(
            NewLocation::local("phone", root)
                .hash_policy(HashPolicy::Eager)
                .hash_algo(algo),
        )
        .unwrap();
    let remote = Arc::new(remote);
    Scanner::start_with_source(
        &catalog,
        "phone",
        Arc::clone(&remote) as Arc<dyn Source>,
        ScanOptions::default(),
    )
    .unwrap()
    .wait()
    .unwrap();
    let rows = rusqlite::Connection::open(catalog.path())
        .unwrap()
        .prepare("SELECT path_key, hash_full FROM entries WHERE kind = 'file' ORDER BY path_key")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    (remote, rows)
}

fn local_digest(path: &Path) -> Vec<u8> {
    let len = fs::metadata(path).unwrap().len();
    hash_reader(HashAlgo::Blake3, fs::File::open(path).unwrap(), len)
        .unwrap()
        .as_bytes()
        .to_vec()
}

#[test]
fn a_remote_scan_stores_local_digests_without_opening_files() {
    let dir = tree();
    let (remote, rows) = scan(dir.path(), Remote::new(dir.path()), HashAlgo::Blake3);
    assert_eq!(remote.opens.load(Ordering::SeqCst), 0);
    assert_eq!(
        rows,
        [
            (
                "a.jpg".to_owned(),
                Some(local_digest(&dir.path().join("a.jpg")))
            ),
            (
                "sub/b.jpg".to_owned(),
                Some(local_digest(&dir.path().join("sub/b.jpg")))
            ),
        ]
    );
}

#[test]
fn an_algorithm_the_remote_lacks_gives_warnings_not_digests() {
    let dir = tree();
    let mut remote = Remote::new(dir.path());
    remote.algos = vec![HashAlgo::Sha256];
    let (remote, rows) = scan(dir.path(), remote, HashAlgo::Blake3);
    assert_eq!(remote.opens.load(Ordering::SeqCst), 0);
    assert!(rows.iter().all(|(_, digest)| digest.is_none()), "{rows:?}");
}

#[test]
fn a_remote_that_fails_midway_leaves_the_rest_unhashed() {
    let dir = tree();
    let mut remote = Remote::new(dir.path());
    remote.fail_after = Some(1);
    let (_, rows) = scan(dir.path(), remote, HashAlgo::Blake3);
    let hashed = rows.iter().filter(|(_, digest)| digest.is_some()).count();
    assert_eq!(hashed, 1, "{rows:?}");
}

#[test]
fn hash_requests_matches_local_hashing_and_checks_metadata() {
    let dir = tree();
    let source = LocalSource::new(dir.path(), &[]).unwrap();
    let path = |key: &str| RelPath::from_raw(PathEncoding::native(), key.into()).unwrap();
    let b = dir.path().join("sub/b.jpg");
    let meta = source.stat(&path("sub/b.jpg")).unwrap();
    let requests = [
        HashRequest {
            path: path("sub/b.jpg"),
            size: meta.size,
            mtime_ns: meta.mtime_ns,
            partial: true,
        },
        HashRequest {
            path: path("sub/b.jpg"),
            size: meta.size,
            mtime_ns: meta.mtime_ns,
            partial: false,
        },
        HashRequest {
            path: path("sub/b.jpg"),
            size: meta.size + 1,
            mtime_ns: meta.mtime_ns,
            partial: false,
        },
        HashRequest {
            path: path("missing.jpg"),
            size: 1,
            mtime_ns: None,
            partial: false,
        },
    ];
    let mut replies = vec![None; requests.len()];
    hash_requests(
        &source,
        HashAlgo::Blake3,
        &requests,
        NonZeroUsize::new(2).unwrap(),
        |i, reply| {
            replies[i] = Some(reply);
        },
    )
    .unwrap();
    let expected_partial =
        partial_hash(HashAlgo::Blake3, fs::File::open(&b).unwrap(), meta.size).unwrap();
    assert_eq!(replies[0], Some(HashReply::Partial(expected_partial)));
    assert!(
        matches!(&replies[1], Some(HashReply::Full(d)) if d.as_bytes() == local_digest(&b).as_slice())
    );
    assert!(
        matches!(replies[2], Some(HashReply::Warning(_))),
        "size changed"
    );
    assert!(
        matches!(replies[3], Some(HashReply::Warning(_))),
        "vanished"
    );
}
