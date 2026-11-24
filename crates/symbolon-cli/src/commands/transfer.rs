//! `symbolon export` and `symbolon import` (ADR-0005, ADR-0013).

use serde::Serialize;
use symbolon_core::catalog::{ImportAction, ImportConflict, ImportOptions, TransferError};

use super::open_catalog;
use crate::cli::{ExportArgs, Global, ImportArgs};
use crate::exit::{Exit, Refused};
use crate::output::{self, Record};

pub(super) fn export(global: &Global, args: &ExportArgs) -> anyhow::Result<Exit> {
    if args.output.exists() && !args.force {
        return Err(Refused(format!(
            "{} exists; pass --force to replace it",
            args.output.display()
        ))
        .into());
    }
    let catalog = open_catalog(global)?;
    let summary = catalog.export(&args.label, &args.output)?;
    let record = TransferRecord {
        label: summary.label,
        action: "exported",
        entries: summary.entries,
        scanned_at: output::time(summary.scanned_at),
    };
    output::list(global.format(), "exports", &[record])?;
    Ok(Exit::Success)
}

pub(super) fn import(global: &Global, args: &ImportArgs) -> anyhow::Result<Exit> {
    let catalog = open_catalog(global)?;
    let mut options = ImportOptions::default()
        .replace(args.replace)
        .force(args.force);
    if let Some(label) = &args.as_label {
        options = options.as_label(label);
    }
    if let Some(label) = &args.into {
        options = options.merge_into(label);
    }
    let summary = match catalog.import(&args.file, &options) {
        Ok(summary) => summary,
        Err(TransferError::Conflict(conflict)) => return Err(Refused(choices(&conflict)).into()),
        Err(error) => return Err(error.into()),
    };
    let record = TransferRecord {
        label: summary.label,
        action: match summary.action {
            ImportAction::Added => "added",
            _ => "replaced",
        },
        entries: summary.entries,
        scanned_at: output::time(summary.scanned_at),
    };
    output::list(global.format(), "imports", &[record])?;
    Ok(Exit::Success)
}

/// The conflict with the flags that resolve it.
fn choices(conflict: &ImportConflict) -> String {
    match conflict {
        ImportConflict::OlderScan {
            label,
            imported,
            existing,
        } => format!(
            "location {label:?} was scanned at {}, after the file's scan at {}; pass --force to \
             replace its data with the older file anyway",
            output::time(*existing),
            output::time(*imported)
        ),
        ImportConflict::LabelTaken { label } => format!(
            "a location labelled {label:?} already exists for another device; import the file \
             as a new location (--as <LABEL>) or replace that location's data (--replace)"
        ),
        ImportConflict::SameDevice { existing, .. } => format!(
            "the file's device is already saved as {existing:?}; update that location \
             (--into {existing}) or import the file as a new location (--as <LABEL>)"
        ),
        other => other.to_string(),
    }
}

/// One exported or imported location.
#[derive(Debug, Serialize)]
struct TransferRecord {
    label: String,
    /// `exported`, `added` or `replaced`.
    action: &'static str,
    entries: u64,
    scanned_at: String,
}

impl Record for TransferRecord {
    const FIELDS: &'static [&'static str] = &["label", "action", "entries", "scanned_at"];
    const HEADERS: &'static [&'static str] = &["LABEL", "ACTION", "ENTRIES", "SCANNED"];

    fn cells(&self) -> Vec<String> {
        vec![
            self.label.clone(),
            self.action.to_owned(),
            self.entries.to_string(),
            self.scanned_at.clone(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_match_the_record() {
        output::assert_fields(&TransferRecord {
            label: String::new(),
            action: "added",
            entries: 0,
            scanned_at: String::new(),
        });
    }
}
