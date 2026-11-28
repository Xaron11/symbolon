//! Core library of Symbolon: scan file locations into a catalog and compare them.
//!
//! Symbolon answers one question for two or more locations (directories, disks, pendrives,
//! Android phones): do they match, and if not, how exactly do they differ? This crate holds all
//! of the logic; the CLI, the desktop app and the Android agent are thin clients.
//!
//! # Two independent halves
//!
//! - **[`Scanner`](scan::Scanner)** writes: it scans one location, hashes file contents, stores
//!   results in the catalog as it goes and streams progress events. It can be cancelled and
//!   paused.
//! - **[`Query`](query::Query)** reads: it lists locations and scans, browses and searches scanned
//!   trees and groups files by hash. It works while a scan is writing.
//!
//! A client may use both halves in one process or in different processes; this is what makes
//! browsing partial results during a scan cheap. A [`Comparison`](compare::Comparison) of two or
//! more locations is built from the catalog on its own connection.
//!
//! # Modules
//!
//! | Module        | Responsibility                                                          |
//! | ------------- | ----------------------------------------------------------------------- |
//! | [`scan`]      | `Scanner`: walk, store, hash; events, cancel, pause, rescans            |
//! | [`query`]     | `Query`: browse, search, duplicates, scan history; paginated            |
//! | [`compare`]   | `Comparison` of two or more sides: categories, summary, rows, resolving |
//! | [`catalog`]   | SQLite catalog: opening, migrations, locations, export and import       |
//! | [`pathkey`]   | Lossless relative paths and normalized matching keys                    |
//! | [`hash`]      | Hash algorithms, streaming full digests and partial digests             |
//! | [`identity`]  | Device fingerprints: finding a location's device again                  |
//! | [`source`]    | Walking and reading a location; `LocalSource` (feature `local`)         |
//! | [`transport`] | `AndroidTransport` over `adb` (feature `android-adb`)                   |
//! | `agent`       | Host–agent protocol for the Android agent (feature `agent-protocol`)    |
//!
//! _(planned)_ `plan`: write operations (M7). See the book chapter *Architecture* in `docs/`.
//!
//! # Guarantees
//!
//! - No `unsafe` code, and no panics on I/O paths: errors are returned, never unwrapped.
//! - Raw paths are never converted lossily.
//! - Two files are reported identical only on equal full hashes of the same algorithm.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr)]

#[cfg(feature = "agent-protocol")]
pub mod agent;
pub mod catalog;
pub mod compare;
pub mod hash;
pub mod identity;
pub mod pathkey;
pub mod query;
pub mod scan;
pub mod source;
pub mod transport;

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
