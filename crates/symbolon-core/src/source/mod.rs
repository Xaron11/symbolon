//! Sources: where a location's files are listed and read from.
//!
//! A [`Source`] walks a location without reading file contents, opens files for hashing and
//! re-checks single entries. [`LocalSource`] covers local and mounted filesystems; the Android
//! source (M5) implements the same trait over ADB.
//!
//! Problems with single entries (permission denied, a file vanishing mid-walk, I/O errors)
//! are reported as [`SourceWarning`]s and never stop a walk; only an unusable root is an error.

#[cfg(feature = "android-adb")]
mod adb;
#[cfg(feature = "local")]
mod local;

use std::fmt;
use std::io::{self, Read, Seek};
use std::ops::ControlFlow;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "android-adb")]
pub use self::adb::{AdbMode, AdbOptions, AdbSource};
#[cfg(feature = "local")]
pub use self::local::LocalSource;
use crate::hash::{Digest, HashAlgo, PartialDigest};
use crate::pathkey::RelPath;

/// What kind of filesystem object an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// A symbolic link, recorded but never followed.
    Symlink,
    /// Anything else: sockets, FIFOs, device nodes.
    Other,
}

impl EntryKind {
    /// The stable name stored in the catalog.
    pub const fn name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Dir => "dir",
            Self::Symlink => "symlink",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for EntryKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file" => Ok(Self::File),
            "dir" => Ok(Self::Dir),
            "symlink" => Ok(Self::Symlink),
            "other" => Ok(Self::Other),
            _ => Err(format!("unknown entry kind {s:?}")),
        }
    }
}

/// One object found by a walk: metadata only, no contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    /// Path relative to the location root.
    pub path: RelPath,
    /// File, directory, symlink or other.
    pub kind: EntryKind,
    /// Size in bytes: content length for files, link target length for symlinks, 0 for
    /// directories.
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch, if the source reports one.
    /// Precision depends on the filesystem and platform (for example 100 ns on Windows, 2 s on
    /// FAT).
    pub mtime_ns: Option<i64>,
    /// The symlink's target as raw bytes in the source's path encoding; `None` for other kinds
    /// or when it could not be read.
    pub link_target: Option<Vec<u8>>,
}

/// Category of a [`SourceWarning`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarningKind {
    /// Access denied.
    Permission,
    /// The entry disappeared between being listed and being read.
    Vanished,
    /// The entry changed while it was being hashed.
    Changed,
    /// A directory reached twice (bind mount or filesystem loop); not descended again.
    Cycle,
    /// Any other I/O error.
    Io,
}

impl WarningKind {
    /// The stable name stored in the catalog.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Vanished => "vanished",
            Self::Changed => "changed",
            Self::Cycle => "cycle",
            Self::Io => "io",
        }
    }

    /// The warning kind for an I/O error.
    pub fn from_io(error: &io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::PermissionDenied => Self::Permission,
            io::ErrorKind::NotFound => Self::Vanished,
            _ => Self::Io,
        }
    }
}

impl FromStr for WarningKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        [
            Self::Permission,
            Self::Vanished,
            Self::Changed,
            Self::Cycle,
            Self::Io,
        ]
        .into_iter()
        .find(|kind| kind.name() == s)
        .ok_or_else(|| format!("unknown warning kind {s:?}"))
    }
}

impl fmt::Display for WarningKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A problem with one entry; the walk or scan continues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWarning {
    /// The entry concerned, if known.
    pub path: Option<RelPath>,
    /// Category.
    pub kind: WarningKind,
    /// Human-readable detail.
    pub message: String,
}

impl SourceWarning {
    /// A warning for an I/O error on `path`.
    pub fn io(path: Option<RelPath>, error: &io::Error) -> Self {
        Self {
            path,
            kind: WarningKind::from_io(error),
            message: error.to_string(),
        }
    }
}

/// An item reported by [`Source::walk`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkItem {
    /// An entry was found.
    Entry(SourceEntry),
    /// An entry could not be listed or inspected.
    Warning(SourceWarning),
}

/// How a walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOutcome {
    /// Every reachable entry was reported.
    Complete,
    /// The visitor asked to stop.
    Stopped,
}

/// Errors that make a whole source unusable.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// The root is missing or not a directory (for example an unplugged drive).
    #[error("location root {root} is unavailable: {source}")]
    RootUnavailable {
        /// The root, for display.
        root: String,
        /// The underlying error.
        source: io::Error,
    },
    /// An ignore pattern is not valid `.gitignore` syntax.
    #[error("invalid ignore pattern {pattern:?}: {message}")]
    InvalidPattern {
        /// The offending pattern.
        pattern: String,
        /// Why it is invalid.
        message: String,
    },
    /// The connection to a remote source failed (for example the phone was unplugged).
    #[error("{0}")]
    Remote(String),
}

/// A file to hash where it is, with the metadata it was listed with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashRequest {
    /// The file.
    pub path: RelPath,
    /// Size when listed; a different size makes the reply a warning.
    pub size: u64,
    /// Modification time when listed; a different time makes the reply a warning.
    pub mtime_ns: Option<i64>,
    /// A partial digest (size plus the first and last 64 KiB) instead of the full content.
    pub partial: bool,
}

/// The answer to one [`HashRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashReply {
    /// The full-content digest.
    Full(Digest),
    /// The partial digest.
    Partial(PartialDigest),
    /// No digest: the file could not be read, or it changed since it was listed.
    Warning(SourceWarning),
}

/// Hashes files on the source's side, for sources whose contents are expensive to move (a
/// phone over ADB, ADR-0017). Digests must be exactly what [`crate::hash`] computes locally.
pub trait RemoteHasher: Send + Sync {
    /// Whether digests of `algo` can be computed.
    fn supports(&self, algo: HashAlgo) -> bool;

    /// Whether partial digests can be computed; if not, partial requests are sent as full ones.
    fn partial_digests(&self) -> bool;

    /// Hashes `requests`, calling `each` with each request's index and reply as they arrive,
    /// in any order. Stops early when `each` returns [`ControlFlow::Break`].
    ///
    /// # Errors
    ///
    /// [`SourceError::Remote`] when the source becomes unusable; requests without a reply are
    /// then reported as warnings by the caller.
    fn hash(
        &self,
        algo: HashAlgo,
        requests: &[HashRequest],
        each: &mut dyn FnMut(usize, HashReply) -> ControlFlow<()>,
    ) -> Result<(), SourceError>;
}

/// Readable and seekable file contents, as needed for full and partial hashing.
pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

/// A location's files: listed by [`Source::walk`], read by [`Source::open`].
pub trait Source: Send + Sync {
    /// Lists every entry under the root, calling `visit` for each entry and each warning, until
    /// the walk is done or `visit` returns [`ControlFlow::Break`]. Contents are not read.
    ///
    /// Order is deterministic: a directory's entries are reported together, sorted by raw name,
    /// and a directory is always reported before any of its contents.
    ///
    /// # Errors
    ///
    /// [`SourceError::RootUnavailable`] if the root itself cannot be listed.
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError>;

    /// Opens a file's contents.
    ///
    /// # Errors
    ///
    /// The I/O error from opening the file.
    fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>>;

    /// Current metadata of one entry, without following symlinks.
    ///
    /// # Errors
    ///
    /// The I/O error from inspecting the entry.
    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry>;

    /// Hashing on the source's side, if the source offers it; scans and comparisons then never
    /// [`open`](Source::open) its files.
    fn remote_hasher(&self) -> Option<&dyn RemoteHasher> {
        None
    }

    /// Something the user should know about how the location is read, such as which path a
    /// phone is scanned through and why.
    fn note(&self) -> Option<String> {
        None
    }
}

/// Nanoseconds since the Unix epoch (negative before it), saturating at the `i64` range.
#[cfg_attr(
    not(any(feature = "local", test)),
    expect(dead_code, reason = "used by sources, which are feature-gated")
)]
pub(crate) fn system_time_to_ns(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, |ns| -ns),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn names_round_trip() {
        for kind in [
            EntryKind::File,
            EntryKind::Dir,
            EntryKind::Symlink,
            EntryKind::Other,
        ] {
            assert_eq!(kind.name().parse::<EntryKind>().unwrap(), kind);
        }
        assert!("pipe".parse::<EntryKind>().is_err());
    }

    #[test]
    fn io_errors_map_to_warning_kinds() {
        let kind = |k| WarningKind::from_io(&io::Error::from(k));
        assert_eq!(
            kind(io::ErrorKind::PermissionDenied),
            WarningKind::Permission
        );
        assert_eq!(kind(io::ErrorKind::NotFound), WarningKind::Vanished);
        assert_eq!(kind(io::ErrorKind::InvalidData), WarningKind::Io);
    }

    #[test]
    fn times_convert_on_both_sides_of_the_epoch() {
        // 1.5 µs: representable on every platform (Windows counts in 100 ns ticks).
        assert_eq!(
            system_time_to_ns(UNIX_EPOCH + Duration::from_nanos(1_500)),
            1_500
        );
        assert_eq!(
            system_time_to_ns(UNIX_EPOCH - Duration::from_secs(1)),
            -1_000_000_000
        );
    }
}
