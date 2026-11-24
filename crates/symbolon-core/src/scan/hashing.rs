//! Hashing selected entries in parallel: shared by scans and by comparisons resolving what they
//! need. Workers read through a [`Source`]; results reach the caller's thread, which is the only
//! writer.

use std::time::Duration;

use crossbeam_channel::RecvTimeoutError;
use rayon::prelude::*;

use super::Control;
use crate::hash::{
    Digest, HashAlgo, HashError, PARTIAL_BLOCK, PartialDigest, hash_reader, partial_hash,
};
use crate::pathkey::RelPath;
use crate::source::{Source, SourceWarning, WarningKind};

/// Which digest to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Tier {
    /// Size plus the first and last [`PARTIAL_BLOCK`] bytes.
    Partial,
    /// The whole content.
    Full,
}

impl Tier {
    /// Bytes this tier reads from a file of `size` bytes.
    pub(crate) fn bytes(self, size: u64) -> u64 {
        match self {
            Self::Partial => size.min(2 * PARTIAL_BLOCK),
            Self::Full => size,
        }
    }
}

/// One entry to hash, with the metadata it was listed with.
#[derive(Debug, Clone)]
pub(crate) struct Job {
    pub(crate) id: i64,
    pub(crate) path: RelPath,
    pub(crate) size: u64,
    pub(crate) mtime_ns: Option<i64>,
    pub(crate) tier: Tier,
}

pub(crate) enum Outcome<'a> {
    Full(&'a Job, Digest),
    Partial(&'a Job, PartialDigest),
    Warning(&'a Job, SourceWarning),
}

/// Hashes `jobs` on `pool`, calling `handle` on this thread with every outcome, and with `None`
/// at least every `tick` (for progress and batching). Returns when all jobs are done or
/// cancelled, or on the first error from `handle`.
pub(crate) fn run<'j, E>(
    pool: &rayon::ThreadPool,
    source: &dyn Source,
    algo: HashAlgo,
    jobs: &'j [Job],
    control: &Control,
    tick: Duration,
    mut handle: impl FnMut(Option<Outcome<'j>>) -> Result<(), E>,
) -> Result<(), E> {
    let (sender, results) = crossbeam_channel::bounded::<Outcome<'j>>(512);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            pool.install(|| {
                jobs.par_iter().for_each_with(sender, |sender, job| {
                    if control.proceed() {
                        // The receiver only goes away if the caller failed; stop quietly.
                        sender.send(hash_one(source, algo, job)).ok();
                    }
                });
            });
        });
        loop {
            match results.recv_timeout(tick) {
                Ok(outcome) => handle(Some(outcome))?,
                Err(RecvTimeoutError::Timeout) => handle(None)?,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
    })
}

/// Hashes one entry and re-checks it afterwards; any change turns the result into a warning.
fn hash_one<'a>(source: &dyn Source, algo: HashAlgo, job: &'a Job) -> Outcome<'a> {
    let warning = |kind, message: String| {
        Outcome::Warning(
            job,
            SourceWarning {
                path: Some(job.path.clone()),
                kind,
                message,
            },
        )
    };
    let io_warning =
        |e: &std::io::Error| Outcome::Warning(job, SourceWarning::io(Some(job.path.clone()), e));
    let reader = match source.open(&job.path) {
        Ok(reader) => reader,
        Err(e) => return io_warning(&e),
    };
    let result = match job.tier {
        Tier::Full => hash_reader(algo, reader, job.size).map(|d| Outcome::Full(job, d)),
        Tier::Partial => partial_hash(algo, reader, job.size).map(|d| Outcome::Partial(job, d)),
    };
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(HashError::SizeMismatch { expected, actual }) => {
            return warning(
                WarningKind::Changed,
                format!("size changed while hashing (listed {expected} bytes, read {actual})"),
            );
        }
        Err(HashError::Io(e)) => return io_warning(&e),
        Err(e) => return warning(WarningKind::Io, e.to_string()),
    };
    match source.stat(&job.path) {
        Ok(now) if now.size == job.size && now.mtime_ns == job.mtime_ns => outcome,
        Ok(_) => warning(
            WarningKind::Changed,
            "modified while hashing; hash discarded".to_owned(),
        ),
        Err(e) => io_warning(&e),
    }
}

/// The guarded statement storing a digest of `tier`: it does nothing if the entry changed since
/// it was listed.
pub(crate) const fn store_statement(tier: Tier) -> &'static str {
    match tier {
        Tier::Full => {
            "UPDATE main.entries SET hash_full = ?1 WHERE id = ?2 AND size = ?3 AND mtime_ns IS ?4"
        }
        Tier::Partial => {
            "UPDATE main.entries SET hash_partial = ?1 WHERE id = ?2 AND size = ?3 AND mtime_ns IS ?4"
        }
    }
}
