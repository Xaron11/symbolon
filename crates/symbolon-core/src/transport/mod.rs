//! Connections to devices whose files are not on a local filesystem.
//!
//! [`android`] (feature `android-adb`) reaches phones through `adb` (ADR-0017).

#[cfg(feature = "android-adb")]
pub mod android;
