//! `symbolon scan`.

use std::time::Duration;

use crossbeam_channel::RecvTimeoutError;
use serde::Serialize;
use symbolon_core::catalog::{Catalog, LocationRoot};
use symbolon_core::query::{Page, Query};
use symbolon_core::scan::{
    ScanEvent, ScanId, ScanOptions, ScanPhase, ScanProgress, ScanStatus, ScanSummary, Scanner,
};

use super::{follow, open_catalog};
use crate::cli::{Global, ScanArgs};
use crate::exit::{Exit, Refused};
use crate::interrupt;
use crate::output::{self, Record};
use crate::progress::Progress;

pub(super) fn run(global: &Global, args: &ScanArgs) -> anyhow::Result<Exit> {
    let catalog = open_catalog(global)?;
    let labels = if args.all {
        catalog
            .locations()?
            .into_iter()
            .filter(|location| {
                matches!(
                    location.root,
                    LocationRoot::Local(_) | LocationRoot::Android { .. }
                )
            })
            .map(|location| location.label)
            .collect()
    } else {
        // Every label is checked before anything is scanned.
        for label in &args.labels {
            catalog.location(label)?;
        }
        args.labels.clone()
    };
    // Devices are followed or confirmed for every location before the first scan starts.
    let mut ready = Vec::new();
    let mut refusals = Vec::new();
    for label in labels {
        match follow::prepare(&catalog, &label, args.yes, args.all, global.quiet)? {
            follow::Ready::Yes => ready.push(label),
            follow::Ready::Skip(reason) if args.all => {
                if !global.quiet {
                    eprintln!("{reason}");
                }
            }
            follow::Ready::Skip(reason) => refusals.push(reason),
        }
    }
    if !refusals.is_empty() {
        return Err(Refused(refusals.join("\n")).into());
    }
    let labels = ready;
    interrupt::install()?;
    let mut progress = Progress::new(global.quiet);
    let mut records = Vec::new();
    let mut exit = Exit::Success;
    for label in &labels {
        let summary = scan_one(&catalog, label, args.full, global.quiet, &mut progress)?;
        if !global.quiet {
            report_warnings(&catalog, label, &summary, args.verbose)?;
        }
        records.push(ScanRecord::new(label, &summary));
        if summary.status == ScanStatus::Cancelled {
            exit = Exit::Interrupted;
            break;
        }
    }
    output::list(global.format(), "scans", &records)?;
    Ok(exit)
}

fn scan_one(
    catalog: &Catalog,
    label: &str,
    full: bool,
    quiet: bool,
    progress: &mut Progress,
) -> anyhow::Result<ScanSummary> {
    let handle = Scanner::start(catalog, label, ScanOptions::default().hash_all(full))?;
    loop {
        if interrupt::requested() {
            handle.cancel();
        }
        match handle.events().recv_timeout(Duration::from_millis(100)) {
            Ok(ScanEvent::Progress(update)) => progress.show(&describe(label, &update)),
            Ok(ScanEvent::Note(note)) => {
                progress.clear();
                if !quiet {
                    eprintln!("{label}: {note}");
                }
            }
            Ok(ScanEvent::Finished(_)) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
        }
    }
    progress.clear();
    Ok(handle.wait()?)
}

fn describe(label: &str, update: &ScanProgress) -> String {
    let counts = &update.counts;
    match update.phase {
        ScanPhase::Walking => format!(
            "{label}: listing, {} files, {}",
            counts.files,
            output::size(counts.bytes)
        ),
        ScanPhase::Hashing => {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a display rate; rounding is fine"
            )]
            let rate = update.bytes_per_sec.max(0.0) as u64;
            format!(
                "{label}: hashing {}/{} files, {} of {}, {}/s",
                counts.hashed_files,
                update.to_hash_files,
                output::size(counts.hashed_bytes),
                output::size(update.to_hash_bytes),
                output::size(rate)
            )
        }
    }
}

/// Warnings come from the catalog: the event stream may have dropped some.
fn report_warnings(
    catalog: &Catalog,
    label: &str,
    summary: &ScanSummary,
    verbose: bool,
) -> anyhow::Result<()> {
    let count = summary.counts.warnings;
    if count == 0 {
        return Ok(());
    }
    if !verbose {
        eprintln!(
            "{label}: {}; run with --verbose to list them",
            output::count(count, "warning")
        );
        return Ok(());
    }
    let query = Query::open(catalog)?;
    for_each_warning(&query, summary.scan_id, |warning| {
        let path = warning
            .path
            .as_ref()
            .map_or_else(|| label.to_owned(), |p| format!("{label}:{}", p.key()));
        eprintln!(
            "warning: {}: {} ({})",
            output::printable(&path),
            output::printable(&warning.message),
            warning.kind
        );
    })
}

fn for_each_warning(
    query: &Query,
    scan: ScanId,
    mut f: impl FnMut(&symbolon_core::query::WarningInfo),
) -> anyhow::Result<()> {
    let mut page = Page::first(1000);
    loop {
        let result = query.warnings(scan, &page)?;
        result.items.iter().for_each(&mut f);
        match result.next {
            Some(cursor) => page = Page::after(cursor, 1000),
            None => return Ok(()),
        }
    }
}

/// One finished scan.
#[derive(Debug, Serialize)]
struct ScanRecord {
    label: String,
    status: String,
    files: u64,
    dirs: u64,
    symlinks: u64,
    bytes: u64,
    hashed_files: u64,
    hashed_bytes: u64,
    reused_hashes: u64,
    warnings: u64,
    elapsed_ms: u64,
}

impl ScanRecord {
    fn new(label: &str, summary: &ScanSummary) -> Self {
        let c = &summary.counts;
        Self {
            label: label.to_owned(),
            status: summary.status.to_string(),
            files: c.files,
            dirs: c.dirs,
            symlinks: c.symlinks,
            bytes: c.bytes,
            hashed_files: c.hashed_files,
            hashed_bytes: c.hashed_bytes,
            reused_hashes: c.reused_hashes,
            warnings: c.warnings,
            elapsed_ms: u64::try_from(summary.elapsed.as_millis()).unwrap_or(u64::MAX),
        }
    }
}

impl Record for ScanRecord {
    const FIELDS: &'static [&'static str] = &[
        "label",
        "status",
        "files",
        "dirs",
        "symlinks",
        "bytes",
        "hashed_files",
        "hashed_bytes",
        "reused_hashes",
        "warnings",
        "elapsed_ms",
    ];
    const HEADERS: &'static [&'static str] = &[
        "LABEL", "STATUS", "FILES", "SIZE", "HASHED", "REUSED", "WARNINGS", "TIME",
    ];

    fn cells(&self) -> Vec<String> {
        vec![
            self.label.clone(),
            self.status.clone(),
            self.files.to_string(),
            output::size(self.bytes),
            format!(
                "{} ({})",
                self.hashed_files,
                output::size(self.hashed_bytes)
            ),
            self.reused_hashes.to_string(),
            self.warnings.to_string(),
            format!(
                "{:.1} s",
                Duration::from_millis(self.elapsed_ms).as_secs_f64()
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_match_the_record() {
        output::assert_fields(&ScanRecord {
            label: String::new(),
            status: String::new(),
            files: 0,
            dirs: 0,
            symlinks: 0,
            bytes: 0,
            hashed_files: 0,
            hashed_bytes: 0,
            reused_hashes: 0,
            warnings: 0,
            elapsed_ms: 0,
        });
    }
}
