//! Core library of Symbolon: scanning, catalog, and N-way comparison of file locations.
//!
//! Clients write through `Scanner` and read through `Query`; both are added in M1.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::print_stdout, clippy::print_stderr)]

/// Version of this crate, shared by the whole workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        assert_eq!(super::VERSION.split('.').count(), 3);
    }
}
