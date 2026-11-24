//! A one-line progress display on stderr, drawn only when stderr is a terminal (ADR-0011).

use std::io::{self, IsTerminal as _, Write as _};

#[derive(Debug)]
pub(crate) struct Progress {
    enabled: bool,
    /// Characters on the line now, to blank them out; no ANSI escapes, so plain Windows
    /// consoles work too.
    width: usize,
}

impl Progress {
    pub(crate) fn new(quiet: bool) -> Self {
        Self {
            enabled: !quiet && io::stderr().is_terminal(),
            width: 0,
        }
    }

    pub(crate) fn show(&mut self, line: &str) {
        if !self.enabled {
            return;
        }
        let len = line.chars().count();
        let padding = self.width.saturating_sub(len);
        let mut err = io::stderr().lock();
        // Progress is best-effort; a broken stderr must not fail the work.
        write!(err, "\r{line}{}", " ".repeat(padding)).ok();
        err.flush().ok();
        self.width = len;
    }

    pub(crate) fn clear(&mut self) {
        if self.width > 0 {
            let mut err = io::stderr().lock();
            write!(err, "\r{}\r", " ".repeat(self.width)).ok();
            err.flush().ok();
            self.width = 0;
        }
    }
}
