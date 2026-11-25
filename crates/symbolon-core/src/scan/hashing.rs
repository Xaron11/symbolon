//! Hashing selected entries in parallel: shared by scans and by comparisons resolving what they
//! need. Workers read through a [`Source`]; results reach the caller's thread, which is the only
//! writer.

use std::time::Duration;

use crossbeam_channel::RecvTimeoutError;
use rayon::prelude::*;

use std::num::NonZeroUsize;
use std::ops::ControlFlow;

use super::{Control, ScanError};
use crate::hash::{
    Digest, HashAlgo, HashError, PARTIAL_BLOCK, PartialDigest, hash_reader, partial_hash,
};
use crate::pathkey::RelPath;
use crate::source::{HashReply, HashRequest, RemoteHasher, Source, SourceWarning, WarningKind};

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

/// Hashes `jobs` on `pool`, or on the source's side when it has a [`RemoteHasher`], calling
/// `handle` on this thread with every outcome, and with `None` at least every `tick` (for
/// progress and batching). Returns when all jobs are done or cancelled, or on the first error
/// from `handle`.
pub(crate) fn run<'j, E>(
    pool: &rayon::ThreadPool,
    source: &dyn Source,
    algo: HashAlgo,
    jobs: &'j [Job],
    control: &Control,
    tick: Duration,
    handle: impl FnMut(Option<Outcome<'j>>) -> Result<(), E>,
) -> Result<(), E> {
    match source.remote_hasher() {
        Some(remote) => run_remote(remote, algo, jobs, control, tick, handle),
        None => run_local(pool, source, algo, jobs, control, tick, handle),
    }
}

fn run_local<'j, E>(
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

/// Sends the jobs to the source's hasher. A partial job becomes a full request when the source
/// has no partial digests, and its outcome is then [`Outcome::Full`]. Jobs the source never
/// answers (it failed or was unplugged) become warnings.
fn run_remote<'j, E>(
    remote: &dyn RemoteHasher,
    algo: HashAlgo,
    jobs: &'j [Job],
    control: &Control,
    tick: Duration,
    mut handle: impl FnMut(Option<Outcome<'j>>) -> Result<(), E>,
) -> Result<(), E> {
    if !remote.supports(algo) {
        let message = format!("this source cannot compute {algo} digests");
        for job in jobs {
            handle(Some(Outcome::Warning(
                job,
                SourceWarning {
                    path: Some(job.path.clone()),
                    kind: WarningKind::Io,
                    message: message.clone(),
                },
            )))?;
        }
        return Ok(());
    }
    let partial = remote.partial_digests();
    let requests: Vec<HashRequest> = jobs
        .iter()
        .map(|job| HashRequest {
            path: job.path.clone(),
            size: job.size,
            mtime_ns: job.mtime_ns,
            partial: partial && job.tier == Tier::Partial,
        })
        .collect();
    let (sender, results) = crossbeam_channel::bounded::<(usize, HashReply)>(512);
    let mut answered = vec![false; jobs.len()];
    let failure = std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            remote.hash(algo, &requests, &mut |index, reply| {
                if control.proceed() && sender.send((index, reply)).is_ok() {
                    ControlFlow::Continue(())
                } else {
                    ControlFlow::Break(())
                }
            })
        });
        loop {
            match results.recv_timeout(tick) {
                Ok((index, reply)) => {
                    let Some(job) = jobs.get(index) else { continue };
                    if let Some(done) = answered.get_mut(index) {
                        *done = true;
                    }
                    handle(Some(match reply {
                        HashReply::Full(digest) => Outcome::Full(job, digest),
                        HashReply::Partial(digest) => Outcome::Partial(job, digest),
                        HashReply::Warning(warning) => Outcome::Warning(job, warning),
                    }))?;
                }
                Err(RecvTimeoutError::Timeout) => handle(None)?,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        Ok(worker.join().ok().and_then(Result::err))
    })?;
    if let Some(error) = failure
        && !control.is_cancelled()
    {
        let message = error.to_string();
        for (job, _) in jobs.iter().zip(&answered).filter(|(_, done)| !**done) {
            handle(Some(Outcome::Warning(
                job,
                SourceWarning {
                    path: Some(job.path.clone()),
                    kind: WarningKind::Io,
                    message: message.clone(),
                },
            )))?;
        }
    }
    Ok(())
}

/// Hashes `requests` in `source` on `threads` threads with exactly the checks scans use,
/// calling `each` with each request's index and reply as they finish. This is what a remote
/// source runs on its side (the Android agent, ADR-0017).
///
/// # Errors
///
/// [`ScanError::Threads`] if the threads cannot be started.
pub fn hash_requests(
    source: &dyn Source,
    algo: HashAlgo,
    requests: &[HashRequest],
    threads: NonZeroUsize,
    mut each: impl FnMut(usize, HashReply),
) -> Result<(), ScanError> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.get())
        .build()
        .map_err(|e| ScanError::Threads(e.to_string()))?;
    let jobs: Vec<Job> = requests
        .iter()
        .zip(0_i64..)
        .map(|(request, id)| Job {
            id,
            path: request.path.clone(),
            size: request.size,
            mtime_ns: request.mtime_ns,
            tier: if request.partial {
                Tier::Partial
            } else {
                Tier::Full
            },
        })
        .collect();
    let index = |job: &Job| usize::try_from(job.id).unwrap_or(usize::MAX);
    run_local(
        &pool,
        source,
        algo,
        &jobs,
        &Control::default(),
        Duration::from_secs(1),
        |outcome| {
            let reply = match outcome {
                Some(Outcome::Full(job, digest)) => Some((job, HashReply::Full(digest))),
                Some(Outcome::Partial(job, digest)) => Some((job, HashReply::Partial(digest))),
                Some(Outcome::Warning(job, warning)) => Some((job, HashReply::Warning(warning))),
                None => None,
            };
            if let Some((job, reply)) = reply {
                each(index(job), reply);
            }
            Ok::<(), ScanError>(())
        },
    )
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

/// The guarded statement storing a digest of `tier`, with parameters digest, id, size,
/// mtime, raw path and algorithm name. It does nothing if the entry changed since it was
/// listed, or if the location's algorithm changed meanwhile (an import replacing its data).
pub(crate) const fn store_statement(tier: Tier) -> &'static str {
    match tier {
        Tier::Full => {
            "UPDATE main.entries SET hash_full = ?1 WHERE id = ?2 AND size = ?3 AND mtime_ns IS ?4 \
             AND rel_path_raw = ?5 \
             AND (SELECT hash_algo FROM main.locations WHERE id = location_id) = ?6"
        }
        Tier::Partial => {
            "UPDATE main.entries SET hash_partial = ?1 WHERE id = ?2 AND size = ?3 AND mtime_ns IS ?4 \
             AND rel_path_raw = ?5 \
             AND (SELECT hash_algo FROM main.locations WHERE id = location_id) = ?6"
        }
    }
}
