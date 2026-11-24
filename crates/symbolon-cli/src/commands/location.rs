//! `symbolon location add|list|remove`.

use std::io::{self, BufRead as _, IsTerminal as _, Write as _};

use serde::Serialize;
use symbolon_core::catalog::{Location, LocationRoot, NewLocation};
use symbolon_core::query::{Page, Query};

use super::open_catalog;
use crate::cli::{AddArgs, Global, LocationCommand};
use crate::exit::{Exit, Refused};
use crate::output::{self, Format, Record};

pub(super) fn run(global: &Global, command: &LocationCommand) -> anyhow::Result<Exit> {
    match command {
        LocationCommand::Add(args) => add(global, args),
        LocationCommand::List => list(global),
        LocationCommand::Remove { label, yes } => remove(global, label, *yes),
    }
}

/// A saved location with its last scan.
#[derive(Debug, Serialize)]
struct LocationRecord {
    label: String,
    root: String,
    /// `local`, `imported` (from a .symbolon file) or `other_platform` (a catalog moved
    /// between operating systems).
    kind: &'static str,
    /// Only local locations can be scanned; the others can be compared.
    scannable: bool,
    hash_policy: String,
    hash_algo: String,
    files: u64,
    bytes: u64,
    hashed_files: u64,
    last_scan_status: Option<String>,
    last_scan_started_at: Option<String>,
}

impl Record for LocationRecord {
    const FIELDS: &'static [&'static str] = &[
        "label",
        "root",
        "kind",
        "scannable",
        "hash_policy",
        "hash_algo",
        "files",
        "bytes",
        "hashed_files",
        "last_scan_status",
        "last_scan_started_at",
    ];
    const HEADERS: &'static [&'static str] = &[
        "LABEL",
        "ROOT",
        "POLICY",
        "ALGO",
        "FILES",
        "SIZE",
        "HASHED",
        "LAST SCAN",
    ];

    fn cells(&self) -> Vec<String> {
        let root = match self.kind {
            "imported" => format!("{} (imported)", self.root),
            "other_platform" => format!("{} (other platform)", self.root),
            _ => self.root.clone(),
        };
        let last_scan = match (&self.last_scan_status, &self.last_scan_started_at) {
            (Some(status), Some(at)) => format!("{status} {at}"),
            _ => "never".to_owned(),
        };
        vec![
            self.label.clone(),
            root,
            self.hash_policy.clone(),
            self.hash_algo.clone(),
            self.files.to_string(),
            output::size(self.bytes),
            self.hashed_files.to_string(),
            last_scan,
        ]
    }
}

fn record(query: &Query, location: &Location) -> anyhow::Result<LocationRecord> {
    let stats = query.location_stats(&location.label)?;
    let last = query
        .scans(&location.label, &Page::first(1))?
        .items
        .into_iter()
        .next();
    Ok(LocationRecord {
        label: location.label.clone(),
        root: location.root.display(),
        kind: match location.root {
            LocationRoot::Local(_) => "local",
            LocationRoot::Imported { .. } => "imported",
            _ => "other_platform",
        },
        scannable: matches!(location.root, LocationRoot::Local(_)),
        hash_policy: location.hash_policy.to_string(),
        hash_algo: location.hash_algo.to_string(),
        files: stats.files,
        bytes: stats.bytes,
        hashed_files: stats.verified_files,
        last_scan_status: last.as_ref().map(|scan| scan.status.to_string()),
        last_scan_started_at: last.map(|scan| output::time(scan.started_at)),
    })
}

fn add(global: &Global, args: &AddArgs) -> anyhow::Result<Exit> {
    let catalog = open_catalog(global)?;
    let mut new = NewLocation::local(&args.label, &args.path).ignore_patterns(args.ignore.clone());
    if let Some(policy) = args.policy {
        new = new.hash_policy(policy.into());
    }
    if let Some(algo) = args.algo {
        new = new.hash_algo(algo.into());
    }
    let location = catalog.add_location(new)?;
    let query = Query::open(&catalog)?;
    output::list(global.format(), "locations", &[record(&query, &location)?])?;
    Ok(Exit::Success)
}

fn list(global: &Global) -> anyhow::Result<Exit> {
    let catalog = open_catalog(global)?;
    let query = Query::open(&catalog)?;
    let records = catalog
        .locations()?
        .iter()
        .map(|location| record(&query, location))
        .collect::<anyhow::Result<Vec<_>>>()?;
    if records.is_empty() && global.format() == Format::Table {
        if !global.quiet {
            eprintln!("No locations yet. Add one with: symbolon location add <LABEL> <PATH>");
        }
        return Ok(Exit::Success);
    }
    output::list(global.format(), "locations", &records)?;
    Ok(Exit::Success)
}

#[derive(Debug, Serialize)]
struct Removed {
    label: String,
}

impl Record for Removed {
    const FIELDS: &'static [&'static str] = &["label"];
    const HEADERS: &'static [&'static str] = &["REMOVED"];

    fn cells(&self) -> Vec<String> {
        vec![self.label.clone()]
    }
}

fn remove(global: &Global, label: &str, yes: bool) -> anyhow::Result<Exit> {
    let catalog = open_catalog(global)?;
    catalog.location(label)?;
    if !yes && !confirm(label)? {
        return Ok(Exit::Refused);
    }
    catalog.remove_location(label)?;
    output::list(
        global.format(),
        "removed",
        &[Removed {
            label: label.to_owned(),
        }],
    )?;
    Ok(Exit::Success)
}

/// Asks on the terminal; without one, the user has to decide with `--yes` up front.
fn confirm(label: &str) -> anyhow::Result<bool> {
    if !(io::stdin().is_terminal() && io::stderr().is_terminal()) {
        return Err(Refused(format!(
            "removing location {label:?} needs confirmation; pass --yes to remove it"
        ))
        .into());
    }
    eprint!(
        "Remove location {label:?} and everything recorded about it? Its files are not touched. [y/N] "
    );
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_match_the_records() {
        output::assert_fields(&LocationRecord {
            label: String::new(),
            root: String::new(),
            kind: "local",
            scannable: true,
            hash_policy: String::new(),
            hash_algo: String::new(),
            files: 0,
            bytes: 0,
            hashed_files: 0,
            last_scan_status: None,
            last_scan_started_at: None,
        });
        output::assert_fields(&Removed {
            label: String::new(),
        });
    }
}
