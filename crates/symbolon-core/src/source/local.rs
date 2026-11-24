//! [`LocalSource`]: a directory on a local or mounted filesystem.

use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use super::{
    EntryKind, ReadSeek, Source, SourceEntry, SourceError, SourceWarning, WalkItem, WalkOutcome,
    WarningKind, system_time_to_ns,
};
use crate::pathkey::{PathEncoding, RelPath};

/// A directory tree on a local or mounted filesystem.
///
/// Symlinks are recorded but never followed. On Unix, directories are tracked by device and
/// inode, so a directory reachable twice (bind mounts) is reported as a
/// [`WarningKind::Cycle`] instead of being walked again.
#[derive(Debug)]
pub struct LocalSource {
    root: PathBuf,
    ignore: Gitignore,
}

impl LocalSource {
    /// A source rooted at `root`, skipping paths matched by `ignore_patterns` (`.gitignore`
    /// syntax, relative to the root; `!pattern` re-includes).
    ///
    /// # Errors
    ///
    /// [`SourceError::InvalidPattern`] for a pattern that is not valid `.gitignore` syntax.
    pub fn new(root: impl Into<PathBuf>, ignore_patterns: &[String]) -> Result<Self, SourceError> {
        let root = root.into();
        let mut builder = GitignoreBuilder::new(&root);
        for pattern in ignore_patterns {
            builder
                .add_line(None, pattern)
                .map_err(|e| SourceError::InvalidPattern {
                    pattern: pattern.clone(),
                    message: e.to_string(),
                })?;
        }
        let ignore = builder.build().map_err(|e| SourceError::InvalidPattern {
            pattern: ignore_patterns.join("\n"),
            message: e.to_string(),
        })?;
        Ok(Self { root, ignore })
    }

    /// The root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn native(&self, path: &RelPath) -> io::Result<PathBuf> {
        path.to_native(&self.root)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }

    fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        self.ignore.matched(path, is_dir).is_ignore()
    }
}

impl Source for LocalSource {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        let unavailable = |source| SourceError::RootUnavailable {
            root: self.root.display().to_string(),
            source,
        };
        let root_meta = fs::metadata(&self.root).map_err(unavailable)?;
        if !root_meta.is_dir() {
            return Err(unavailable(io::Error::new(
                io::ErrorKind::NotADirectory,
                "not a directory",
            )));
        }
        let mut seen_dirs = HashSet::new();
        seen_dirs.insert(dir_identity(&root_meta));

        // One directory at a time from an explicit stack: its sorted entries are reported, then
        // its subdirectories are pushed in reverse so they are visited in sorted order.
        let mut stack = vec![(RelPath::root(PathEncoding::native()), self.root.clone())];
        let mut first = true;
        while let Some((rel, dir)) = stack.pop() {
            let read = match fs::read_dir(&dir) {
                Ok(read) => read,
                Err(e) if first => return Err(unavailable(e)),
                Err(e) => {
                    if visit(WalkItem::Warning(SourceWarning::io(Some(rel), &e))).is_break() {
                        return Ok(WalkOutcome::Stopped);
                    }
                    continue;
                }
            };
            first = false;
            let mut children = Vec::new();
            for item in read {
                match item {
                    Ok(item) => children.push(item),
                    Err(e) => {
                        if visit(WalkItem::Warning(SourceWarning::io(Some(rel.clone()), &e)))
                            .is_break()
                        {
                            return Ok(WalkOutcome::Stopped);
                        }
                    }
                }
            }
            children.sort_by_cached_key(fs::DirEntry::file_name);

            let mut subdirs = Vec::new();
            for child in children {
                let native = child.path();
                let Ok(child_rel) = rel.join(&child.file_name()) else {
                    continue;
                };
                let meta = match fs::symlink_metadata(&native) {
                    Ok(meta) => meta,
                    Err(e) => {
                        if visit(WalkItem::Warning(SourceWarning::io(Some(child_rel), &e)))
                            .is_break()
                        {
                            return Ok(WalkOutcome::Stopped);
                        }
                        continue;
                    }
                };
                let kind = entry_kind(&meta);
                if self.is_ignored(&native, kind == EntryKind::Dir) {
                    continue;
                }
                if kind == EntryKind::Dir && !seen_dirs.insert(dir_identity(&meta)) {
                    let warning = SourceWarning {
                        path: Some(child_rel),
                        kind: WarningKind::Cycle,
                        message: "directory already visited (bind mount or filesystem loop)"
                            .to_owned(),
                    };
                    if visit(WalkItem::Warning(warning)).is_break() {
                        return Ok(WalkOutcome::Stopped);
                    }
                    continue;
                }
                let entry = entry_from_metadata(child_rel.clone(), &meta);
                if visit(WalkItem::Entry(entry)).is_break() {
                    return Ok(WalkOutcome::Stopped);
                }
                if kind == EntryKind::Dir {
                    subdirs.push((child_rel, native));
                }
            }
            stack.extend(subdirs.into_iter().rev());
        }
        Ok(WalkOutcome::Complete)
    }

    fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        Ok(Box::new(fs::File::open(self.native(path)?)?))
    }

    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        let meta = fs::symlink_metadata(self.native(path)?)?;
        Ok(entry_from_metadata(path.clone(), &meta))
    }
}

fn entry_kind(meta: &Metadata) -> EntryKind {
    let file_type = meta.file_type();
    if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_dir() {
        EntryKind::Dir
    } else if file_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

fn entry_from_metadata(path: RelPath, meta: &Metadata) -> SourceEntry {
    let kind = entry_kind(meta);
    SourceEntry {
        path,
        kind,
        size: if kind == EntryKind::Dir {
            0
        } else {
            meta.len()
        },
        mtime_ns: meta.modified().ok().map(system_time_to_ns),
    }
}

#[cfg(unix)]
fn dir_identity(meta: &Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt as _;
    (meta.dev(), meta.ino())
}

/// Without stable device/inode numbers every directory counts as new; symlinks are never
/// followed, so only exotic setups (junction loops on Windows) could repeat.
#[cfg(not(unix))]
fn dir_identity(_meta: &Metadata) -> (u64, u64) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    (u64::MAX, NEXT.fetch_add(1, Ordering::Relaxed))
}
