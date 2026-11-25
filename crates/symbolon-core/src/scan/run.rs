//! The scan pipeline (ADR-0009): walk and upsert, drop unseen entries, hash, finish.

use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use rusqlite::{Connection, TransactionBehavior, params};

use super::hashing::{self, Job, Outcome, Tier};
use super::{
    Control, ScanCounts, ScanError, ScanEvent, ScanId, ScanOptions, ScanPhase, ScanProgress,
    ScanStatus, ScanSummary,
};
use crate::catalog::{HashPolicy, Location, LocationRoot, now_ms};
use crate::hash::Digest;
use crate::pathkey::RelPath;
#[cfg(feature = "local")]
use crate::source::LocalSource;
use crate::source::{EntryKind, Source, SourceEntry, SourceWarning, WalkItem, WalkOutcome};

/// Entries or hash results per write transaction.
const BATCH: usize = 512;
/// Longest time results wait before being committed, so readers see steady progress.
const MAX_BATCH_AGE: Duration = Duration::from_millis(250);

/// Marks an unchanged entry as seen. Only `seen_scan_id`, which no index covers, is written,
/// so a rescan of an unchanged location touches no index.
const MARK_UNCHANGED: &str = "\
UPDATE entries SET seen_scan_id = ?3
WHERE location_id = ?1 AND rel_path_raw = ?2 AND kind = ?4 AND size = ?5
    AND (mtime_ns IS ?6 OR ?6 BETWEEN mtime_ns - ?7 AND mtime_ns + ?7)";

/// Inserts a new entry, or replaces a changed one and clears its digests.
const UPSERT_CHANGED: &str = "\
INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, seen_scan_id)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT (location_id, rel_path_raw) DO UPDATE SET
    seen_scan_id = excluded.seen_scan_id,
    kind = excluded.kind,
    size = excluded.size,
    mtime_ns = excluded.mtime_ns,
    hash_partial = NULL,
    hash_full = NULL";

const INSERT_WARNING: &str =
    "INSERT INTO scan_warnings (scan_id, rel_path_raw, kind, message) VALUES (?1, ?2, ?3, ?4)";

pub(crate) fn source_for(
    #[cfg_attr(
        not(feature = "android-adb"),
        expect(unused_variables, reason = "only phone locations need the catalog")
    )]
    catalog: &crate::catalog::Catalog,
    location: &Location,
) -> Result<Arc<dyn Source>, ScanError> {
    match &location.root {
        #[cfg(feature = "android-adb")]
        LocationRoot::Android { .. } => Ok(Arc::new(catalog.adb_source(location)?)),
        #[cfg(not(feature = "android-adb"))]
        LocationRoot::Android { .. } => Err(ScanError::Unsupported(location.label.clone())),
        #[cfg(feature = "local")]
        LocationRoot::Local(root) => Ok(Arc::new(LocalSource::new(
            root.clone(),
            &location.ignore_patterns,
        )?)),
        #[cfg(not(feature = "local"))]
        LocationRoot::Local(_) => Err(ScanError::Unsupported(location.label.clone())),
        LocationRoot::Foreign { .. } | LocationRoot::Imported { .. } => {
            Err(ScanError::NotScannable(location.label.clone()))
        }
    }
}

pub(super) fn run(
    catalog: &crate::catalog::Catalog,
    location: &Location,
    source: &dyn Source,
    options: &ScanOptions,
    control: &Control,
    events: &Sender<ScanEvent>,
) -> Result<ScanSummary, ScanError> {
    let mut conn = catalog.connect()?;
    let scan_id = begin(&mut conn, location)?;
    let mut scan = Scan {
        conn,
        location,
        source,
        options,
        control,
        events,
        id: scan_id,
        counts: ScanCounts::default(),
        started: Instant::now(),
        last_progress: None,
        hashing_started: None,
        to_hash: (0, 0),
        pending_warnings: Vec::new(),
    };
    scan.emit(ScanEvent::Started {
        scan_id,
        location: location.label.clone(),
    });
    match scan.execute() {
        Ok(status) => scan.finish(status, None),
        Err(error) => {
            // Best effort: the original error matters more than a failure to record it.
            scan.finish(ScanStatus::Failed, Some(&error.to_string()))
                .ok();
            Err(error)
        }
    }
}

/// Marks crashed scans of the location as interrupted and opens a new one.
fn begin(conn: &mut Connection, location: &Location) -> Result<ScanId, ScanError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = now_ms();
    tx.execute(
        "UPDATE scans SET status = 'interrupted', finished_at = ?2 \
         WHERE location_id = ?1 AND status = 'running'",
        params![location.id.get(), now],
    )?;
    tx.execute(
        "INSERT INTO scans (location_id, started_at, status) VALUES (?1, ?2, 'running')",
        params![location.id.get(), now],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(ScanId(id))
}

struct Scan<'a> {
    conn: Connection,
    location: &'a Location,
    source: &'a dyn Source,
    options: &'a ScanOptions,
    control: &'a Control,
    events: &'a Sender<ScanEvent>,
    id: ScanId,
    counts: ScanCounts,
    started: Instant,
    last_progress: Option<Instant>,
    hashing_started: Option<Instant>,
    to_hash: (u64, u64),
    pending_warnings: Vec<SourceWarning>,
}

impl Scan<'_> {
    fn execute(&mut self) -> Result<ScanStatus, ScanError> {
        let outcome = self.walk()?;
        if outcome == WalkOutcome::Complete && !self.control.is_cancelled() {
            self.conn.execute(
                "DELETE FROM entries WHERE location_id = ?1 AND seen_scan_id != ?2",
                params![self.location.id.get(), self.id.get()],
            )?;
        }
        self.counts.reused_hashes = self.conn.query_row(
            "SELECT count(*) FROM entries WHERE location_id = ?1 AND seen_scan_id = ?2 \
             AND kind = 'file' AND hash_full IS NOT NULL",
            params![self.location.id.get(), self.id.get()],
            |row| row.get(0),
        )?;
        let hash = self.location.hash_policy == HashPolicy::Eager || self.options.hash_all;
        if hash && !self.control.is_cancelled() {
            self.hash()?;
        }
        Ok(if self.control.is_cancelled() {
            ScanStatus::Cancelled
        } else {
            ScanStatus::Completed
        })
    }

    /// Sends without waiting: with the buffer full the event is dropped, never the scan
    /// paused. A dropped receiver only means nobody is listening.
    fn emit(&self, event: ScanEvent) {
        self.events.try_send(event).ok();
    }

    fn progress(&mut self, phase: ScanPhase, force: bool) {
        let due = self
            .last_progress
            .is_none_or(|last| last.elapsed() >= self.options.progress_interval);
        if !(due || force) {
            return;
        }
        self.last_progress = Some(Instant::now());
        let bytes_per_sec = self
            .hashing_started
            .map_or(0.0, |start| rate(self.counts.hashed_bytes, start.elapsed()));
        self.emit(ScanEvent::Progress(ScanProgress {
            phase,
            counts: self.counts,
            to_hash_files: self.to_hash.0,
            to_hash_bytes: self.to_hash.1,
            bytes_per_sec,
        }));
    }

    fn warn(&mut self, warning: SourceWarning) {
        self.counts.warnings += 1;
        self.emit(ScanEvent::Warning(warning.clone()));
        self.pending_warnings.push(warning);
    }

    fn walk(&mut self) -> Result<WalkOutcome, ScanError> {
        let source = self.source;
        let mut batch: Vec<SourceEntry> = Vec::with_capacity(BATCH);
        let mut batch_started = Instant::now();
        let mut failure = None;
        let outcome = source.walk(&mut |item| {
            if !self.control.proceed() {
                return ControlFlow::Break(());
            }
            match item {
                WalkItem::Entry(entry) => {
                    match entry.kind {
                        EntryKind::File => {
                            self.counts.files += 1;
                            self.counts.bytes += entry.size;
                        }
                        EntryKind::Dir => self.counts.dirs += 1,
                        EntryKind::Symlink => self.counts.symlinks += 1,
                        EntryKind::Other => {}
                    }
                    if self.options.file_events {
                        self.emit(ScanEvent::FileDiscovered {
                            path: entry.path.clone(),
                            kind: entry.kind,
                            size: entry.size,
                        });
                    }
                    batch.push(entry);
                }
                WalkItem::Warning(warning) => self.warn(warning),
            }
            if batch.len() >= BATCH || batch_started.elapsed() >= MAX_BATCH_AGE {
                if let Err(e) = self.write_entries(&mut batch) {
                    failure = Some(e);
                    return ControlFlow::Break(());
                }
                batch_started = Instant::now();
            }
            self.progress(ScanPhase::Walking, false);
            ControlFlow::Continue(())
        })?;
        if let Some(e) = failure {
            return Err(e);
        }
        self.write_entries(&mut batch)?;
        self.progress(ScanPhase::Walking, true);
        Ok(outcome)
    }

    fn write_entries(&mut self, batch: &mut Vec<SourceEntry>) -> Result<(), ScanError> {
        let tolerance = i64::try_from(self.options.mtime_tolerance.as_nanos()).unwrap_or(i64::MAX);
        let tx = self.conn.transaction()?;
        {
            let mut unchanged = tx.prepare_cached(MARK_UNCHANGED)?;
            let mut changed = tx.prepare_cached(UPSERT_CHANGED)?;
            for entry in batch.drain(..) {
                let location = self.location.id.get();
                let seen = unchanged.execute(params![
                    location,
                    entry.path.as_bytes(),
                    self.id.get(),
                    entry.kind.name(),
                    entry.size,
                    entry.mtime_ns,
                    tolerance,
                ])?;
                if seen == 0 {
                    let key = entry.path.key();
                    changed.execute(params![
                        location,
                        entry.path.as_bytes(),
                        key.as_str(),
                        key.parent(),
                        entry.kind.name(),
                        entry.size,
                        entry.mtime_ns,
                        self.id.get(),
                    ])?;
                }
            }
            insert_warnings(&tx, self.id, &mut self.pending_warnings)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn hash_jobs(&self) -> Result<Vec<Job>, ScanError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, rel_path_raw, size, mtime_ns FROM entries \
             WHERE location_id = ?1 AND seen_scan_id = ?2 AND kind = 'file' AND hash_full IS NULL \
             ORDER BY rel_path_raw",
        )?;
        let rows = stmt.query_map(params![self.location.id.get(), self.id.get()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?;
        let mut jobs = Vec::new();
        for row in rows {
            let (id, raw, size, mtime_ns) = row?;
            let path = RelPath::from_raw(self.location.path_encoding, raw)
                .map_err(|e| crate::catalog::CatalogError::Corrupt(e.to_string()))?;
            jobs.push(Job {
                id,
                path,
                size,
                mtime_ns,
                tier: Tier::Full,
            });
        }
        Ok(jobs)
    }

    fn hash(&mut self) -> Result<(), ScanError> {
        let jobs = self.hash_jobs()?;
        self.to_hash = (jobs.len() as u64, jobs.iter().map(|job| job.size).sum());
        self.hashing_started = Some(Instant::now());
        self.progress(ScanPhase::Hashing, true);

        let pool = hashing_pool(self.options)?;
        let (source, control, algo) = (self.source, self.control, self.location.hash_algo);
        let tick = self.options.progress_interval;
        let mut batch: Vec<(&Job, Digest)> = Vec::with_capacity(BATCH);
        let mut batch_started = Instant::now();
        hashing::run(&pool, source, algo, &jobs, control, tick, |outcome| {
            match outcome {
                Some(Outcome::Full(job, digest)) => {
                    self.counts.hashed_files += 1;
                    self.counts.hashed_bytes += job.size;
                    if self.options.file_events {
                        self.emit(ScanEvent::FileHashed {
                            path: job.path.clone(),
                            digest,
                        });
                    }
                    batch.push((job, digest));
                }
                Some(Outcome::Warning(_, warning)) => self.warn(warning),
                // Scans only compute full digests.
                Some(Outcome::Partial(..)) | None => {}
            }
            if batch.len() >= BATCH || batch_started.elapsed() >= MAX_BATCH_AGE {
                self.store_hashes(&mut batch)?;
                batch_started = Instant::now();
            }
            self.progress(ScanPhase::Hashing, false);
            Ok::<(), ScanError>(())
        })?;
        self.store_hashes(&mut batch)?;
        self.progress(ScanPhase::Hashing, true);
        Ok(())
    }

    fn store_hashes(&mut self, batch: &mut Vec<(&Job, Digest)>) -> Result<(), ScanError> {
        let tx = self.conn.transaction()?;
        {
            // The guard skips entries a newer scan changed in the meantime.
            let mut update = tx.prepare_cached(hashing::store_statement(Tier::Full))?;
            for (job, digest) in batch.drain(..) {
                update.execute(params![
                    digest.as_bytes(),
                    job.id,
                    job.size,
                    job.mtime_ns,
                    job.path.as_bytes(),
                    digest.algo().name()
                ])?;
            }
            insert_warnings(&tx, self.id, &mut self.pending_warnings)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn finish(
        &mut self,
        status: ScanStatus,
        error: Option<&str>,
    ) -> Result<ScanSummary, ScanError> {
        let tx = self.conn.transaction()?;
        insert_warnings(&tx, self.id, &mut self.pending_warnings)?;
        let c = self.counts;
        tx.execute(
            "UPDATE scans SET status = ?2, finished_at = ?3, files = ?4, dirs = ?5, symlinks = ?6, \
             bytes = ?7, hashed_files = ?8, hashed_bytes = ?9, reused_hashes = ?10, warnings = ?11, \
             error = ?12 WHERE id = ?1",
            params![
                self.id.get(),
                status.name(),
                now_ms(),
                c.files,
                c.dirs,
                c.symlinks,
                c.bytes,
                c.hashed_files,
                c.hashed_bytes,
                c.reused_hashes,
                c.warnings,
                error,
            ],
        )?;
        tx.commit()?;
        Ok(ScanSummary {
            scan_id: self.id,
            status,
            counts: self.counts,
            elapsed: self.started.elapsed(),
        })
    }
}

fn insert_warnings(
    tx: &rusqlite::Transaction<'_>,
    scan_id: ScanId,
    warnings: &mut Vec<SourceWarning>,
) -> Result<(), ScanError> {
    let mut insert = tx.prepare_cached(INSERT_WARNING)?;
    for warning in warnings.drain(..) {
        insert.execute(params![
            scan_id.get(),
            warning.path.as_ref().map(RelPath::as_bytes),
            warning.kind.name(),
            warning.message,
        ])?;
    }
    Ok(())
}

/// The thread pool hashing runs on, sized by [`ScanOptions::threads`].
pub(crate) fn hashing_pool(options: &ScanOptions) -> Result<rayon::ThreadPool, ScanError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(options.resolved_threads())
        .thread_name(|i| format!("symbolon-hash-{i}"))
        .build()
        .map_err(|e| ScanError::Threads(e.to_string()))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a throughput figure needs no more than f64 precision"
)]
fn rate(bytes: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs > 0.0 { bytes as f64 / secs } else { 0.0 }
}
