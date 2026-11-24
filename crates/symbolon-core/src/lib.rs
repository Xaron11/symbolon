//! Core library of Symbolon: scan file locations into a catalog and compare them.
//!
//! Symbolon answers one question for two or more locations (directories, disks, pendrives,
//! Android phones): do they match, and if not, how exactly do they differ? This crate holds all
//! of the logic; the CLI, the desktop app and the Android agent are thin clients.
//!
//! # Two independent halves
//!
//! - **`Scanner`** writes: it scans one location, hashes file contents, stores results in the
//!   catalog as it goes and streams progress events. It can be cancelled and paused.
//! - **`Query`** reads: it lists locations, browses and searches scanned trees, groups files by
//!   hash and runs comparisons. It works while a scan is writing.
//!
//! A client may use both halves in one process or in different processes; this is what makes
//! browsing partial results during a scan cheap.
//!
//! # Status
//!
//! Milestone M1 is in progress; modules land one at a time. Implemented so far:
//!
//! | Module      | Responsibility                                              |
//! | ----------- | ----------------------------------------------------------- |
//! | [`hash`]    | Hash algorithms, streaming full digests and partial digests |
//! | [`pathkey`] | Lossless relative paths and normalized matching keys        |
//!
//! _(planned)_ `source` (walking a location), `catalog`
//! (SQLite storage, internal), `scan` (`Scanner`), `query` (`Query`), then `compare` and `plan`
//! in later milestones. See the book chapter *Architecture* in `docs/`.
//!
//! # Guarantees
//!
//! - No `unsafe` code, and no panics on I/O paths: errors are returned, never unwrapped.
//! - Raw paths are never converted lossily.
//! - Two files are reported identical only on equal full hashes of the same algorithm.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod hash;
pub mod pathkey;

/// Version of this crate, shared by the whole workspace.
///
/// ```
/// assert!(!symbolon_core::VERSION.is_empty());
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        assert_eq!(super::VERSION.split('.').count(), 3);
    }
}
