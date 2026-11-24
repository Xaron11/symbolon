//! PoC: M0 spike probe for running a static binary on Android over `adb exec-out`.
//! It reports the execution environment, then walks and hashes a tree, streaming NDJSON.
//! Follow-up (M5): replace with the real scanner built on `symbolon-core`.

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, UNIX_EPOCH};

use anyhow::Context;
use clap::Parser;
use rayon::prelude::*;
use serde::Serialize;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Parser)]
#[command(name = "symbolon-agent", version)]
struct Args {
    /// Directory to walk and hash.
    root: PathBuf,
    /// Worker threads for hashing (default: available parallelism).
    #[arg(long)]
    threads: Option<usize>,
    /// Emit one event per hashed file instead of only the summary.
    #[arg(long)]
    emit_files: bool,
    /// Only walk, do not read file contents.
    #[arg(long)]
    walk_only: bool,
}

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Event<'a> {
    Env {
        version: &'a str,
        arch: &'a str,
        os: &'a str,
        parallelism: usize,
        threads: usize,
        exe: Option<String>,
        uid_line: Option<String>,
        selinux_context: Option<String>,
    },
    Walked {
        files: u64,
        dirs: u64,
        symlinks: u64,
        bytes: u64,
        elapsed_ms: u128,
    },
    File {
        path: String,
        size: u64,
        mtime_ns: Option<i128>,
        blake3: String,
    },
    Warning {
        path: String,
        error: String,
    },
    Finished {
        files: u64,
        bytes: u64,
        warnings: u64,
        elapsed_ms: u128,
        mib_per_s: f64,
    },
}

struct Emitter {
    out: Mutex<BufWriter<io::Stdout>>,
    warnings: AtomicU64,
}

impl Emitter {
    fn emit(&self, event: &Event<'_>) {
        if let Ok(mut out) = self.out.lock() {
            // A broken pipe means the host went away; nothing useful to do with the error.
            let _ = serde_json::to_writer(&mut *out, event).map(|()| out.write_all(b"\n"));
        }
    }

    fn warn(&self, path: &Path, error: impl ToString) {
        self.warnings.fetch_add(1, Ordering::Relaxed);
        self.emit(&Event::Warning {
            path: path.to_string_lossy().into_owned(),
            error: error.to_string(),
        });
    }
}

struct FileEntry {
    path: PathBuf,
    size: u64,
    mtime_ns: Option<i128>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let parallelism = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads = args.threads.unwrap_or(parallelism);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .context("building thread pool")?;

    let emitter = Emitter {
        out: Mutex::new(BufWriter::new(io::stdout())),
        warnings: AtomicU64::new(0),
    };
    emitter.emit(&Event::Env {
        version: env!("CARGO_PKG_VERSION"),
        arch: std::env::consts::ARCH,
        os: std::env::consts::OS,
        parallelism,
        threads,
        exe: std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().into_owned()),
        uid_line: fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| s.lines().find(|l| l.starts_with("Uid:")).map(str::to_owned)),
        selinux_context: fs::read_to_string("/proc/self/attr/current")
            .ok()
            .map(|s| s.trim_end_matches(['\0', '\n']).to_owned()),
    });

    let start = Instant::now();
    let mut files = Vec::new();
    let (mut dirs, mut symlinks) = (0u64, 0u64);
    let mut stack = vec![args.root.clone()];
    while let Some(dir) = stack.pop() {
        dirs += 1;
        let read = match fs::read_dir(&dir) {
            Ok(read) => read,
            Err(e) => {
                emitter.warn(&dir, e);
                continue;
            }
        };
        for entry in read {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    emitter.warn(&dir, e);
                    continue;
                }
            };
            let path = entry.path();
            // symlink_metadata: never follow links, so cycles are impossible.
            let meta = match fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(e) => {
                    emitter.warn(&path, e);
                    continue;
                }
            };
            let kind = meta.file_type();
            if kind.is_symlink() {
                symlinks += 1;
            } else if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                let mtime_ns = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as i128);
                files.push(FileEntry {
                    path,
                    size: meta.len(),
                    mtime_ns,
                });
            }
        }
    }
    let walked_bytes: u64 = files.iter().map(|f| f.size).sum();
    emitter.emit(&Event::Walked {
        files: files.len() as u64,
        dirs,
        symlinks,
        bytes: walked_bytes,
        elapsed_ms: start.elapsed().as_millis(),
    });

    let hashed_bytes = AtomicU64::new(0);
    let hashed_files = AtomicU64::new(0);
    if !args.walk_only {
        files.par_iter().for_each(|f| match hash_file(&f.path) {
            Ok((hash, len)) => {
                hashed_bytes.fetch_add(len, Ordering::Relaxed);
                hashed_files.fetch_add(1, Ordering::Relaxed);
                if args.emit_files {
                    emitter.emit(&Event::File {
                        path: f.path.to_string_lossy().into_owned(),
                        size: f.size,
                        mtime_ns: f.mtime_ns,
                        blake3: hash.to_hex().to_string(),
                    });
                }
            }
            Err(e) => emitter.warn(&f.path, e),
        });
    }

    let elapsed = start.elapsed();
    let bytes = hashed_bytes.load(Ordering::Relaxed);
    emitter.emit(&Event::Finished {
        files: hashed_files.load(Ordering::Relaxed),
        bytes,
        warnings: emitter.warnings.load(Ordering::Relaxed),
        elapsed_ms: elapsed.as_millis(),
        mib_per_s: bytes as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64().max(1e-9),
    });
    emitter
        .out
        .into_inner()
        .map_err(|_| anyhow::anyhow!("output lock poisoned"))?
        .flush()
        .context("flushing output")?;
    Ok(())
}

fn hash_file(path: &Path) -> io::Result<(blake3::Hash, u64)> {
    let mut hasher = blake3::Hasher::new();
    let file = fs::File::open(path)?;
    hasher.update_reader(file)?;
    Ok((hasher.finalize(), hasher.count()))
}
