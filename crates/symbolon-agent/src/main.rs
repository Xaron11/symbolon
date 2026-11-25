//! `symbolon-agent`: the scanner pushed to an Android phone and run through `adb exec-out`
//! (ADR-0003, ADR-0017). It walks and hashes on the device with `symbolon-core` and writes
//! [`Line`]s to stdout, so only the index travels to the host.

use std::fs;
use std::io::{self, BufWriter, Write as _};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use symbolon_core::agent::{Line, Request};
use symbolon_core::hash::HashAlgo;
use symbolon_core::scan::hash_requests;
use symbolon_core::source::{LocalSource, Source as _, WalkItem};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Parser)]
#[command(name = "symbolon-agent", version, about)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List every entry under ROOT without reading contents
    Walk {
        /// Directory to list
        root: PathBuf,
        /// Skip paths matching a pattern in .gitignore syntax; repeatable
        #[arg(long = "ignore", value_name = "PATTERN")]
        ignore: Vec<String>,
    },
    /// Hash the files listed in a request file, one JSON request per line
    Hash {
        /// Directory the requested paths are relative to
        root: PathBuf,
        /// Hash algorithm
        #[arg(long)]
        algo: HashAlgo,
        /// Hashing threads; on phone storage more than two are slower (ADR-0003)
        #[arg(long, default_value = "2")]
        threads: NonZeroUsize,
        /// The request file
        #[arg(long)]
        requests: PathBuf,
    },
}

/// Lines go to stdout, buffered; a broken pipe means the host is gone and ends the run.
struct Out(BufWriter<io::Stdout>);

impl Out {
    fn line(&mut self, line: &Line) -> io::Result<()> {
        serde_json::to_writer(&mut self.0, line)?;
        self.0.write_all(b"\n")
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    let mut out = Out(BufWriter::new(io::stdout()));
    let result = out
        .line(&Line::hello())
        .map_err(anyhow::Error::from)
        .and_then(|()| run(&args.command, &mut out));
    let end = match &result {
        Ok(count) => Line::Done { count: *count },
        Err(error) => Line::Error {
            message: format!("{error:#}"),
        },
    };
    let written = out.line(&end).and_then(|()| out.0.flush());
    if result.is_ok() && written.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(command: &Command, out: &mut Out) -> anyhow::Result<u64> {
    match command {
        Command::Walk { root, ignore } => walk(root, ignore, out),
        Command::Hash {
            root,
            algo,
            threads,
            requests,
        } => hash(root, *algo, *threads, requests, out),
    }
}

fn walk(root: &PathBuf, ignore: &[String], out: &mut Out) -> anyhow::Result<u64> {
    let source = LocalSource::new(root, ignore)?;
    let mut count = 0;
    let mut written = Ok(());
    source.walk(&mut |item| {
        let line = match &item {
            WalkItem::Entry(entry) => {
                count += 1;
                Line::entry(entry)
            }
            WalkItem::Warning(warning) => Line::warning(None, warning),
        };
        written = out.line(&line);
        if written.is_ok() {
            std::ops::ControlFlow::Continue(())
        } else {
            std::ops::ControlFlow::Break(())
        }
    })?;
    written?;
    Ok(count)
}

fn hash(
    root: &PathBuf,
    algo: HashAlgo,
    threads: NonZeroUsize,
    requests: &PathBuf,
    out: &mut Out,
) -> anyhow::Result<u64> {
    let text = fs::read_to_string(requests)
        .with_context(|| format!("cannot read the request file {}", requests.display()))?;
    let requests = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str::<Request>(line)
                .ok()
                .and_then(|request| request.to_hash_request())
                .with_context(|| format!("invalid request {line:?}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let source = LocalSource::new(root, &[])?;
    let mut count = 0;
    let mut written = Ok(());
    hash_requests(&source, algo, &requests, threads, |index, reply| {
        count += 1;
        if written.is_ok() {
            written = out.line(&Line::reply(index, &reply));
            if written.is_ok() {
                // Each digest can take a while; let the host see progress.
                written = out.0.flush();
            }
        }
    })?;
    written?;
    Ok(count)
}
