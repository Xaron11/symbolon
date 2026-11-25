//! The command-line definition (spec §10, ADR-0011).

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use symbolon_core::catalog::HashPolicy;
use symbolon_core::compare::Category;
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
    /// Compare locations, or folders of them; exits 1 if anything differs
    Diff(DiffArgs),
    /// Write a location's latest complete scan to a .symbolon file, to compare it elsewhere
    Export(ExportArgs),
    /// Add a location from a .symbolon file, or update one with it
    Import(ImportArgs),
    /// List files with identical content within each location
    Dupes(DupesArgs),
    /// List phones connected over adb and whether they are ready
    Devices,
    /// Print a shell completion script, for example: symbolon completions bash > ~/.local/share/bash-completion/completions/symbolon
    Completions {
        /// The shell
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Write man pages for every command into a directory (for packagers)
    #[command(hide = true)]
    Man {
        /// Directory to write the pages into
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
}

#[derive(Debug, Args)]
pub(crate) struct ExportArgs {
    /// The location to export
    pub(crate) label: String,
    /// File to write, usually ending in .symbolon
    #[arg(long, short, value_name = "FILE")]
    pub(crate) output: PathBuf,
    /// Replace the file if it exists
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    /// The .symbolon file
    pub(crate) file: PathBuf,
    /// Add it as a new location with this label
    #[arg(long = "as", value_name = "LABEL", conflicts_with_all = ["into", "replace"])]
    pub(crate) as_label: Option<String>,
    /// Update this location with it: the same device saved under another label
    #[arg(long, value_name = "LABEL", conflicts_with = "replace")]
    pub(crate) into: Option<String>,
    /// Replace the data of the location with the file's label, although it is another device
    #[arg(long)]
    pub(crate) replace: bool,
    /// Update a location even when its own scan is newer than the file's
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent command-line flags"
)]
pub(crate) struct DiffArgs {
    /// Locations to compare, as LABEL or LABEL:folder/in/it; two or more
    #[arg(required = true, num_args = 2.., value_name = "SIDE")]
    pub(crate) sides: Vec<String>,
    /// Match paths regardless of letter case
    #[arg(long)]
    pub(crate) case_insensitive: bool,
    /// Match paths regardless of Unicode normalization (NFC and NFD spellings)
    #[arg(long)]
    pub(crate) nfc: bool,
    /// Treat files with equal size and modification times within this tolerance as equal
    /// without hashing (shown as assumed), for example 2s or 500ms
    #[arg(long, value_name = "TOLERANCE", value_parser = parse_duration)]
    pub(crate) fast: Option<Duration>,
    /// Do not hash files to decide unverified rows
    #[arg(long)]
    pub(crate) no_hash: bool,
    /// List only these categories
    #[arg(long, value_enum, value_delimiter = ',', value_name = "CATEGORY")]
    pub(crate) only: Vec<CategoryArg>,
    /// List identical rows too
    #[arg(long, conflicts_with = "only")]
    pub(crate) all: bool,
    /// List only files with this extension
    #[arg(long, value_name = "EXT")]
    pub(crate) ext: Option<String>,
    /// List only files of at least this many bytes
    #[arg(long, value_name = "BYTES")]
    pub(crate) min_size: Option<u64>,
    /// List only files of at most this many bytes
    #[arg(long, value_name = "BYTES")]
    pub(crate) max_size: Option<u64>,
    /// List only paths under this folder (relative to the compared folders)
    #[arg(long, value_name = "FOLDER")]
    pub(crate) under: Option<String>,
    /// Print the totals only
    #[arg(long)]
    pub(crate) summary: bool,
    /// List every file that could not be hashed instead of counting them
    #[arg(long, short)]
    pub(crate) verbose: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DupesArgs {
    /// Locations to search, as LABEL or LABEL:folder/in/it
    #[arg(required = true, value_name = "SIDE")]
    pub(crate) sides: Vec<String>,
    /// Ignore files smaller than this many bytes
    #[arg(long, value_name = "BYTES")]
    pub(crate) min_size: Option<u64>,
    /// Do not hash files; only already hashed files are compared
    #[arg(long)]
    pub(crate) no_hash: bool,
    /// List every file that could not be hashed instead of counting them
    #[arg(long, short)]
    pub(crate) verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CategoryArg {
    Identical,
    Different,
    #[value(alias = "only_in")]
    OnlyIn,
    Moved,
    Renamed,
    Collision,
    Unverified,
}

impl From<CategoryArg> for Category {
    fn from(category: CategoryArg) -> Self {
        match category {
            CategoryArg::Identical => Self::Identical,
            CategoryArg::Different => Self::Different,
            CategoryArg::OnlyIn => Self::OnlyIn,
            CategoryArg::Moved => Self::Moved,
            CategoryArg::Renamed => Self::Renamed,
            CategoryArg::Collision => Self::Collision,
            CategoryArg::Unverified => Self::Unverified,
        }
    }
}

/// `500ms`, `2s`, `1.5s`, `3m`, `1h`; a bare number is seconds.
fn parse_duration(text: &str) -> Result<Duration, String> {
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number
        .parse()
        .map_err(|_| format!("{text:?} is not a duration such as 2s or 500ms"))?;
    let seconds = match unit.trim() {
        "ms" => value / 1000.0,
        "" | "s" => value,
        "m" | "min" => value * 60.0,
        "h" => value * 3600.0,
        other => return Err(format!("unknown unit {other:?}; use ms, s, m or h")),
    };
    Duration::try_from_secs_f64(seconds).map_err(|e| e.to_string())
}

#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent command-line flags"
)]
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
    /// When another device (or an empty mount point) is at a location's root, scan what is
    /// there as the location without asking
    #[arg(long, short)]
    pub(crate) yes: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LocationCommand {
    /// Save a directory as a location
    Add(AddArgs),
    /// List saved locations
    List,
    /// Change a location's label, root or hash policy
    Set(SetArgs),
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
    /// Directory holding the files, or a directory on a phone written adb:SERIAL:/PATH; leave
    /// SERIAL empty for the only connected phone
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

#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("change").required(true).multiple(true).args(["new_label", "root", "policy"])))]
pub(crate) struct SetArgs {
    /// The location's current label
    pub(crate) label: String,
    /// New label
    #[arg(long = "label", value_name = "NEW_LABEL")]
    pub(crate) new_label: Option<String>,
    /// New root directory, for example where its device is mounted now; records the device
    /// found there
    #[arg(long, value_name = "PATH")]
    pub(crate) root: Option<PathBuf>,
    /// When to hash contents from now on: during every scan (eager) or only when a comparison
    /// needs it (lazy)
    #[arg(long, value_enum)]
    pub(crate) policy: Option<Policy>,
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

    #[test]
    fn durations_parse_with_units() {
        use std::time::Duration;
        let parse = super::parse_duration;
        assert_eq!(parse("2"), Ok(Duration::from_secs(2)));
        assert_eq!(parse("2s"), Ok(Duration::from_secs(2)));
        assert_eq!(parse("1.5s"), Ok(Duration::from_millis(1500)));
        assert_eq!(parse("500ms"), Ok(Duration::from_millis(500)));
        assert_eq!(parse("3m"), Ok(Duration::from_secs(180)));
        assert_eq!(parse("1h"), Ok(Duration::from_secs(3600)));
        assert!(parse("2 weeks").is_err());
        assert!(parse("fast").is_err());
        assert!(parse("").is_err());
    }
}
