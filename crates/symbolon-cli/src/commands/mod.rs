//! One module per command; each returns the exit status of a successful run.

mod devices;
mod diff;
mod dupes;
mod follow;
mod generate;
mod location;
mod scan;
#[cfg(debug_assertions)]
mod test_mounts;
mod transfer;

use std::path::PathBuf;

use anyhow::Context as _;
use std::sync::Arc;

use symbolon_core::catalog::{self, Catalog};
use symbolon_core::source::AdbOptions;
use symbolon_core::transport::android::SystemAdb;

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
        Command::Devices => devices::run(&cli.global),
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
    catalog::create_catalog_dir(&path)
        .with_context(|| format!("cannot create {}", path.parent().unwrap_or(&path).display()))?;
    let catalog = Catalog::open(&path)
        .with_context(|| format!("cannot open the catalog {}", path.display()))?
        .with_android(
            Arc::new(SystemAdb::new()),
            find_agent(),
            AdbOptions::default(),
        );
    #[cfg(debug_assertions)]
    let catalog = test_mounts::apply(catalog);
    Ok(catalog)
}

/// The agent to push to phones (ADR-0017): `SYMBOLON_AGENT`, else next to this executable,
/// else in `../libexec/symbolon/` (Nix). Without one, phones are read with the fallback.
fn find_agent() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SYMBOLON_AGENT") {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    catalog::agent_candidates(exe.parent()?)
        .into_iter()
        .find(|path| path.is_file())
}

fn default_catalog() -> anyhow::Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .context("no local data directory on this system; pass --catalog <PATH>")?;
    Ok(catalog::default_catalog_path(&dir))
}
