//! Core library of Symbolon: scanning, catalog, and N-way comparison of file locations.
//!
//! Clients write through `Scanner` and read through `Query`; both are added in M1.

#![forbid(unsafe_code)]

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        assert_eq!(super::VERSION.split('.').count(), 3);
    }
}
