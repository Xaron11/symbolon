//! Read-only access to the catalog, usable while scans are writing.
//!
//! A [`Query`] owns one read-only SQLite connection. In WAL mode it sees every batch a running
//! scan has committed, so clients can browse partial results during a scan. Everything that can
//! grow with a location is paginated ([`Page`], [`PageOf`]), by keyset cursor or by offset
//! (ADR-0023). Cursor pages stay fast at any depth and do not skip or repeat rows when the scan
//! inserts more; offset pages let a list jump anywhere, and a client re-reads counts while a
//! scan is writing.

use rusqlite::{Connection, OptionalExtension as _, params};

use crate::catalog::{Catalog, CatalogError, LocationId, ms_to_system_time};
use crate::hash::{Digest, HashAlgo, PartialDigest};
use crate::pathkey::{PathEncoding, PathKey, RelPath};
use crate::scan::{ScanCounts, ScanId, ScanStatus};
use crate::source::{EntryKind, WarningKind};

/// Largest page [`Page::limit`] allows.
pub const MAX_PAGE: u32 = 10_000;

/// Which slice of a result to return: by cursor (continue after a known item) or by offset
/// (jump to a position, as a scrolled list does).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Continue after this cursor (from [`PageOf::next`]); `None` starts at the beginning.
    pub after: Option<Cursor>,
    /// Skip this many items instead; never set together with [`Page::after`].
    pub offset: Option<u64>,
    /// Maximum items, clamped to `1..=`[`MAX_PAGE`].
    pub limit: u32,
}

impl Page {
    /// The first `limit` items.
    pub fn first(limit: u32) -> Self {
        Self {
            after: None,
            offset: None,
            limit,
        }
    }

    /// The `limit` items after `cursor`.
    pub fn after(cursor: Cursor, limit: u32) -> Self {
        Self {
            after: Some(cursor),
            offset: None,
            limit,
        }
    }

    /// The `limit` items from position `offset`. Offset pages return no [`PageOf::next`].
    pub fn at(offset: u64, limit: u32) -> Self {
        Self {
            after: None,
            offset: Some(offset),
            limit,
        }
    }

    /// The offset as an SQL `OFFSET` (0 for cursor pages), or an error for a page with both.
    pub(crate) fn skip(&self) -> Result<i64, CatalogError> {
        Ok(self.checked_offset()?.unwrap_or(0).cast_signed())
    }

    /// The offset to skip, or an error for a page that also has a cursor.
    pub(crate) fn checked_offset(&self) -> Result<Option<u64>, CatalogError> {
        if self.after.is_some() && self.offset.is_some() {
            return Err(CatalogError::Corrupt(
                "page has both cursor and offset".into(),
            ));
        }
        Ok(self.offset)
    }

    fn effective_limit(&self) -> u32 {
        self.limit.clamp(1, MAX_PAGE)
    }
}

/// Opaque position in a paginated result. Only meaningful for the query that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cursor(String);

impl Cursor {
    /// The cursor as text, for passing through a UI or command line.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Rebuilds a cursor from [`Cursor::as_str`].
    pub fn from_string(cursor: String) -> Self {
        Self(cursor)
    }
}

/// One page of results.
#[derive(Debug, Clone, PartialEq)]
pub struct PageOf<T> {
    /// The items, in the query's stable order.
    pub items: Vec<T>,
    /// Cursor for the next page; `None` when this is the last page.
    pub next: Option<Cursor>,
}

/// How much of an entry's content is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashState {
    /// Only metadata.
    Unhashed,
    /// A partial digest: enough to rule out matches, not to prove one.
    Partial,
    /// A full-content digest.
    Verified,
}

/// An entry as stored in the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EntryInfo {
    /// Raw path relative to the location root.
    pub path: RelPath,
    /// Matching key of `path`.
    pub key: PathKey,
    /// File, directory, symlink or other.
    pub kind: EntryKind,
    /// Size in bytes.
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch.
    pub mtime_ns: Option<i64>,
    /// How much of the content is known.
    pub hash_state: HashState,
    /// Full-content digest, when verified.
    pub digest: Option<Digest>,
    /// Partial digest, when computed.
    pub partial: Option<PartialDigest>,
}

/// Size and hashing state of one location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct LocationStats {
    /// Regular files.
    pub files: u64,
    /// Directories.
    pub dirs: u64,
    /// Symlinks.
    pub symlinks: u64,
    /// Total size of the files.
    pub bytes: u64,
    /// Files with a full digest.
    pub verified_files: u64,
    /// Files with only a partial digest.
    pub partial_files: u64,
}

/// A scan as recorded in the catalog.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ScanInfo {
    /// The scan's id.
    pub id: ScanId,
    /// The scanned location.
    pub location_id: LocationId,
    /// Current or final state.
    pub status: ScanStatus,
    /// When it started.
    pub started_at: std::time::SystemTime,
    /// When it ended, if it did.
    pub finished_at: Option<std::time::SystemTime>,
    /// Counters (final for finished scans; zero while running).
    pub counts: ScanCounts,
    /// Error that failed the scan.
    pub error: Option<String>,
}

/// A scan in [`Query::recent_scans`], with the label of its location.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RecentScan {
    /// Label of the scanned location.
    pub label: String,
    /// The scan.
    pub scan: ScanInfo,
}

/// A warning recorded by a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WarningInfo {
    /// Entry concerned, if any.
    pub path: Option<RelPath>,
    /// Category.
    pub kind: WarningKind,
    /// Detail.
    pub message: String,
}

/// Files of one location with identical content.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DuplicateGroup {
    /// The shared full-content digest.
    pub digest: Digest,
    /// Size of each copy.
    pub size: u64,
    /// The copies, ordered by path key.
    pub entries: Vec<EntryInfo>,
}

impl DuplicateGroup {
    /// Bytes that removing all but one copy would free.
    pub fn redundant_bytes(&self) -> u64 {
        self.size * (self.entries.len() as u64).saturating_sub(1)
    }
}

struct LocationRef {
    id: i64,
    encoding: PathEncoding,
    algo: HashAlgo,
}

const ENTRY_COLUMNS: &str = "rel_path_raw, path_key, kind, size, mtime_ns, hash_partial, hash_full";

/// Read-only access to a catalog; see the module documentation.
#[derive(Debug)]
pub struct Query {
    conn: Connection,
}

impl Query {
    /// Opens a read-only connection to `catalog`.
    ///
    /// # Errors
    ///
    /// A database error if the catalog cannot be opened.
    pub fn open(catalog: &Catalog) -> Result<Self, CatalogError> {
        Ok(Self {
            conn: catalog.connect_read_only()?,
        })
    }

    fn location(&self, label: &str) -> Result<LocationRef, CatalogError> {
        let row = self
            .conn
            .query_row(
                "SELECT id, path_encoding, hash_algo FROM locations WHERE label = ?1",
                [label],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CatalogError::LocationNotFound(label.to_owned()))?;
        Ok(LocationRef {
            id: row.0,
            encoding: row
                .1
                .parse()
                .map_err(|_| CatalogError::Corrupt(format!("path encoding of {label:?}")))?,
            algo: row
                .2
                .parse()
                .map_err(|_| CatalogError::Corrupt(format!("hash algorithm of {label:?}")))?,
        })
    }

    /// Counts of entries and hashing progress for the location labelled `label`.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn location_stats(&self, label: &str) -> Result<LocationStats, CatalogError> {
        let location = self.location(label)?;
        Ok(self.conn.query_row(
            "SELECT count(*) FILTER (WHERE kind = 'file'), count(*) FILTER (WHERE kind = 'dir'), \
             count(*) FILTER (WHERE kind = 'symlink'), coalesce(sum(size) FILTER (WHERE kind = 'file'), 0), \
             count(hash_full), count(*) FILTER (WHERE hash_full IS NULL AND hash_partial IS NOT NULL) \
             FROM entries WHERE location_id = ?1",
            [location.id],
            |row| {
                Ok(LocationStats {
                    files: row.get(0)?,
                    dirs: row.get(1)?,
                    symlinks: row.get(2)?,
                    bytes: row.get(3)?,
                    verified_files: row.get(4)?,
                    partial_files: row.get(5)?,
                })
            },
        )?)
    }

    /// Scans of the location, newest first.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn scans(&self, label: &str, page: &Page) -> Result<PageOf<ScanInfo>, CatalogError> {
        let location = self.location(label)?;
        let after = page
            .after
            .as_ref()
            .map(|c| c.0.parse::<i64>())
            .transpose()
            .map_err(|_| CatalogError::Corrupt("invalid cursor".into()))?
            .unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {SCAN_COLUMNS} \
             FROM scans WHERE location_id = ?1 AND id < ?2 ORDER BY id DESC LIMIT ?3 OFFSET ?4"
        ))?;
        let rows = stmt.query_map(
            params![location.id, after, page.effective_limit(), page.skip()?],
            RawScan::from_row,
        )?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?.into_info(location.id)?);
        }
        let next = next_cursor(&items, page, |scan| scan.id.get().to_string());
        Ok(PageOf { items, next })
    }

    /// Number of scans of the location labelled `label`.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn scan_count(&self, label: &str) -> Result<u64, CatalogError> {
        let location = self.location(label)?;
        Ok(self.conn.query_row(
            "SELECT count(*) FROM scans WHERE location_id = ?1",
            [location.id],
            |row| row.get(0),
        )?)
    }

    /// Scans of every location, newest first, by offset pages only ([`Page::at`]).
    ///
    /// # Errors
    ///
    /// A database error, or [`CatalogError::Corrupt`] for a cursor page or unreadable rows.
    pub fn recent_scans(&self, page: &Page) -> Result<PageOf<RecentScan>, CatalogError> {
        if page.offset.is_none() {
            return Err(CatalogError::Corrupt(
                "recent scans are read by offset".into(),
            ));
        }
        let columns = SCAN_COLUMNS
            .split(", ")
            .map(|c| format!("s.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {columns}, s.location_id, l.label FROM scans s \
             JOIN locations l ON l.id = s.location_id \
             ORDER BY s.started_at DESC, s.id DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![page.effective_limit(), page.skip()?], |row| {
            Ok((
                RawScan::from_row(row)?,
                row.get::<_, i64>(13)?,
                row.get::<_, String>(14)?,
            ))
        })?;
        let mut items = Vec::new();
        for row in rows {
            let (raw, location_id, label) = row?;
            items.push(RecentScan {
                label,
                scan: raw.into_info(location_id)?,
            });
        }
        Ok(PageOf { items, next: None })
    }

    /// Number of scans of every location.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn recent_scan_count(&self) -> Result<u64, CatalogError> {
        Ok(self
            .conn
            .query_row("SELECT count(*) FROM scans", [], |row| row.get(0))?)
    }

    /// Number of warnings recorded by scan `scan` (0 for an unknown scan).
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn warning_count(&self, scan: ScanId) -> Result<u64, CatalogError> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM scan_warnings WHERE scan_id = ?1",
            [scan.get()],
            |row| row.get(0),
        )?)
    }

    /// Warnings recorded by scan `scan`, in the order they were found.
    ///
    /// # Errors
    ///
    /// A database error, or [`CatalogError::Corrupt`] for unreadable rows.
    pub fn warnings(&self, scan: ScanId, page: &Page) -> Result<PageOf<WarningInfo>, CatalogError> {
        let (encoding, after) = {
            let encoding: String = self
                .conn
                .query_row(
                    "SELECT l.path_encoding FROM scans s JOIN locations l ON l.id = s.location_id \
                     WHERE s.id = ?1",
                    [scan.get()],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| CatalogError::Corrupt(format!("no scan {}", scan.get())))?;
            let after = page
                .after
                .as_ref()
                .map(|c| c.0.parse::<i64>())
                .transpose()
                .map_err(|_| CatalogError::Corrupt("invalid cursor".into()))?
                .unwrap_or(0);
            (
                encoding
                    .parse::<PathEncoding>()
                    .map_err(|e| CatalogError::Corrupt(e.to_string()))?,
                after,
            )
        };
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, rel_path_raw, kind, message FROM scan_warnings \
             WHERE scan_id = ?1 AND id > ?2 ORDER BY id LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt.query_map(
            params![scan.get(), after, page.effective_limit(), page.skip()?],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )?;
        let mut items = Vec::new();
        let mut last_id = 0;
        for row in rows {
            let (id, raw, kind, message) = row?;
            last_id = id;
            items.push(WarningInfo {
                path: raw
                    .map(|raw| RelPath::from_raw(encoding, raw))
                    .transpose()
                    .map_err(|e| CatalogError::Corrupt(e.to_string()))?,
                kind: warning_kind(&kind)?,
                message,
            });
        }
        let next = next_cursor(&items, page, |_| last_id.to_string());
        Ok(PageOf { items, next })
    }

    /// The entry at `path` in the location, if it exists.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn entry(&self, label: &str, path: &RelPath) -> Result<Option<EntryInfo>, CatalogError> {
        let location = self.location(label)?;
        self.conn
            .query_row(
                &format!("SELECT {ENTRY_COLUMNS} FROM entries WHERE location_id = ?1 AND rel_path_raw = ?2"),
                params![location.id, path.as_bytes()],
                RawEntry::from_row,
            )
            .optional()?
            .map(|raw| raw.into_entry(&location))
            .transpose()
    }

    /// Entries directly inside the directory with key `parent` (`""` for the root), ordered by
    /// path key.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn children(
        &self,
        label: &str,
        parent: &str,
        page: &Page,
    ) -> Result<PageOf<EntryInfo>, CatalogError> {
        let location = self.location(label)?;
        let after = page.after.as_ref().map_or("", |c| c.0.as_str());
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {ENTRY_COLUMNS} FROM entries WHERE location_id = ?1 AND parent_key = ?2 \
             AND path_key > ?3 ORDER BY path_key LIMIT ?4 OFFSET ?5"
        ))?;
        let rows = stmt.query_map(
            params![
                location.id,
                parent,
                after,
                page.effective_limit(),
                page.skip()?
            ],
            RawEntry::from_row,
        )?;
        let items = rows
            .map(|row| row?.into_entry(&location))
            .collect::<Result<Vec<_>, _>>()?;
        let next = next_cursor(&items, page, |e| e.key.as_str().to_owned());
        Ok(PageOf { items, next })
    }

    /// Number of entries directly inside `parent` (a path key, `""` for the root).
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn child_count(&self, label: &str, parent: &str) -> Result<u64, CatalogError> {
        let location = self.location(label)?;
        Ok(self.conn.query_row(
            "SELECT count(*) FROM entries WHERE location_id = ?1 AND parent_key = ?2",
            params![location.id, parent],
            |row| row.get(0),
        )?)
    }

    /// Entries whose path contains `needle` (ASCII letters match case-insensitively), ordered by
    /// path key.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn search(
        &self,
        label: &str,
        needle: &str,
        page: &Page,
    ) -> Result<PageOf<EntryInfo>, CatalogError> {
        let location = self.location(label)?;
        // Keys write `%` as `%25`; then escape LIKE's own wildcards.
        let pattern = format!(
            "%{}%",
            needle
                .replace('%', "%25")
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let after = page.after.as_ref().map_or("", |c| c.0.as_str());
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {ENTRY_COLUMNS} FROM entries WHERE location_id = ?1 \
             AND path_key LIKE ?2 ESCAPE '\\' AND path_key > ?3 ORDER BY path_key LIMIT ?4"
        ))?;
        let rows = stmt.query_map(
            params![location.id, pattern, after, page.effective_limit()],
            RawEntry::from_row,
        )?;
        let items = rows
            .map(|row| row?.into_entry(&location))
            .collect::<Result<Vec<_>, _>>()?;
        let next = next_cursor(&items, page, |e| e.key.as_str().to_owned());
        Ok(PageOf { items, next })
    }

    /// Groups of files in the location with identical verified content, ordered by digest. Only
    /// files with a full digest are considered.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn duplicates(
        &self,
        label: &str,
        page: &Page,
    ) -> Result<PageOf<DuplicateGroup>, CatalogError> {
        let location = self.location(label)?;
        let after = page
            .after
            .as_ref()
            .map(|c| hex_decode(&c.0))
            .transpose()?
            .unwrap_or_default();
        let digests: Vec<Vec<u8>> = {
            let mut stmt = self.conn.prepare_cached(
                "SELECT hash_full FROM entries WHERE location_id = ?1 AND kind = 'file' \
                 AND hash_full IS NOT NULL AND hash_full > ?2 \
                 GROUP BY hash_full HAVING count(*) > 1 ORDER BY hash_full LIMIT ?3",
            )?;
            stmt.query_map(params![location.id, after, page.effective_limit()], |row| {
                row.get(0)
            })?
            .collect::<Result<_, _>>()?
        };
        let mut items = Vec::with_capacity(digests.len());
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {ENTRY_COLUMNS} FROM entries WHERE location_id = ?1 AND kind = 'file' \
             AND hash_full = ?2 ORDER BY path_key"
        ))?;
        for raw_digest in &digests {
            let entries = stmt
                .query_map(params![location.id, raw_digest], RawEntry::from_row)?
                .map(|row| row?.into_entry(&location))
                .collect::<Result<Vec<_>, _>>()?;
            let digest = Digest::from_bytes(location.algo, raw_digest)
                .map_err(|e| CatalogError::Corrupt(e.to_string()))?;
            let size = entries.first().map_or(0, |e| e.size);
            items.push(DuplicateGroup {
                digest,
                size,
                entries,
            });
        }
        let next = next_cursor(&items, page, |group| group.digest.to_hex());
        Ok(PageOf { items, next })
    }
}

/// The columns of `scans` a [`ScanInfo`] is built from, in [`RawScan::from_row`] order.
const SCAN_COLUMNS: &str = "id, status, started_at, finished_at, files, dirs, symlinks, bytes, \
    hashed_files, hashed_bytes, reused_hashes, warnings, error";

/// A `scans` row as stored.
struct RawScan {
    id: i64,
    status: String,
    started: i64,
    finished: Option<i64>,
    counts: ScanCounts,
    error: Option<String>,
}

impl RawScan {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            status: row.get(1)?,
            started: row.get(2)?,
            finished: row.get(3)?,
            counts: ScanCounts {
                files: row.get(4)?,
                dirs: row.get(5)?,
                symlinks: row.get(6)?,
                bytes: row.get(7)?,
                hashed_files: row.get(8)?,
                hashed_bytes: row.get(9)?,
                reused_hashes: row.get(10)?,
                warnings: row.get(11)?,
            },
            error: row.get(12)?,
        })
    }

    fn into_info(self, location_id: i64) -> Result<ScanInfo, CatalogError> {
        Ok(ScanInfo {
            id: ScanId(self.id),
            location_id: LocationId(location_id),
            status: self.status.parse()?,
            started_at: ms_to_system_time(self.started),
            finished_at: self.finished.map(ms_to_system_time),
            counts: self.counts,
            error: self.error,
        })
    }
}

fn next_cursor<T>(items: &[T], page: &Page, key: impl Fn(&T) -> String) -> Option<Cursor> {
    if page.offset.is_some() || items.len() < page.effective_limit() as usize {
        return None;
    }
    items.last().map(|last| Cursor(key(last)))
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, CatalogError> {
    let invalid = || CatalogError::Corrupt("invalid cursor".into());
    if !hex.len().is_multiple_of(2) {
        return Err(invalid());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(hex.get(i..i + 2).ok_or_else(invalid)?, 16).map_err(|_| invalid())
        })
        .collect()
}

fn warning_kind(name: &str) -> Result<WarningKind, CatalogError> {
    [
        WarningKind::Permission,
        WarningKind::Vanished,
        WarningKind::Changed,
        WarningKind::Cycle,
        WarningKind::Io,
    ]
    .into_iter()
    .find(|kind| kind.name() == name)
    .ok_or_else(|| CatalogError::Corrupt(format!("unknown warning kind {name:?}")))
}

struct RawEntry {
    raw: Vec<u8>,
    key: String,
    kind: String,
    size: u64,
    mtime_ns: Option<i64>,
    partial: Option<Vec<u8>>,
    full: Option<Vec<u8>>,
}

impl RawEntry {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            raw: row.get(0)?,
            key: row.get(1)?,
            kind: row.get(2)?,
            size: row.get(3)?,
            mtime_ns: row.get(4)?,
            partial: row.get(5)?,
            full: row.get(6)?,
        })
    }

    fn into_entry(self, location: &LocationRef) -> Result<EntryInfo, CatalogError> {
        let corrupt =
            |e: &dyn std::fmt::Display| CatalogError::Corrupt(format!("entry {:?}: {e}", self.key));
        let digest = self
            .full
            .as_deref()
            .map(|bytes| Digest::from_bytes(location.algo, bytes))
            .transpose()
            .map_err(|e| corrupt(&e))?;
        let partial = self
            .partial
            .as_deref()
            .map(|bytes| PartialDigest::from_bytes(location.algo, bytes))
            .transpose()
            .map_err(|e| corrupt(&e))?;
        let hash_state = match (&digest, &partial) {
            (Some(_), _) => HashState::Verified,
            (None, Some(_)) => HashState::Partial,
            (None, None) => HashState::Unhashed,
        };
        Ok(EntryInfo {
            path: RelPath::from_raw(location.encoding, self.raw.clone())
                .map_err(|e| corrupt(&e))?,
            kind: self.kind.parse().map_err(|e: String| corrupt(&e))?,
            key: PathKey::from_stored(self.key),
            size: self.size,
            mtime_ns: self.mtime_ns,
            hash_state,
            digest,
            partial,
        })
    }
}
