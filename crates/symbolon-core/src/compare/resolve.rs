//! Hashing what a comparison needs, then reclassifying (ADR-0010, "Hashing on demand").

use std::collections::{BTreeMap, HashSet};
use std::ops::ControlFlow;
use std::time::Instant;

use rusqlite::{named_params, params};

use super::{CompareError, Comparison};
use crate::catalog::{CatalogError, LocationRoot};
use crate::hash::PARTIAL_BLOCK;
use crate::pathkey::RelPath;
use crate::scan::hashing::{self, Job, Outcome, Tier};
use crate::scan::{Control, ScanOptions, hashing_pool, source_for};
use crate::source::SourceWarning;

/// Most hashing rounds [`Comparison::resolve`] runs: partial digests, then full digests, then
/// one more for files whose partners only got digests in the second round.
const MAX_ROUNDS: u32 = 3;
const BATCH: usize = 512;

/// Hashing one location needs before the comparison can decide more rows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HashNeed {
    /// Location label.
    pub label: String,
    /// Files needing a partial digest (first and last 64 KiB).
    pub partial_files: u64,
    /// Files needing a full digest.
    pub full_files: u64,
    /// Bytes that will be read.
    pub bytes: u64,
}

/// Progress of [`Comparison::resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolveEvent {
    /// Hashing of a location starts.
    Hashing {
        /// Location label.
        label: String,
        /// Round, starting at 1.
        round: u32,
        /// Files to hash.
        files: u64,
        /// Bytes to read.
        bytes: u64,
    },
    /// Progress within a location.
    Progress {
        /// Location label.
        label: String,
        /// Files done so far.
        files: u64,
        /// Bytes read so far.
        bytes: u64,
    },
    /// A file could not be hashed; its rows stay unverified.
    Warning {
        /// Location label.
        label: String,
        /// What went wrong.
        warning: SourceWarning,
    },
    /// A location cannot be read (for example an unplugged drive); its rows stay unverified.
    Unreachable {
        /// Location label.
        label: String,
        /// Why.
        reason: String,
    },
}

/// Result of [`Comparison::resolve`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ResolveSummary {
    /// Hashing rounds run.
    pub rounds: u32,
    /// Digests computed (partial and full).
    pub hashed_files: u64,
    /// Bytes read.
    pub hashed_bytes: u64,
    /// Files that could not be hashed.
    pub warnings: u64,
    /// Locations that could not be read.
    pub unreachable: Vec<String>,
    /// Whether the event callback stopped the work early.
    pub cancelled: bool,
}

/// Entries to hash, grouped by location id.
struct Plan {
    jobs: BTreeMap<i64, Vec<Job>>,
}

impl Comparison {
    /// What would have to be hashed for the comparison to decide more rows: unverified rows
    /// and only-in files that may be moves. Files whose size matches no candidate are never
    /// listed.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn hash_needs(&self) -> Result<Vec<HashNeed>, CompareError> {
        let plan = self.plan(&HashSet::new())?;
        Ok(plan
            .jobs
            .iter()
            .map(|(location, jobs)| HashNeed {
                label: self.label_of(*location),
                partial_files: jobs.iter().filter(|j| j.tier == Tier::Partial).count() as u64,
                full_files: jobs.iter().filter(|j| j.tier == Tier::Full).count() as u64,
                bytes: jobs.iter().map(|j| j.tier.bytes(j.size)).sum(),
            })
            .collect())
    }

    /// Hashes what [`Comparison::hash_needs`] lists, in up to three rounds (partial digests
    /// first, full digests only where partial ones still match), storing digests in the
    /// catalog and reclassifying after each round.
    ///
    /// `on_event` sees progress and may return [`ControlFlow::Break`] to stop early; work done
    /// so far is kept. Locations that cannot be read are skipped and their rows stay
    /// unverified.
    ///
    /// # Errors
    ///
    /// A database error, or [`CompareError::Catalog`] if a location vanished from the catalog.
    pub fn resolve(
        &mut self,
        options: &ScanOptions,
        mut on_event: impl FnMut(ResolveEvent) -> ControlFlow<()>,
    ) -> Result<ResolveSummary, CompareError> {
        let pool = hashing_pool(options).map_err(|e| CatalogError::Corrupt(e.to_string()))?;
        let control = Control::default();
        let mut summary = ResolveSummary::default();
        let mut failed = HashSet::new();
        for round in 1..=MAX_ROUNDS {
            let plan = self.plan(&failed)?;
            if plan.jobs.is_empty() || control.is_cancelled() {
                break;
            }
            summary.rounds = round;
            for (location_id, jobs) in &plan.jobs {
                let label = self.label_of(*location_id);
                let location = self.catalog.location(&label)?;
                let reachable = match &location.root {
                    LocationRoot::Local(root) => root.is_dir(),
                    LocationRoot::Foreign { .. } => false,
                };
                let source = match source_for(&location) {
                    Ok(source) if reachable => source,
                    result => {
                        let reason = result.err().map_or_else(
                            || "location root is not available".to_owned(),
                            |e| e.to_string(),
                        );
                        failed.extend(jobs.iter().map(|job| job.id));
                        summary.unreachable.push(label.clone());
                        if on_event(ResolveEvent::Unreachable { label, reason }).is_break() {
                            control.cancel();
                        }
                        continue;
                    }
                };
                if on_event(ResolveEvent::Hashing {
                    label: label.clone(),
                    round,
                    files: jobs.len() as u64,
                    bytes: jobs.iter().map(|j| j.tier.bytes(j.size)).sum(),
                })
                .is_break()
                {
                    control.cancel();
                }
                self.hash_location(
                    &pool,
                    &*source,
                    &label,
                    jobs,
                    options,
                    &control,
                    &mut on_event,
                    &mut summary,
                    &mut failed,
                )?;
                if control.is_cancelled() {
                    break;
                }
            }
            self.refresh()?;
        }
        summary.cancelled = control.is_cancelled();
        summary.unreachable.sort();
        summary.unreachable.dedup();
        Ok(summary)
    }

    #[expect(clippy::too_many_arguments, reason = "the state of one resolve round")]
    fn hash_location(
        &mut self,
        pool: &rayon::ThreadPool,
        source: &dyn crate::source::Source,
        label: &str,
        jobs: &[Job],
        options: &ScanOptions,
        control: &Control,
        on_event: &mut impl FnMut(ResolveEvent) -> ControlFlow<()>,
        summary: &mut ResolveSummary,
        failed: &mut HashSet<i64>,
    ) -> Result<(), CompareError> {
        let algo = self.sides[0].hash_algo;
        let mut batch: Vec<(&Job, Vec<u8>)> = Vec::with_capacity(BATCH);
        let (mut files, mut bytes) = (0_u64, 0_u64);
        let mut last_progress = Instant::now();
        let conn = &mut self.conn;
        hashing::run(
            pool,
            source,
            algo,
            jobs,
            control,
            options.progress_interval,
            |outcome| {
                match outcome {
                    Some(Outcome::Full(job, digest)) => {
                        batch.push((job, digest.as_bytes().to_vec()));
                    }
                    Some(Outcome::Partial(job, digest)) => {
                        batch.push((job, digest.as_bytes().to_vec()));
                    }
                    Some(Outcome::Warning(job, warning)) => {
                        failed.insert(job.id);
                        summary.warnings += 1;
                        let event = ResolveEvent::Warning {
                            label: label.to_owned(),
                            warning,
                        };
                        if on_event(event).is_break() {
                            control.cancel();
                        }
                    }
                    None => {}
                }
                if batch.len() >= BATCH || last_progress.elapsed() >= options.progress_interval {
                    let (stored_files, stored_bytes) = store(conn, &mut batch)?;
                    files += stored_files;
                    bytes += stored_bytes;
                    last_progress = Instant::now();
                    let event = ResolveEvent::Progress {
                        label: label.to_owned(),
                        files,
                        bytes,
                    };
                    if on_event(event).is_break() {
                        control.cancel();
                    }
                }
                Ok::<(), CompareError>(())
            },
        )?;
        let (stored_files, stored_bytes) = store(conn, &mut batch)?;
        summary.hashed_files += files + stored_files;
        summary.hashed_bytes += bytes + stored_bytes;
        Ok(())
    }

    fn label_of(&self, location_id: i64) -> String {
        self.sides
            .iter()
            .find(|side| side.location_id == location_id)
            .map_or_else(String::new, |side| side.side.label.clone())
    }

    /// The entries whose digests would decide more rows, excluding `failed` ones.
    fn plan(&self, failed: &HashSet<i64>) -> Result<Plan, CompareError> {
        let mut stmt = self.conn.prepare_cached(
            "WITH candidates AS (
                 SELECT f.side, f.entry_id, f.raw, f.size, f.mtime_ns, f.partial
                 FROM temp.cmp_files f JOIN temp.cmp_rows r ON r.id = f.row_id
                 WHERE r.category = 'unverified' AND f.full IS NULL
                 UNION
                 SELECT f.side, f.entry_id, f.raw, f.size, f.mtime_ns, f.partial
                 FROM temp.cmp_files f JOIN temp.cmp_rows r ON r.id = f.row_id
                 WHERE r.category = 'only_in' AND f.full IS NULL
                   AND EXISTS (
                       SELECT 1 FROM temp.cmp_files g JOIN temp.cmp_rows o ON o.id = g.row_id
                       WHERE g.size = f.size AND o.category = 'only_in'
                         AND (o.presence & r.presence) = 0
                         AND (f.partial IS NULL OR g.partial IS NULL OR g.partial = f.partial)
                   )
             )
             SELECT side, entry_id, raw, size, mtime_ns,
                    size <= :small OR partial IS NOT NULL AS full
             FROM candidates ORDER BY side, raw",
        )?;
        let rows = stmt.query_map(named_params! { ":small": 2 * PARTIAL_BLOCK }, |row| {
            Ok((
                row.get::<_, usize>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, bool>(5)?,
            ))
        })?;
        let mut jobs: BTreeMap<i64, Vec<Job>> = BTreeMap::new();
        let mut seen = HashSet::new();
        for row in rows {
            let (side_index, id, raw, size, mtime_ns, full) = row?;
            if failed.contains(&id) || !seen.insert(id) {
                continue;
            }
            let info = &self.sides[side_index];
            let path = RelPath::from_raw(info.path_encoding, raw)
                .map_err(|e| CatalogError::Corrupt(e.to_string()))?;
            jobs.entry(info.location_id).or_default().push(Job {
                id,
                path,
                size,
                mtime_ns,
                tier: if full { Tier::Full } else { Tier::Partial },
            });
        }
        Ok(Plan { jobs })
    }
}

/// Stores digests with the guarded statements; returns the files and bytes stored.
fn store(
    conn: &mut rusqlite::Connection,
    batch: &mut Vec<(&Job, Vec<u8>)>,
) -> Result<(u64, u64), CompareError> {
    if batch.is_empty() {
        return Ok((0, 0));
    }
    let tx = conn.transaction()?;
    let (mut files, mut bytes) = (0, 0);
    {
        let mut full = tx.prepare_cached(hashing::store_statement(Tier::Full))?;
        let mut partial = tx.prepare_cached(hashing::store_statement(Tier::Partial))?;
        for (job, digest) in batch.drain(..) {
            let stmt = match job.tier {
                Tier::Full => &mut full,
                Tier::Partial => &mut partial,
            };
            stmt.execute(params![digest, job.id, job.size, job.mtime_ns])?;
            files += 1;
            bytes += job.tier.bytes(job.size);
        }
    }
    tx.commit()?;
    Ok((files, bytes))
}
