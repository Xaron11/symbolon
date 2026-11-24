//! Scanning: list a location, store its entries in the catalog and hash file contents.
//!
//! [`Scanner::start`] runs one scan on a background thread and returns a [`ScanHandle`] that
//! streams [`ScanEvent`]s and can cancel, pause and resume the scan. Results are committed in
//! short batches as the scan goes, so `Query` sees partial results immediately, and nothing
//! committed is lost if the scan is cancelled, crashes or loses its device: the next scan of
//! the location reuses it. See ADR-0009 for the pipeline.

mod run;

use std::fmt;
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::Receiver;

use crate::catalog::{Catalog, CatalogError};
use crate::hash::Digest;
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
    #[error("location {0:?} was recorded on another platform and cannot be scanned here")]
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
    /// patterns. Errors during the scan are returned by [`ScanHandle::wait`].
    pub fn start(
        catalog: &Catalog,
        label: &str,
        options: ScanOptions,
    ) -> Result<ScanHandle, ScanError> {
        let location = catalog.location(label)?;
        let source = run::source_for(&location)?;
        Ok(Self::spawn(catalog.clone(), location, source, options))
    }

    /// Like [`Scanner::start`], reading through a custom `source` instead of the one the
    /// location's root implies (Android transports, tests).
    ///
    /// # Errors
    ///
    /// [`ScanError::Catalog`] if the location does not exist.
    pub fn start_with_source(
        catalog: &Catalog,
        label: &str,
        source: Arc<dyn Source>,
        options: ScanOptions,
    ) -> Result<ScanHandle, ScanError> {
        let location = catalog.location(label)?;
        Ok(Self::spawn(catalog.clone(), location, source, options))
    }

    fn spawn(
        catalog: Catalog,
        location: crate::catalog::Location,
        source: Arc<dyn Source>,
        options: ScanOptions,
    ) -> ScanHandle {
        let (sender, events) = crossbeam_channel::bounded(EVENT_BUFFER);
        let control = Arc::new(Control::default());
        let thread_control = Arc::clone(&control);
        let thread = std::thread::Builder::new()
            .name(format!("symbolon-scan-{}", location.label))
            .spawn(move || {
                run::run(
                    &catalog,
                    &location,
                    &*source,
                    &options,
                    &thread_control,
                    &sender,
                )
            });
        ScanHandle {
            events,
            control,
            thread: thread.ok(),
        }
    }
}

/// A running scan. Drop the events receiver or call [`ScanHandle::wait`] when done reading
/// events; an unread, full event buffer pauses the scan until it is drained.
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
struct Control {
    cancelled: AtomicBool,
    paused: Mutex<bool>,
    unpaused: Condvar,
}

impl Control {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        // Wake paused workers so they observe the cancellation.
        self.set_paused(false);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn set_paused(&self, paused: bool) {
        *self.paused.lock().unwrap_or_else(PoisonError::into_inner) = paused;
        self.unpaused.notify_all();
    }

    /// Blocks while paused; returns `false` if the scan was cancelled.
    fn proceed(&self) -> bool {
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
