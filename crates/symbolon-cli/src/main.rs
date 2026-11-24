//! `symbolon`: command-line client for comparing file locations.
//!
//! A thin layer over `symbolon-core`; see the book chapter "Architecture" for how the pieces fit.

use clap::Parser;

/// Compare file locations and report how exactly they differ.
#[derive(Debug, Parser)]
#[command(name = "symbolon", version = symbolon_core::VERSION)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        super::Cli::command().debug_assert();
    }
}
