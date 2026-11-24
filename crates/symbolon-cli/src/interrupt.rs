//! Ctrl-C (ADR-0011): the first press asks the running work to stop cleanly, the second exits
//! at once.

use std::sync::atomic::{AtomicUsize, Ordering};

static PRESSES: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn install() -> anyhow::Result<()> {
    ctrlc::set_handler(|| {
        if PRESSES.fetch_add(1, Ordering::SeqCst) >= 1 {
            // Every catalog write is a transaction; the next scan marks this one interrupted.
            std::process::exit(130);
        }
    })?;
    Ok(())
}

pub(crate) fn requested() -> bool {
    PRESSES.load(Ordering::SeqCst) > 0
}
