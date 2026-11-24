//! The `symbolon` binary end to end: one module per command.

#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

mod common;
mod diff;
mod location;
mod scan;
mod transfer;
