//! The scan pipeline (ADR-0009): walk and upsert, drop unseen entries, hash, finish.

use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{RecvTimeoutError, Sender};
use rayon::prelude::*;
use rusqlite::{Connection, TransactionBehavior, params};

use super::{
    Control, ScanCounts, ScanError, ScanEvent, ScanId, ScanOptions, ScanPhase, ScanProgress,
    ScanStatus, ScanSummary,
};
use crate::catalog::{HashPolicy, Location, LocationRoot, now_ms};
use crate::hash::{Digest, HashAlgo, HashError, hash_reader};
use crate::pathkey::RelPath;
#[cfg(feature = "local")]
use crate::source::LocalSource;
use crate::source::{
    EntryKind, Source, SourceEntry, SourceWarning, WalkItem, WalkOutcome, WarningKind,
};

/// Entries or hash results per write transaction.
const BATCH: usize = 512;
/// Longest time results wait before being committed, so readers see steady progress.
const MAX_BATCH_AGE: Duration = Duration::from_millis(250);

const UPSERT_ENTRY: &str = "\
INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, seen_scan_id)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT (location_id, rel_path_raw) DO UPDATE SET
    path_key = excluded.path_key,
    parent_key = excluded.parent_key,
    seen_scan_id = excluded.seen_scan_id,
    hash_partial = CASE WHEN entries.kind = excluded.kind AND entries.size = excluded.size
        AND (entries.mtime_ns IS excluded.mtime_ns
             OR excluded.mtime_ns BETWEEN entries.mtime_ns - ?9 AND entries.mtime_ns + ?9)
        THEN entries.hash_partial END,
    hash_full = CASE WHEN entries.kind = excluded.kind AND entries.size = excluded.size
        AND (entries.mtime_ns IS excluded.mtime_ns
             OR excluded.mtime_ns BETWEEN entries.mtime_ns - ?9 AND entries.mtime_ns + ?9)
        THEN entries.hash_full END,
    kind = excluded.kind,
    size = excluded.size,
    mtime_ns = excluded.mtime_ns";

const INSERT_WARNING: &str =
    "INSERT INTO scan_warnings (scan_id, rel_path_raw, kind, message) VALUES (?1, ?2, ?3, ?4)";

pub(super) fn source_for(location: &Location) -> Result<Arc<dyn Source>, ScanError> {
    match &location.root {
        #[cfg(feature = "local")]
        LocationRoot::Local(root) => Ok(Arc::new(LocalSource::new(
            root.clone(),
            &location.ignore_patterns,
        )?)),
        #[cfg(not(feature = "local"))]
        LocationRoot::Local(_) => Err(ScanError::Unsupported(location.label.clone())),
        LocationRoot::Foreign { .. } => Err(ScanError::NotScannable(location.label.clone())),
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
        Ok(status) => {
            let summary = scan.finish(status, None)?;
            scan.emit(ScanEvent::Finished(summary.clone()));
            Ok(summary)
        }
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

struct HashJob {
    id: i64,
    path: RelPath,
    size: u64,
    mtime_ns: Option<i64>,
}

enum HashOutcome<'a> {
    Hashed(&'a HashJob, Digest),
    Warning(SourceWarning),
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

    fn emit(&self, event: ScanEvent) {
        // A dropped receiver only means nobody is listening; the scan goes on.
        self.events.send(event).ok();
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
            let mut upsert = tx.prepare_cached(UPSERT_ENTRY)?;
            for entry in batch.drain(..) {
                let key = entry.path.key();
                upsert.execute(params![
                    self.location.id.get(),
                    entry.path.as_bytes(),
                    key.as_str(),
                    key.parent(),
                    entry.kind.name(),
                    entry.size,
                    entry.mtime_ns,
                    self.id.get(),
                    tolerance,
                ])?;
            }
            insert_warnings(&tx, self.id, &mut self.pending_warnings)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn hash_jobs(&self) -> Result<Vec<HashJob>, ScanError> {
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
            jobs.push(HashJob {
                id,
                path,
                size,
                mtime_ns,
            });
        }
        Ok(jobs)
    }

    fn hash(&mut self) -> Result<(), ScanError> {
        let jobs = self.hash_jobs()?;
        self.to_hash = (jobs.len() as u64, jobs.iter().map(|job| job.size).sum());
        self.hashing_started = Some(Instant::now());
        self.progress(ScanPhase::Hashing, true);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.options.resolved_threads())
            .thread_name(|i| format!("symbolon-hash-{i}"))
            .build()
            .map_err(|e| ScanError::Threads(e.to_string()))?;
        let (sender, results) = crossbeam_channel::bounded::<HashOutcome<'_>>(BATCH);
        let (source, control, algo) = (self.source, self.control, self.location.hash_algo);

        std::thread::scope(|scope| {
            let jobs = &jobs;
            scope.spawn(move || {
                pool.install(|| {
                    jobs.par_iter().for_each_with(sender, |sender, job| {
                        if control.proceed() {
                            // The receiver only goes away if the writer failed; stop quietly.
                            sender.send(hash_one(source, algo, job)).ok();
                        }
                    });
                });
            });
            self.write_hashes(&results)
        })?;
        self.progress(ScanPhase::Hashing, true);
        Ok(())
    }

    fn write_hashes(
        &mut self,
        results: &crossbeam_channel::Receiver<HashOutcome<'_>>,
    ) -> Result<(), ScanError> {
        let mut batch: Vec<(&HashJob, Digest)> = Vec::with_capacity(BATCH);
        let mut batch_started = Instant::now();
        loop {
            let done = match results.recv_timeout(self.options.progress_interval) {
                Ok(HashOutcome::Hashed(job, digest)) => {
                    self.counts.hashed_files += 1;
                    self.counts.hashed_bytes += job.size;
                    if self.options.file_events {
                        self.emit(ScanEvent::FileHashed {
                            path: job.path.clone(),
                            digest,
                        });
                    }
                    batch.push((job, digest));
                    false
                }
                Ok(HashOutcome::Warning(warning)) => {
                    self.warn(warning);
                    false
                }
                Err(RecvTimeoutError::Timeout) => false,
                Err(RecvTimeoutError::Disconnected) => true,
            };
            if done || batch.len() >= BATCH || batch_started.elapsed() >= MAX_BATCH_AGE {
                self.store_hashes(&mut batch)?;
                batch_started = Instant::now();
            }
            if done {
                return Ok(());
            }
            self.progress(ScanPhase::Hashing, false);
        }
    }

    fn store_hashes(&mut self, batch: &mut Vec<(&HashJob, Digest)>) -> Result<(), ScanError> {
        let tx = self.conn.transaction()?;
        {
            // The guard skips entries a newer scan changed in the meantime.
            let mut update = tx.prepare_cached(
                "UPDATE entries SET hash_full = ?1 WHERE id = ?2 AND size = ?3 AND mtime_ns IS ?4",
            )?;
            for (job, digest) in batch.drain(..) {
                update.execute(params![digest.as_bytes(), job.id, job.size, job.mtime_ns])?;
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

/// Hashes one file and re-checks it afterwards; any change turns the result into a warning.
fn hash_one<'a>(source: &dyn Source, algo: HashAlgo, job: &'a HashJob) -> HashOutcome<'a> {
    let warning = |kind, message: String| {
        HashOutcome::Warning(SourceWarning {
            path: Some(job.path.clone()),
            kind,
            message,
        })
    };
    let reader = match source.open(&job.path) {
        Ok(reader) => reader,
        Err(e) => return HashOutcome::Warning(SourceWarning::io(Some(job.path.clone()), &e)),
    };
    let digest = match hash_reader(algo, reader, job.size) {
        Ok(digest) => digest,
        Err(HashError::SizeMismatch { expected, actual }) => {
            return warning(
                WarningKind::Changed,
                format!("size changed while hashing (listed {expected} bytes, read {actual})"),
            );
        }
        Err(HashError::Io(e)) => {
            return HashOutcome::Warning(SourceWarning::io(Some(job.path.clone()), &e));
        }
        Err(e) => return warning(WarningKind::Io, e.to_string()),
    };
    match source.stat(&job.path) {
        Ok(now) if now.size == job.size && now.mtime_ns == job.mtime_ns => {
            HashOutcome::Hashed(job, digest)
        }
        Ok(_) => warning(
            WarningKind::Changed,
            "modified while hashing; hash discarded".to_owned(),
        ),
        Err(e) => HashOutcome::Warning(SourceWarning::io(Some(job.path.clone()), &e)),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a throughput figure needs no more than f64 precision"
)]
fn rate(bytes: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs > 0.0 { bytes as f64 / secs } else { 0.0 }
}
