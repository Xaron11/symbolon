//! Shell completions and man pages, generated from the command-line definition.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write as _};
use std::path::Path;

use anyhow::Context as _;
use clap::CommandFactory as _;

use crate::cli::Cli;
use crate::exit::Exit;

pub(super) fn completions(shell: clap_complete::Shell) -> anyhow::Result<Exit> {
    let mut out = BufWriter::new(io::stdout().lock());
    clap_complete::generate(shell, &mut Cli::command(), "symbolon", &mut out);
    out.flush()?;
    Ok(Exit::Success)
}

/// `symbolon.1` and one page per command, such as `symbolon-location-add.1`.
pub(super) fn man_pages(dir: &Path) -> anyhow::Result<Exit> {
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let mut command = Cli::command();
    // Building propagates global options into every subcommand's page.
    command.build();
    write_page(&command, "symbolon", dir)?;
    Ok(Exit::Success)
}

fn write_page(command: &clap::Command, name: &str, dir: &Path) -> anyhow::Result<()> {
    let path = dir.join(format!("{name}.1"));
    let mut file = BufWriter::new(
        File::create(&path).with_context(|| format!("cannot write {}", path.display()))?,
    );
    clap_mangen::Man::new(command.clone().name(name.to_owned())).render(&mut file)?;
    file.flush()?;
    for sub in command
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set() && sub.get_name() != "help")
    {
        write_page(sub, &format!("{name}-{}", sub.get_name()), dir)?;
    }
    Ok(())
}
