//! The command-line definition (spec §10, ADR-0011).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use symbolon_core::catalog::HashPolicy;
use symbolon_core::hash::HashAlgo;

use crate::output::Format;

/// Compare file locations and report how exactly they differ.
#[derive(Debug, Parser)]
#[command(name = "symbolon", version = symbolon_core::VERSION, propagate_version = true)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) global: Global,
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Options every command accepts.
#[derive(Debug, Args)]
pub(crate) struct Global {
    /// Catalog database [default: symbolon/catalog.db in the local data directory]
    #[arg(long, global = true, env = "SYMBOLON_CATALOG", value_name = "PATH")]
    pub(crate) catalog: Option<PathBuf>,
    /// Output format: table for people, json and csv for scripts
    #[arg(long, global = true, value_enum, default_value_t = Format::Table)]
    pub(crate) format: Format,
    /// Same as --format json
    #[arg(long, global = true, conflicts_with = "format")]
    pub(crate) json: bool,
    /// Print no progress or warnings, only results and errors
    #[arg(long, short, global = true)]
    pub(crate) quiet: bool,
}

impl Global {
    pub(crate) fn format(&self) -> Format {
        if self.json { Format::Json } else { self.format }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Manage saved locations
    #[command(subcommand)]
    Location(LocationCommand),
    /// Scan locations: record their files and hash contents as their policy says
    Scan(ScanArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ScanArgs {
    /// Locations to scan, one after another
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub(crate) labels: Vec<String>,
    /// Scan every location of this computer
    #[arg(long)]
    pub(crate) all: bool,
    /// Hash every file, also in lazy locations
    #[arg(long)]
    pub(crate) full: bool,
    /// List every warning instead of counting them
    #[arg(long, short)]
    pub(crate) verbose: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LocationCommand {
    /// Save a directory as a location
    Add(AddArgs),
    /// List saved locations
    List,
    /// Forget a location and everything recorded about it; its files are not touched
    Remove {
        /// The location's label
        label: String,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
}

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    /// Name for the location, used by every other command
    pub(crate) label: String,
    /// Directory holding the files
    pub(crate) path: PathBuf,
    /// When to hash contents: during every scan (eager) or only when a comparison needs it (lazy)
    #[arg(long, value_enum)]
    pub(crate) policy: Option<Policy>,
    /// Hash algorithm; only locations with the same algorithm can be compared
    #[arg(long, value_enum)]
    pub(crate) algo: Option<Algo>,
    /// Skip paths matching a pattern in .gitignore syntax; repeatable
    #[arg(long = "ignore", value_name = "PATTERN")]
    pub(crate) ignore: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Policy {
    Lazy,
    Eager,
}

impl From<Policy> for HashPolicy {
    fn from(policy: Policy) -> Self {
        match policy {
            Policy::Lazy => Self::Lazy,
            Policy::Eager => Self::Eager,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Algo {
    Blake3,
    Xxh3,
    Sha256,
}

impl From<Algo> for HashAlgo {
    fn from(algo: Algo) -> Self {
        match algo {
            Algo::Blake3 => Self::Blake3,
            Algo::Xxh3 => Self::Xxh3,
            Algo::Sha256 => Self::Sha256,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;

    #[test]
    fn definition_is_valid() {
        super::Cli::command().debug_assert();
    }
}
