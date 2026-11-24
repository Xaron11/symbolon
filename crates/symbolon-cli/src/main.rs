//! `symbolon`: command-line client for comparing file locations.
//!
//! A thin layer over `symbolon-core`; conventions (catalog path, formats, exit codes) are in
//! ADR-0011.

mod cli;
mod commands;
mod exit;
mod output;

use std::process::ExitCode;

use clap::Parser as _;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match commands::run(&cli) {
        Ok(exit) => exit.into(),
        Err(error) => {
            eprintln!("error: {error:#}");
            exit::Exit::for_error(&error).into()
        }
    }
}
