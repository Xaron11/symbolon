//! `symbolon dupes`.

use std::io::{self, BufWriter, Write as _};

use serde::Serialize;
use symbolon_core::compare::{CompareOptions, Comparison, DuplicateSet, Side};
use symbolon_core::query::Page;

use super::diff::resolve;
use super::open_catalog;
use crate::cli::{DupesArgs, Global};
use crate::exit::Exit;
use crate::interrupt;
use crate::output::{self, Format};

const PAGE: u32 = 1000;

pub(super) fn run(global: &Global, args: &DupesArgs) -> anyhow::Result<Exit> {
    let catalog = open_catalog(global)?;
    let sides = args
        .sides
        .iter()
        .map(|spec| spec.parse::<Side>())
        .collect::<Result<Vec<_>, _>>()?;
    let mut cmp = Comparison::new(&catalog, &sides, CompareOptions::default().duplicates(true))?;
    let mut interrupted = false;
    if !args.no_hash {
        interrupt::install()?;
        interrupted = resolve(&mut cmp, global.quiet, args.verbose)?.cancelled;
    }
    let names: Vec<String> = cmp.sides().iter().map(|i| i.side.to_string()).collect();
    let groups = groups(&cmp, &names, args.min_size.unwrap_or(0))?;
    let mut out = BufWriter::new(io::stdout().lock());
    match global.format() {
        Format::Table => table(&mut out, &groups, &names)?,
        Format::Json => {
            let totals: Vec<Totals> = names.iter().map(|side| Totals::of(side, &groups)).collect();
            serde_json::to_writer_pretty(
                &mut out,
                &Document {
                    totals,
                    groups: &groups,
                },
            )?;
            writeln!(out)?;
        }
        Format::Csv => {
            let mut writer = csv::Writer::from_writer(&mut out);
            writer.write_record(["group", "side", "digest", "size", "path"])?;
            for (number, group) in groups.iter().enumerate() {
                for path in &group.paths {
                    writer.write_record([
                        (number + 1).to_string(),
                        group.side.clone(),
                        group.digest.clone(),
                        group.size.to_string(),
                        path.clone(),
                    ])?;
                }
            }
            writer.flush()?;
        }
    }
    out.flush()?;
    Ok(if interrupted {
        Exit::Interrupted
    } else {
        Exit::Success
    })
}

/// One set of identical files within one side.
#[derive(Debug, Serialize)]
struct Group {
    side: String,
    digest: String,
    size: u64,
    copies: u64,
    /// Bytes that keeping one copy would free.
    redundant_bytes: u64,
    /// Path keys within the location.
    paths: Vec<String>,
}

impl Group {
    fn new(set: &DuplicateSet, names: &[String]) -> Self {
        Self {
            side: names[set.side].clone(),
            digest: set.digest.to_hex(),
            size: set.size,
            copies: set.cells.len() as u64,
            redundant_bytes: set.redundant_bytes(),
            paths: set
                .cells
                .iter()
                .map(|cell| cell.path.key().as_str().to_owned())
                .collect(),
        }
    }
}

/// Every group of at least `min_size` bytes, most redundant bytes first.
fn groups(cmp: &Comparison, names: &[String], min_size: u64) -> anyhow::Result<Vec<Group>> {
    let mut groups = Vec::new();
    for side in 0..names.len() {
        let mut page = Page::first(PAGE);
        loop {
            let result = cmp.duplicates(side, &page)?;
            groups.extend(
                result
                    .items
                    .iter()
                    .filter(|set| set.size >= min_size)
                    .map(|set| Group::new(set, names)),
            );
            match result.next {
                Some(cursor) => page = Page::after(cursor, PAGE),
                None => break,
            }
        }
    }
    groups.sort_by(|a, b| {
        b.redundant_bytes
            .cmp(&a.redundant_bytes)
            .then_with(|| a.side.cmp(&b.side))
            .then_with(|| a.digest.cmp(&b.digest))
    });
    Ok(groups)
}

#[derive(Debug, Serialize)]
struct Totals {
    side: String,
    groups: u64,
    files: u64,
    redundant_bytes: u64,
}

impl Totals {
    fn of(side: &str, groups: &[Group]) -> Self {
        let mine = groups.iter().filter(|g| g.side == side);
        Self {
            side: side.to_owned(),
            groups: mine.clone().count() as u64,
            files: mine.clone().map(|g| g.copies).sum(),
            redundant_bytes: mine.map(|g| g.redundant_bytes).sum(),
        }
    }
}

#[derive(Debug, Serialize)]
struct Document<'a> {
    totals: Vec<Totals>,
    groups: &'a [Group],
}

fn table(out: &mut impl io::Write, groups: &[Group], names: &[String]) -> anyhow::Result<()> {
    let mut rows = Vec::new();
    for (number, group) in groups.iter().enumerate() {
        for path in &group.paths {
            rows.push(vec![
                (number + 1).to_string(),
                group.side.clone(),
                output::size(group.size),
                path.clone(),
            ]);
        }
    }
    if !rows.is_empty() {
        output::table(out, &["GROUP", "SIDE", "SIZE", "PATH"], rows.into_iter())?;
        writeln!(out)?;
    }
    for side in names {
        let t = Totals::of(side, groups);
        writeln!(
            out,
            "{side}: {} groups, {} files, {} could be freed",
            t.groups,
            t.files,
            output::size(t.redundant_bytes)
        )?;
    }
    Ok(())
}
