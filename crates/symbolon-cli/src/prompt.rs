//! Questions on the terminal. Without one nothing is asked: the caller refuses and names the
//! flag or command that decides instead.

use std::io::{self, BufRead as _, IsTerminal as _, Write as _};

/// Whether a person can answer: stdin and stderr are terminals.
pub(crate) fn interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

/// Asks a yes/no question; anything but yes is no.
pub(crate) fn yes(question: &str) -> io::Result<bool> {
    let answer = ask(&format!("{question} [y/N] "))?;
    Ok(matches!(answer.as_str(), "y" | "Y" | "yes" | "Yes"))
}

/// Lists `options` numbered from 1 and asks for one; an empty answer chooses none.
pub(crate) fn choose(question: &str, options: &[String]) -> io::Result<Option<usize>> {
    let mut lines = vec![question.to_owned()];
    lines.extend(
        options
            .iter()
            .enumerate()
            .map(|(i, option)| format!("  {}) {option}", i + 1)),
    );
    lines.push(format!(
        "Choose 1-{}, or press Enter to skip: ",
        options.len()
    ));
    let answer = ask(&lines.join("\n"))?;
    Ok(answer
        .parse::<usize>()
        .ok()
        .filter(|n| (1..=options.len()).contains(n))
        .map(|n| n - 1))
}

fn ask(text: &str) -> io::Result<String> {
    eprint!("{text}");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(answer.trim().to_owned())
}
