//! Browsing a location's catalog, also while it is being scanned.

use serde::Serialize;
use specta_typescript::Number;
use symbolon_core::pathkey::PathKey;
use symbolon_core::query::{EntryInfo, Page, Query};
use symbolon_core::source::EntryKind;

use crate::compare::{HashStateView, KindView};
use crate::error::ApiError;
use crate::state::AppState;

/// An entry of a location as the browse tree shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntryView {
    /// The path key: identifies the entry, and opens it when it is a folder.
    pub(crate) key: String,
    /// The last part of the path, for display (lossy).
    pub(crate) name: String,
    pub(crate) kind: KindView,
    pub(crate) folder: bool,
    #[specta(type = Number)]
    pub(crate) size: u64,
    #[specta(type = Option<Number>)]
    pub(crate) mtime_ms: Option<i64>,
    pub(crate) hash_state: HashStateView,
}

impl From<EntryInfo> for EntryView {
    fn from(entry: EntryInfo) -> Self {
        Self {
            name: PathKey::from_stored(entry.key.file_name().to_owned()).display(),
            key: entry.key.as_str().to_owned(),
            kind: entry.kind.into(),
            folder: entry.kind == EntryKind::Dir,
            size: entry.size,
            mtime_ms: entry.mtime_ns.map(|ns| ns.div_euclid(1_000_000)),
            hash_state: entry.hash_state.into(),
        }
    }
}

impl AppState {
    /// Number of entries directly in folder `parent` (a key; `""` for the root).
    pub(crate) fn browse_child_count(&self, label: &str, parent: &str) -> Result<u64, ApiError> {
        Ok(Query::open(&self.catalog)?.child_count(label, parent)?)
    }

    /// Entries directly in folder `parent`, by key.
    pub(crate) fn browse_children(
        &self,
        label: &str,
        parent: &str,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<EntryView>, ApiError> {
        Ok(Query::open(&self.catalog)?
            .children(label, parent, &Page::at(offset, limit))?
            .items
            .into_iter()
            .map(EntryView::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use symbolon_core::catalog::NewLocation;
    use symbolon_core::scan::{ScanOptions, Scanner};

    use super::*;
    use crate::scans::BrowseChange;
    use crate::sink::testing::{Collect, Gone};
    use crate::state::testing::state;

    fn scanned(dir: &std::path::Path, files: &[&str]) -> AppState {
        let root = dir.join("root");
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let s = state(dir, Vec::new());
        s.catalog
            .add_location(NewLocation::local("loc", &root))
            .unwrap();
        Scanner::start(&s.catalog, "loc", ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
        s
    }

    #[test]
    fn browse_lists_root_then_folder_by_key() {
        let dir = tempfile::tempdir().unwrap();
        let s = scanned(dir.path(), &["a.txt", "docs/b.txt", "docs/c.txt"]);
        assert_eq!(s.browse_child_count("loc", "").unwrap(), 2);
        let root = s.browse_children("loc", "", 0, 10).unwrap();
        assert_eq!(
            root.iter()
                .map(|e| (e.name.as_str(), e.folder))
                .collect::<Vec<_>>(),
            [("a.txt", false), ("docs", true)]
        );
        let docs = s.browse_children("loc", &root[1].key, 1, 10).unwrap();
        assert_eq!(docs[0].name, "c.txt");
        assert_eq!(docs[0].key, "docs/c.txt");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_names_display_lossy_and_expand_by_key() {
        use std::os::unix::ffi::OsStrExt as _;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let odd = root.join(std::ffi::OsStr::from_bytes(b"dir\xe9"));
        fs::create_dir_all(&odd).unwrap();
        fs::write(odd.join("inside.txt"), "x").unwrap();
        let s = scanned(dir.path(), &[]);
        let root = s.browse_children("loc", "", 0, 10).unwrap();
        assert_eq!(root[0].name, "dir\u{FFFD}");
        assert_eq!(root[0].key, "dir%E9");
        let inside = s.browse_children("loc", &root[0].key, 0, 10).unwrap();
        assert_eq!(inside[0].name, "inside.txt");
    }

    #[test]
    fn unsubscribed_browsers_hear_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let s = scanned(dir.path(), &["a", "b"]);
        let changes = Collect::default();
        let id = s.scans.subscribe_browse("loc", Box::new(changes.clone()));
        s.scans.unsubscribe_browse(id);
        Arc::clone(&s.scans)
            .start(&s.catalog, "loc", Box::new(Gone))
            .unwrap();
        while s.scans.is_running("loc") {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(changes.all(), []);
    }

    #[test]
    fn browse_changes_are_throttled_and_finish() {
        let dir = tempfile::tempdir().unwrap();
        let files: Vec<String> = (0..300).map(|i| format!("f{i:04}")).collect();
        let names: Vec<&str> = files.iter().map(String::as_str).collect();
        let s = scanned(dir.path(), &names);
        let changes = Collect::default();
        s.scans.subscribe_browse("loc", Box::new(changes.clone()));
        let started = Instant::now();
        Arc::clone(&s.scans)
            .start(&s.catalog, "loc", Box::new(Gone))
            .unwrap();
        while s.scans.is_running("loc") {
            std::thread::sleep(Duration::from_millis(10));
        }
        let received = changes.all();
        assert_eq!(received.last(), Some(&BrowseChange::Finished));
        let notices = received
            .iter()
            .filter(|c| **c == BrowseChange::Changed)
            .count();
        let allowed = usize::try_from(started.elapsed().as_secs()).unwrap() + 1;
        assert!(
            notices <= allowed,
            "{notices} changes in {:?}",
            started.elapsed()
        );
    }
}
