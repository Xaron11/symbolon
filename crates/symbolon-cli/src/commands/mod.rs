//! One module per command; each returns the exit status of a successful run.

mod diff;
mod dupes;
mod generate;
mod location;
mod scan;
mod transfer;

use std::path::PathBuf;

use anyhow::Context as _;
use symbolon_core::catalog::Catalog;

use crate::cli::{Cli, Command, Global};
use crate::exit::Exit;

pub(crate) fn run(cli: &Cli) -> anyhow::Result<Exit> {
    match &cli.command {
        Command::Location(command) => location::run(&cli.global, command),
        Command::Scan(args) => scan::run(&cli.global, args),
        Command::Diff(args) => diff::run(&cli.global, args),
        Command::Export(args) => transfer::export(&cli.global, args),
        Command::Import(args) => transfer::import(&cli.global, args),
        Command::Dupes(args) => dupes::run(&cli.global, args),
        Command::Completions { shell } => generate::completions(*shell),
        Command::Man { out } => generate::man_pages(out),
    }
}

/// Opens the catalog chosen by `--catalog`, `SYMBOLON_CATALOG` or the default path, creating
/// it on first use.
fn open_catalog(global: &Global) -> anyhow::Result<Catalog> {
    let path = match &global.catalog {
        Some(path) => path.clone(),
        None => default_catalog()?,
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        // Directories created here hold a catalog of private paths: owner only on Unix.
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    Catalog::open(&path).with_context(|| format!("cannot open the catalog {}", path.display()))
}

fn default_catalog() -> anyhow::Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .context("no local data directory on this system; pass --catalog <PATH>")?;
    Ok(dir.join("symbolon").join("catalog.db"))
}
