//! `symbolon diff`.

use std::io::{self, BufWriter, Write};
use std::ops::ControlFlow;

use serde::Serialize;
use symbolon_core::compare::{
    Category, Cell, CompareOptions, Comparison, ResolveEvent, ResolveSummary, Row, RowFilter, Side,
    Summary,
};
use symbolon_core::hash::Digest;
use symbolon_core::pathkey::{KeyOptions, escape_raw};
use symbolon_core::query::Page;
use symbolon_core::scan::ScanOptions;
use symbolon_core::source::EntryKind;

use super::{follow, open_catalog};
use crate::cli::{DiffArgs, Global};
use crate::exit::Exit;
use crate::interrupt;
use crate::output::{self, Format};
use crate::progress::Progress;

const PAGE: u32 = 1000;

pub(super) fn run(global: &Global, args: &DiffArgs) -> anyhow::Result<Exit> {
    let catalog = open_catalog(global)?;
    let sides = args
        .sides
        .iter()
        .map(|spec| spec.parse::<Side>())
        .collect::<Result<Vec<_>, _>>()?;
    let mut options = CompareOptions::default().keys(KeyOptions {
        nfc: args.nfc,
        case_insensitive: args.case_insensitive,
    });
    if let Some(tolerance) = args.fast {
        options = options.assume_equal_within(tolerance);
    }
    for side in &sides {
        follow::follow_moves(&catalog, &side.label, global.quiet)?;
    }
    let mut cmp = Comparison::new(&catalog, &sides, options)?;
    let mut interrupted = false;
    if !args.no_hash {
        interrupt::install()?;
        interrupted = resolve(&mut cmp, global.quiet, args.verbose)?.cancelled;
    }
    let summary = cmp.summary()?;
    let filter = filter(args);
    let mut out = BufWriter::new(io::stdout().lock());
    match global.format() {
        Format::Table => table(&mut out, &cmp, &summary, &filter, args.summary)?,
        Format::Json => json(&mut out, &cmp, &summary, &filter, args.summary)?,
        Format::Csv => csv(&mut out, &cmp, &summary, &filter, args.summary)?,
    }
    out.flush()?;
    Ok(if interrupted {
        Exit::Interrupted
    } else if differs(&summary) {
        Exit::Differences
    } else {
        Exit::Success
    })
}

/// Anything but identical rows counts as a difference, including unverified ones.
fn differs(summary: &Summary) -> bool {
    summary
        .categories
        .iter()
        .any(|(category, totals)| *category != Category::Identical && totals.entries > 0)
}

fn filter(args: &DiffArgs) -> RowFilter {
    let mut filter = if !args.only.is_empty() {
        RowFilter::categories(args.only.iter().map(|&c| Category::from(c)))
    } else if args.all {
        RowFilter::default()
    } else {
        RowFilter::categories(
            Category::ALL
                .into_iter()
                .filter(|&c| c != Category::Identical),
        )
    };
    if let Some(ext) = &args.ext {
        filter = filter.extension(ext);
    }
    filter = filter.size_range(args.min_size, args.max_size);
    if let Some(prefix) = &args.under {
        filter = filter.prefix(prefix);
    }
    if !args.kind.is_empty() {
        filter = filter.kinds(args.kind.iter().map(|&k| EntryKind::from(k)));
    }
    filter
}

/// Hashes what the comparison needs, with progress on stderr; Ctrl-C stops it, keeping what
/// was hashed.
pub(super) fn resolve(
    cmp: &mut Comparison,
    quiet: bool,
    verbose: bool,
) -> anyhow::Result<ResolveSummary> {
    let mut progress = Progress::new(quiet);
    let mut total = (0, 0);
    let summary = cmp.resolve(&ScanOptions::default(), |event| {
        match event {
            ResolveEvent::Hashing {
                label,
                files,
                bytes,
                ..
            } => {
                total = (files, bytes);
                progress.show(&format!(
                    "{label}: hashing {files} files, {}",
                    output::size(bytes)
                ));
            }
            ResolveEvent::Progress {
                label,
                files,
                bytes,
            } => progress.show(&format!(
                "{label}: hashing {files}/{} files, {} of {}",
                total.0,
                output::size(bytes),
                output::size(total.1)
            )),
            ResolveEvent::Warning { label, warning } if verbose && !quiet => {
                progress.clear();
                let path = warning
                    .path
                    .as_ref()
                    .map_or_else(|| label.clone(), |p| format!("{label}:{}", p.key()));
                eprintln!(
                    "warning: {}: {} ({})",
                    output::printable(&path),
                    output::printable(&warning.message),
                    warning.kind
                );
            }
            ResolveEvent::Unreachable { label, reason } if !quiet => {
                progress.clear();
                eprintln!("warning: {label} cannot be read ({reason}); its rows stay unverified");
            }
            _ => {}
        }
        if interrupt::requested() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })?;
    progress.clear();
    if !quiet && !verbose && summary.warnings > 0 {
        eprintln!(
            "{} files could not be hashed and their rows stay unverified; --verbose lists them",
            summary.warnings
        );
    }
    Ok(summary)
}

/// Calls `f` with every row matching `filter`, a page at a time.
fn for_each_row(
    cmp: &Comparison,
    filter: &RowFilter,
    mut f: impl FnMut(&Row) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut page = Page::first(PAGE);
    loop {
        let result = cmp.rows(filter, &page)?;
        for row in &result.items {
            f(row)?;
        }
        match result.next {
            Some(cursor) => page = Page::after(cursor, PAGE),
            None => return Ok(()),
        }
    }
}

/// A cell as the table shows it: the size of a file, `dir`, or a symlink's target.
fn cell_text(cell: &Cell) -> String {
    match cell.kind {
        EntryKind::Dir => "dir".to_owned(),
        EntryKind::Symlink => cell.link_target.as_deref().map_or_else(
            || "-> ?".to_owned(),
            |target| format!("-> {}", escape_raw(target)),
        ),
        EntryKind::File | EntryKind::Other => output::size(cell.size),
    }
}

fn category_label(row: &Row) -> String {
    if row.assumed {
        format!("{} (assumed)", row.category)
    } else {
        row.category.to_string()
    }
}

fn table(
    out: &mut impl Write,
    cmp: &Comparison,
    summary: &Summary,
    filter: &RowFilter,
    summary_only: bool,
) -> anyhow::Result<()> {
    let totals = Category::ALL
        .into_iter()
        .filter_map(|category| {
            let t = summary.get(category);
            (t.rows > 0).then(|| {
                vec![
                    category.to_string(),
                    t.rows.to_string(),
                    t.entries.to_string(),
                    output::size(t.bytes),
                ]
            })
        })
        .collect::<Vec<_>>();
    output::table(
        out,
        &["CATEGORY", "ROWS", "ENTRIES", "SIZE"],
        totals.into_iter(),
    )?;
    if summary_only {
        return Ok(());
    }
    let mut headers = vec!["CATEGORY".to_owned(), "PATH".to_owned()];
    headers.extend(cmp.sides().iter().map(|info| info.side.to_string()));
    let mut rows = Vec::new();
    for_each_row(cmp, filter, |row| {
        let path = if row.cells.iter().all(|cell| cell.kind == EntryKind::Dir) {
            format!("{}/", row.key)
        } else {
            row.key.clone()
        };
        let mut cells = vec![category_label(row), path];
        cells.extend((0..cmp.sides().len()).map(|side| {
            let files: Vec<String> = row
                .cells
                .iter()
                .filter(|cell| cell.side == side)
                .map(|cell| {
                    if cell.key.as_str() == row.key {
                        cell_text(cell)
                    } else {
                        format!("{} ({})", cell.key, cell_text(cell))
                    }
                })
                .collect();
            if files.is_empty() {
                "-".to_owned()
            } else {
                files.join(", ")
            }
        }));
        rows.push(cells);
        Ok(())
    })?;
    if !rows.is_empty() {
        writeln!(out)?;
        output::table(out, &headers, rows.into_iter())?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct SideRecord {
    side: String,
    label: String,
    subpath: String,
    hash_algo: String,
}

#[derive(Debug, Serialize)]
struct TotalsRecord {
    rows: u64,
    entries: u64,
    bytes: u64,
    assumed_rows: u64,
}

/// Every category in order, zeros included.
struct SummaryRecord<'a>(&'a Summary);

impl Serialize for SummaryRecord<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(Category::ALL.len()))?;
        for category in Category::ALL {
            let t = self.0.get(category);
            map.serialize_entry(
                category.name(),
                &TotalsRecord {
                    rows: t.rows,
                    entries: t.entries,
                    bytes: t.bytes,
                    assumed_rows: t.assumed_rows,
                },
            )?;
        }
        map.end()
    }
}

#[derive(Debug, Serialize)]
struct RowRecord<'a> {
    category: &'static str,
    key: &'a str,
    assumed: bool,
    size: Option<u64>,
    entries: Vec<EntryRecord>,
}

#[derive(Debug, Serialize)]
struct EntryRecord {
    side: String,
    /// Path key within the location (not relative to the side's folder).
    path: String,
    kind: &'static str,
    size: u64,
    digest: Option<String>,
    /// A symlink's target, escaped like a path key.
    target: Option<String>,
}

impl<'a> RowRecord<'a> {
    fn new(row: &'a Row, sides: &[String]) -> Self {
        Self {
            category: row.category.name(),
            key: &row.key,
            assumed: row.assumed,
            size: row.size,
            entries: row
                .cells
                .iter()
                .map(|cell| EntryRecord {
                    side: sides[cell.side].clone(),
                    path: cell.path.key().as_str().to_owned(),
                    kind: cell.kind.name(),
                    size: cell.size,
                    digest: cell.digest.as_ref().map(Digest::to_hex),
                    target: cell.link_target.as_deref().map(escape_raw),
                })
                .collect(),
        }
    }
}

fn side_names(cmp: &Comparison) -> Vec<String> {
    cmp.sides()
        .iter()
        .map(|info| info.side.to_string())
        .collect()
}

/// Pretty JSON nested `depth` levels deep.
fn nested(value: &impl Serialize, depth: usize) -> serde_json::Result<String> {
    let text = serde_json::to_string_pretty(value)?;
    Ok(text.replace('\n', &format!("\n{}", "  ".repeat(depth))))
}

/// One document, rows written page by page so large comparisons never sit in memory.
fn json(
    out: &mut impl Write,
    cmp: &Comparison,
    summary: &Summary,
    filter: &RowFilter,
    summary_only: bool,
) -> anyhow::Result<()> {
    let sides: Vec<SideRecord> = cmp
        .sides()
        .iter()
        .map(|info| SideRecord {
            side: info.side.to_string(),
            label: info.side.label.clone(),
            subpath: info.side.subpath.clone(),
            hash_algo: info.hash_algo.to_string(),
        })
        .collect();
    write!(out, "{{\n  \"sides\": {}", nested(&sides, 1)?)?;
    write!(
        out,
        ",\n  \"summary\": {}",
        nested(&SummaryRecord(summary), 1)?
    )?;
    if !summary_only {
        let names = side_names(cmp);
        write!(out, ",\n  \"rows\": [")?;
        let mut first = true;
        for_each_row(cmp, filter, |row| {
            let separator = if first { "\n    " } else { ",\n    " };
            first = false;
            write!(
                out,
                "{separator}{}",
                nested(&RowRecord::new(row, &names), 2)?
            )?;
            Ok(())
        })?;
        write!(out, "{}]", if first { "" } else { "\n  " })?;
    }
    writeln!(out, "\n}}")?;
    Ok(())
}

/// One line per entry; `row` numbers the rows so entries of one row can be grouped.
fn csv(
    out: &mut impl Write,
    cmp: &Comparison,
    summary: &Summary,
    filter: &RowFilter,
    summary_only: bool,
) -> anyhow::Result<()> {
    let mut writer = csv::Writer::from_writer(out);
    if summary_only {
        writer.write_record(["category", "rows", "entries", "bytes", "assumed_rows"])?;
        for category in Category::ALL {
            let t = summary.get(category);
            writer.write_record([
                category.name().to_owned(),
                t.rows.to_string(),
                t.entries.to_string(),
                t.bytes.to_string(),
                t.assumed_rows.to_string(),
            ])?;
        }
        writer.flush()?;
        return Ok(());
    }
    let names = side_names(cmp);
    writer.write_record([
        "row", "category", "assumed", "side", "path", "kind", "size", "digest", "target",
    ])?;
    let mut number = 0_u64;
    for_each_row(cmp, filter, |row| {
        number += 1;
        for cell in &row.cells {
            writer.write_record([
                number.to_string(),
                row.category.name().to_owned(),
                row.assumed.to_string(),
                names[cell.side].clone(),
                cell.path.key().as_str().to_owned(),
                cell.kind.name().to_owned(),
                cell.size.to_string(),
                cell.digest.as_ref().map_or_default(Digest::to_hex),
                cell.link_target.as_deref().map_or_default(escape_raw),
            ])?;
        }
        Ok(())
    })?;
    writer.flush()?;
    Ok(())
}
