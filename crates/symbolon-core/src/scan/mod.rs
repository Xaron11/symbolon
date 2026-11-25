//! Scanning: list a location, store its entries in the catalog and hash file contents.
//!
//! [`Scanner::start`] runs one scan on a background thread and returns a [`ScanHandle`] that
//! streams [`ScanEvent`]s and can cancel, pause and resume the scan. Results are committed in
//! short batches as the scan goes, so `Query` sees partial results immediately, and nothing
//! committed is lost if the scan is cancelled, crashes or loses its device: the next scan of
//! the location reuses it. See ADR-0009 for the pipeline.
//!
//! Only one scan of a location runs at a time, across threads and processes; starting a
//! second one fails with [`ScanError::AlreadyScanning`] (ADR-0012).

pub(crate) mod hashing;
pub(crate) mod lock;
mod run;

pub use self::hashing::hash_requests;
pub(crate) use self::run::{hashing_pool, source_for};

use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use crossbeam_channel::Receiver;

use crate::catalog::{Catalog, CatalogError, Whereabouts};
use crate::hash::Digest;
use crate::identity::Fingerprint;
use crate::pathkey::RelPath;
use crate::source::{EntryKind, Source, SourceError, SourceWarning};

const EVENT_BUFFER: usize = 1024;

/// Errors that stop a scan. Problems with single files are [`ScanEvent::Warning`]s instead.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// Opening the catalog or looking up the location failed.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// Writing scan results failed.
    #[error("catalog database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The location's root cannot be listed (for example an unplugged drive).
    #[error(transparent)]
    Source(#[from] SourceError),
    /// The location was recorded on a platform with another path encoding.
    #[error(
        "location {0:?} was imported or recorded on another platform and cannot be scanned here"
    )]
    NotScannable(String),
    /// The location kind needs a source this build does not include (feature `local`).
    #[error("this build cannot scan location {0:?}: the local source is not compiled in")]
    Unsupported(String),
    /// The hashing threads could not be started.
    #[error("could not start hashing threads: {0}")]
    Threads(String),
    /// The scan thread panicked.
    #[error("the scan thread panicked")]
    Panicked,
    /// Another scan of the location is running, in this process or another one.
    #[error("location {label:?} is already being scanned")]
    AlreadyScanning {
        /// The location's label.
        label: String,
        /// When the running scan started, if it has recorded itself yet.
        since: Option<SystemTime>,
    },
    /// The location's lock file could not be created or locked.
    #[error("could not lock the location for scanning: {0}")]
    Lock(#[source] std::io::Error),
    /// The location's device is mounted somewhere else; relocate it first
    /// ([`Catalog::relocate`], ADR-0016).
    #[error("location {label:?} moved from {} to {}", from.display(), to.display())]
    Moved {
        /// The location's label.
        label: String,
        /// The stored root.
        from: PathBuf,
        /// Where its device is now.
        to: PathBuf,
    },
    /// The location's device matches several mounted directories; the user has to pick one.
    #[error("location {label:?} is not at {}, and its device is mounted at several places", root.display())]
    Ambiguous {
        /// The location's label.
        label: String,
        /// The stored root.
        root: PathBuf,
        /// The matching directories.
        candidates: Vec<PathBuf>,
    },
    /// Another device (or one without a fingerprint) is at the location's root; scanning it
    /// would record its contents as the location's. Accept it with [`Catalog::relocate`].
    #[error("location {label:?} expects device {expected} at {}, but found {}", root.display(), found.as_ref().map_or_else(|| "no device id".to_owned(), ToString::to_string))]
    OtherDevice {
        /// The location's label.
        label: String,
        /// The stored root.
        root: PathBuf,
        /// The recorded fingerprint.
        expected: Fingerprint,
        /// What is at the root now.
        found: Option<Fingerprint>,
    },
}

/// Database id of a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScanId(pub(crate) i64);

impl ScanId {
    /// The numeric id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// State of a scan, as stored in the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanStatus {
    /// In progress (or crashed without being noticed yet).
    Running,
    /// Walked and hashed everything it intended to.
    Completed,
    /// Stopped on request; committed results are kept.
    Cancelled,
    /// Stopped by an error; committed results are kept.
    Failed,
    /// Found still `running` by a later scan: the process crashed or the device vanished.
    Interrupted,
}

impl ScanStatus {
    /// The stable name stored in the catalog.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

impl fmt::Display for ScanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for ScanStatus {
    type Err = CatalogError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        [
            Self::Running,
            Self::Completed,
            Self::Cancelled,
            Self::Failed,
            Self::Interrupted,
        ]
        .into_iter()
        .find(|status| status.name() == s)
        .ok_or_else(|| CatalogError::Corrupt(format!("unknown scan status {s:?}")))
    }
}

/// How a scan runs.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ScanOptions {
    /// Files hashed in parallel. `None` uses the available parallelism, at most 4: storage, not
    /// the CPU, is usually the limit, and slow media get slower with more readers (ADR-0003).
    pub threads: Option<NonZeroUsize>,
    /// How far a modification time may move before a file counts as changed. Zero by default:
    /// any change discards the cached hash. Raise it for sources whose timestamps jitter.
    pub mtime_tolerance: Duration,
    /// Hash every file during this scan even if the location's policy is lazy.
    pub hash_all: bool,
    /// Send [`ScanEvent::FileDiscovered`] and [`ScanEvent::FileHashed`] for every file. Off by
    /// default: a large location would flood the client.
    pub file_events: bool,
    /// Minimum time between [`ScanEvent::Progress`] events.
    pub progress_interval: Duration,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            threads: None,
            mtime_tolerance: Duration::ZERO,
            hash_all: false,
            file_events: false,
            progress_interval: Duration::from_millis(100),
        }
    }
}

impl ScanOptions {
    /// Sets [`ScanOptions::threads`].
    #[must_use]
    pub fn threads(mut self, threads: NonZeroUsize) -> Self {
        self.threads = Some(threads);
        self
    }

    /// Sets [`ScanOptions::mtime_tolerance`].
    #[must_use]
    pub fn mtime_tolerance(mut self, tolerance: Duration) -> Self {
        self.mtime_tolerance = tolerance;
        self
    }

    /// Sets [`ScanOptions::hash_all`].
    #[must_use]
    pub fn hash_all(mut self, hash_all: bool) -> Self {
        self.hash_all = hash_all;
        self
    }

    /// Sets [`ScanOptions::file_events`].
    #[must_use]
    pub fn file_events(mut self, file_events: bool) -> Self {
        self.file_events = file_events;
        self
    }

    fn resolved_threads(&self) -> usize {
        self.threads.map_or_else(
            || std::thread::available_parallelism().map_or(1, |n| n.get().min(4)),
            NonZeroUsize::get,
        )
    }
}

/// Phase of a running scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanPhase {
    /// Listing entries; no file contents are read.
    Walking,
    /// Reading and hashing file contents.
    Hashing,
}

/// Counters of a scan so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ScanCounts {
    /// Regular files listed.
    pub files: u64,
    /// Directories listed.
    pub dirs: u64,
    /// Symlinks listed.
    pub symlinks: u64,
    /// Total size of the listed files.
    pub bytes: u64,
    /// Files hashed in this scan.
    pub hashed_files: u64,
    /// Bytes hashed in this scan.
    pub hashed_bytes: u64,
    /// Files whose digest was kept from an earlier scan because they did not change.
    pub reused_hashes: u64,
    /// Warnings reported.
    pub warnings: u64,
}

/// A progress report, sent at most every [`ScanOptions::progress_interval`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ScanProgress {
    /// Current phase.
    pub phase: ScanPhase,
    /// Counters so far.
    pub counts: ScanCounts,
    /// Files queued for hashing in this scan (0 while walking).
    pub to_hash_files: u64,
    /// Bytes queued for hashing in this scan (0 while walking).
    pub to_hash_bytes: u64,
    /// Hashing throughput so far, in bytes per second (0 while walking).
    pub bytes_per_sec: f64,
}

/// Result of a finished scan.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ScanSummary {
    /// The scan's id.
    pub scan_id: ScanId,
    /// How it ended: completed, cancelled or failed.
    pub status: ScanStatus,
    /// Final counters.
    pub counts: ScanCounts,
    /// Wall-clock duration.
    pub elapsed: Duration,
}

/// Something that happened during a scan.
///
/// Delivery is best-effort except for [`ScanEvent::Started`] (always first) and
/// [`ScanEvent::Finished`]: events that do not fit in the buffer of a slow client are dropped
/// (see [`ScanHandle`]). Dropped warnings are still counted and stored for
/// [`Query::warnings`](crate::query::Query::warnings).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ScanEvent {
    /// The scan started; always the first event.
    Started {
        /// The new scan's id.
        scan_id: ScanId,
        /// Label of the scanned location.
        location: String,
    },
    /// An entry was listed (only with [`ScanOptions::file_events`]).
    FileDiscovered {
        /// The entry.
        path: RelPath,
        /// Its kind.
        kind: EntryKind,
        /// Its size.
        size: u64,
    },
    /// A file was hashed (only with [`ScanOptions::file_events`]).
    FileHashed {
        /// The file.
        path: RelPath,
        /// Its full-content digest.
        digest: Digest,
    },
    /// Something about how the location is read (see [`Source::note`]), right after
    /// [`ScanEvent::Started`].
    Note(String),
    /// Periodic progress.
    Progress(ScanProgress),
    /// A problem with one entry; the scan continues.
    Warning(SourceWarning),
    /// The scan ended; always the last event unless the scan failed with an error.
    Finished(ScanSummary),
}

/// Starts scans. See the module documentation.
#[derive(Debug, Clone, Copy)]
pub struct Scanner;

impl Scanner {
    /// Starts scanning the location labelled `label` on a background thread.
    ///
    /// # Errors
    ///
    /// [`ScanError::Catalog`] if the location does not exist, [`ScanError::NotScannable`] for a
    /// location recorded on another platform, [`ScanError::Source`] for invalid ignore
    /// patterns, [`ScanError::AlreadyScanning`] while another scan of the location runs.
    /// Errors during the scan are returned by [`ScanHandle::wait`].
    pub fn start(
        catalog: &Catalog,
        label: &str,
        options: ScanOptions,
    ) -> Result<ScanHandle, ScanError> {
        let location = catalog.location(label)?;
        let label = location.label.clone();
        match catalog.locate_location(&location)? {
            Whereabouts::Moved { from, to } => return Err(ScanError::Moved { label, from, to }),
            Whereabouts::Ambiguous { root, candidates } => {
                return Err(ScanError::Ambiguous {
                    label,
                    root,
                    candidates,
                });
            }
            Whereabouts::OtherDevice {
                root,
                expected,
                found,
            } => {
                return Err(ScanError::OtherDevice {
                    label,
                    root,
                    expected,
                    found,
                });
            }
            // A missing root fails the walk like before; imported roots fail below.
            _ => {}
        }
        let source = source_for(catalog, &location)?;
        Self::spawn(catalog, location, source, options)
    }

    /// Like [`Scanner::start`], reading through a custom `source` instead of the one the
    /// location's root implies (Android transports, tests).
    ///
    /// # Errors
    ///
    /// [`ScanError::Catalog`] if the location does not exist, [`ScanError::AlreadyScanning`]
    /// while another scan of the location runs.
    pub fn start_with_source(
        catalog: &Catalog,
        label: &str,
        source: Arc<dyn Source>,
        options: ScanOptions,
    ) -> Result<ScanHandle, ScanError> {
        let location = catalog.location(label)?;
        Self::spawn(catalog, location, source, options)
    }

    fn spawn(
        catalog: &Catalog,
        location: crate::catalog::Location,
        source: Arc<dyn Source>,
        options: ScanOptions,
    ) -> Result<ScanHandle, ScanError> {
        let lock = lock::ScanLock::acquire(catalog, &location)?;
        let catalog = catalog.clone();
        let (sender, events) = crossbeam_channel::bounded(EVENT_BUFFER);
        let control = Arc::new(Control::default());
        let thread_control = Arc::clone(&control);
        let thread = std::thread::Builder::new()
            .name(format!("symbolon-scan-{}", location.label))
            .spawn(move || {
                let result = run::run(
                    &catalog,
                    &location,
                    &*source,
                    &options,
                    &thread_control,
                    &sender,
                );
                drop(lock);
                if let Ok(summary) = &result
                    && summary.status == ScanStatus::Completed
                {
                    // Best effort: the scan itself succeeded either way.
                    catalog.record_missing_fingerprint(&location).ok();
                }
                if let Ok(summary) = &result {
                    // The only event that waits for room: the work is done and the location
                    // free, so waiting for the client to read costs nothing.
                    sender.send(ScanEvent::Finished(summary.clone())).ok();
                }
                result
            });
        Ok(ScanHandle {
            events,
            control,
            thread: thread.ok(),
        })
    }
}

/// A running scan.
///
/// Events are buffered; a client that stops reading never slows the scan down. When the buffer
/// is full, new events are dropped, except [`ScanEvent::Finished`], which waits for the client
/// to read or drop the receiver. Authoritative results are in the catalog (`Query`).
#[derive(Debug)]
pub struct ScanHandle {
    events: Receiver<ScanEvent>,
    control: Arc<Control>,
    thread: Option<JoinHandle<Result<ScanSummary, ScanError>>>,
}

impl ScanHandle {
    /// Events from the scan, ending with [`ScanEvent::Finished`].
    pub fn events(&self) -> &Receiver<ScanEvent> {
        &self.events
    }

    /// Asks the scan to stop after the files in progress. Committed results are kept and the
    /// scan ends as [`ScanStatus::Cancelled`].
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// Pauses the scan between files.
    pub fn pause(&self) {
        self.control.set_paused(true);
    }

    /// Resumes a paused scan.
    pub fn resume(&self) {
        self.control.set_paused(false);
    }

    /// Waits for the scan to end, discarding events not yet read.
    ///
    /// # Errors
    ///
    /// The error that stopped the scan; see [`ScanError`].
    pub fn wait(self) -> Result<ScanSummary, ScanError> {
        let Self { events, thread, .. } = self;
        drop(events);
        let thread =
            thread.ok_or_else(|| ScanError::Threads("could not spawn the scan thread".into()))?;
        thread.join().map_err(|_| ScanError::Panicked)?
    }
}

#[derive(Debug, Default)]
pub(crate) struct Control {
    cancelled: AtomicBool,
    paused: Mutex<bool>,
    unpaused: Condvar,
}

impl Control {
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        // Wake paused workers so they observe the cancellation.
        self.set_paused(false);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn set_paused(&self, paused: bool) {
        *self.paused.lock().unwrap_or_else(PoisonError::into_inner) = paused;
        self.unpaused.notify_all();
    }

    /// Blocks while paused; returns `false` if the scan was cancelled.
    pub(crate) fn proceed(&self) -> bool {
        let mut paused = self.paused.lock().unwrap_or_else(PoisonError::into_inner);
        while *paused && !self.is_cancelled() {
            paused = self
                .unpaused
                .wait(paused)
                .unwrap_or_else(PoisonError::into_inner);
        }
        !self.is_cancelled()
    }
}
