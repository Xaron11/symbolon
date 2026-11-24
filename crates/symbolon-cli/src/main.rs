use clap::Parser;

/// Compare file locations and report how exactly they differ.
#[derive(Debug, Parser)]
#[command(name = "symbolon", version = symbolon_core::VERSION)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        super::Cli::command().debug_assert();
    }
}
