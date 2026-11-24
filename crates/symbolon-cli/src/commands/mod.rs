//! One module per command; each returns the exit status of a successful run.

mod location;

use std::path::PathBuf;

use anyhow::Context as _;
use symbolon_core::catalog::Catalog;

use crate::cli::{Cli, Command, Global};
use crate::exit::Exit;

pub(crate) fn run(cli: &Cli) -> anyhow::Result<Exit> {
    match &cli.command {
        Command::Location(command) => location::run(&cli.global, command),
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
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    Catalog::open(&path).with_context(|| format!("cannot open the catalog {}", path.display()))
}

fn default_catalog() -> anyhow::Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .context("no local data directory on this system; pass --catalog <PATH>")?;
    Ok(dir.join("symbolon").join("catalog.db"))
}
