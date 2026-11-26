//! Comparing two or more locations (ADR-0010, ADR-0019).
//!
//! A [`Comparison`] copies the compared files, symlinks and directories of every [`Side`] into
//! temporary tables on its own connection (one consistent snapshot of the catalog), classifies
//! them with SQL, and serves the results as a [`Summary`] and as pages of [`Row`]s. Results stay
//! in SQLite, so comparing large locations never loads every row into memory.
//!
//! Every compared file and symlink belongs to exactly one row; a directory belongs to at most
//! one. Path rows group entries by normalized path (see [`crate::pathkey::KeyOptions`]). A
//! group of directories only is a row only when it is [`Category::OnlyIn`] with nothing
//! compared beneath it. Other groups are classified in this order: [`Category::Collision`],
//! [`Category::Different`] (including a file, directory or symlink clashing with another kind),
//! then by kind: files by content, symlinks by target, as [`Category::Identical`],
//! [`Category::OnlyIn`] or [`Category::Unverified`]. Moves, renames and duplicates are files
//! only.

mod resolve;
mod sql;

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use rusqlite::Connection;

use crate::catalog::{Catalog, CatalogError};
use crate::hash::{Digest, HashAlgo, PartialDigest};
use crate::pathkey::{KeyOptions, PathEncoding, PathKey, RelPath};
use crate::query::{Cursor, HashState, Page, PageOf};
use crate::source::EntryKind;

pub use self::resolve::{HashNeed, ResolveEvent, ResolveSummary};

/// Most sides one comparison can have; presence sets are 64-bit masks.
pub const MAX_SIDES: usize = 63;

/// Errors from setting up or reading a comparison.
#[derive(Debug, thiserror::Error)]
pub enum CompareError {
    /// Looking up a location failed, or the catalog cannot be read.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// A database error while comparing.
    #[error("catalog database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Fewer than two or more than [`MAX_SIDES`] sides.
    #[error("a comparison needs 2 to {MAX_SIDES} sides, got {0}")]
    SideCount(usize),
    /// A side specification or subpath that is not a valid relative path.
    #[error("invalid side {0:?}: expected `label` or `label:relative/path`")]
    InvalidSide(String),
    /// The locations use different hash algorithms, so their digests cannot be compared.
    #[error("locations use different hash algorithms ({}); rehash them with one algorithm first",
        .0.iter().map(|(label, algo)| format!("{label}: {algo}")).collect::<Vec<_>>().join(", "))]
    AlgorithmMismatch(Vec<(String, HashAlgo)>),
}

/// One side of a comparison: a location, or a subdirectory of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Side {
    /// Location label.
    pub label: String,
    /// Path key of the compared directory, relative to the location root; empty for the root.
    pub subpath: String,
}

impl Side {
    /// A whole location.
    pub fn location(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            subpath: String::new(),
        }
    }

    /// A subdirectory of a location, given as a relative native path.
    ///
    /// # Errors
    ///
    /// [`CompareError::InvalidSide`] if `subpath` is absolute or contains `..`.
    pub fn subdir(label: impl Into<String>, subpath: &Path) -> Result<Self, CompareError> {
        let label = label.into();
        let rel = RelPath::from_path(subpath)
            .map_err(|_| CompareError::InvalidSide(format!("{label}:{}", subpath.display())))?;
        Ok(Self {
            label,
            subpath: rel.key().as_str().to_owned(),
        })
    }
}

impl FromStr for Side {
    type Err = CompareError;

    /// Parses `label` or `label:sub/path` (labels cannot contain `:`).
    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        match spec.split_once(':') {
            None if !spec.is_empty() => Ok(Self::location(spec)),
            Some((label, subpath)) if !label.is_empty() => Self::subdir(label, Path::new(subpath)),
            _ => Err(CompareError::InvalidSide(spec.to_owned())),
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.subpath.is_empty() {
            f.write_str(&self.label)
        } else {
            write!(f, "{}:{}", self.label, self.subpath)
        }
    }
}

/// How a comparison matches and decides.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct CompareOptions {
    /// Path matching: Unicode normalization and case-insensitivity.
    pub keys: KeyOptions,
    /// Fast mode: treat files with equal size and modification times within this tolerance as
    /// equal without hashing. Such rows are marked [`Row::assumed`] and must be presented as
    /// unverified. `None` (the default) never assumes.
    pub assume_equal_within: Option<Duration>,
    /// Also find duplicates within each side: [`Comparison::hash_needs`] then includes files
    /// that share a size with another file of the same side. Off by default (extra hashing).
    pub duplicates: bool,
}

impl CompareOptions {
    /// Sets [`CompareOptions::keys`].
    #[must_use]
    pub fn keys(mut self, keys: KeyOptions) -> Self {
        self.keys = keys;
        self
    }

    /// Sets [`CompareOptions::duplicates`].
    #[must_use]
    pub fn duplicates(mut self, duplicates: bool) -> Self {
        self.duplicates = duplicates;
        self
    }

    /// Enables the fast mode; see [`CompareOptions::assume_equal_within`].
    #[must_use]
    pub fn assume_equal_within(mut self, tolerance: Duration) -> Self {
        self.assume_equal_within = Some(tolerance);
        self
    }
}

/// What a row says about the entries it groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// Same path and the same full digest (files) or target (symlinks) on every side.
    Identical,
    /// Same path on two or more sides with different size, content or target, or of
    /// different kinds.
    Different,
    /// Present on some sides only, with equal content or target where present; for a
    /// directory, with nothing compared beneath it.
    OnlyIn,
    /// Same content at different directories with the same file name.
    Moved,
    /// Same content under a different file name.
    Renamed,
    /// Several entries of one side share the normalized path (directories alone merge
    /// instead).
    Collision,
    /// Same path on two or more sides, but equality is unknown: files of the same size not
    /// hashed yet, or symlinks with an unknown target.
    Unverified,
}

impl Category {
    /// Every category.
    pub const ALL: [Self; 7] = [
        Self::Identical,
        Self::Different,
        Self::OnlyIn,
        Self::Moved,
        Self::Renamed,
        Self::Collision,
        Self::Unverified,
    ];

    /// Stable name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::Different => "different",
            Self::OnlyIn => "only_in",
            Self::Moved => "moved",
            Self::Renamed => "renamed",
            Self::Collision => "collision",
            Self::Unverified => "unverified",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Category {
    type Err = CatalogError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|c| c.name() == s)
            .ok_or_else(|| CatalogError::Corrupt(format!("unknown category {s:?}")))
    }
}

/// A set of sides, by index into [`Comparison::sides`]. Sets order by their bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SideSet(u64);

impl SideSet {
    /// Whether side `index` is in the set.
    pub const fn contains(self, index: usize) -> bool {
        index < 64 && self.0 & (1 << index) != 0
    }

    /// Number of sides in the set.
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Whether the set is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The side indices, ascending.
    pub fn iter(self) -> impl Iterator<Item = usize> {
        (0..64).filter(move |&i| self.contains(i))
    }

    /// The set as a bitmask (bit `i` = side `i`).
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// A set from a bitmask.
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}

/// A resolved side: the location behind a [`Side`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SideInfo {
    /// The side as requested.
    pub side: Side,
    /// Hash algorithm of the location.
    pub hash_algo: HashAlgo,
    /// Path encoding of the location.
    pub path_encoding: PathEncoding,
    pub(crate) location_id: i64,
}

/// One compared entry (file, directory or symlink) within a [`Row`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Cell {
    /// Index of the side (into [`Comparison::sides`]).
    pub side: usize,
    /// Path within the location (not relative to the side's subpath).
    pub path: RelPath,
    /// Path key relative to the side's subpath, before normalization.
    pub key: PathKey,
    /// File, directory or symlink.
    pub kind: EntryKind,
    /// A symlink's target as raw bytes in the side's path encoding; `None` for other kinds
    /// or when unknown (ADR-0019).
    pub link_target: Option<Vec<u8>>,
    /// Size in bytes as recorded by the scan; only meaningful for files.
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch.
    pub mtime_ns: Option<i64>,
    /// How much of the content is known.
    pub hash_state: HashState,
    /// Full digest, if known.
    pub digest: Option<Digest>,
    /// Partial digest, if known.
    pub partial: Option<PartialDigest>,
}

/// A classified group of entries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Row {
    /// What the row says.
    pub category: Category,
    /// Normalized path relative to the sides' subpaths; for moves, the first placement's.
    pub key: String,
    /// Sides holding an entry of this row.
    pub presence: SideSet,
    /// Equality assumed from size and modification time (fast mode), not verified.
    pub assumed: bool,
    /// Common size, when the row holds only files and they all have the same size.
    pub size: Option<u64>,
    /// The entries, ordered by side, then path.
    pub cells: Vec<Cell>,
}

/// Totals of one category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Totals {
    /// Rows.
    pub rows: u64,
    /// Entries (files, directories and symlinks) in those rows.
    pub entries: u64,
    /// Bytes of the files in those rows.
    pub bytes: u64,
    /// Rows whose equality is assumed (fast mode).
    pub assumed_rows: u64,
}

/// Per-category totals of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Summary {
    /// Totals by category; categories without rows are absent.
    pub categories: BTreeMap<Category, Totals>,
    /// Totals of the [`Category::OnlyIn`] rows by the sides holding them.
    pub only_in: BTreeMap<SideSet, Totals>,
}

impl Summary {
    /// Totals for `category` (zero if absent).
    pub fn get(&self, category: Category) -> Totals {
        self.categories.get(&category).copied().unwrap_or_default()
    }

    /// Entries compared in total.
    pub fn entries(&self) -> u64 {
        self.categories.values().map(|t| t.entries).sum()
    }
}

/// Which rows [`Comparison::rows`] and [`Comparison::filtered_summary`] consider. Every set
/// field must match; the default matches all rows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct RowFilter {
    /// Only these categories; empty means all.
    pub categories: Vec<Category>,
    /// Only rows whose file name has this extension (case-insensitive, without the dot).
    pub extension: Option<String>,
    /// Only rows whose largest file has at least this many bytes.
    pub min_size: Option<u64>,
    /// Only rows whose largest file has at most this many bytes.
    pub max_size: Option<u64>,
    /// Only rows at or under this relative path (matched with the comparison's key options).
    pub prefix: Option<String>,
    /// Only rows present on exactly these sides.
    pub presence: Option<SideSet>,
    /// Only rows with an entry of one of these kinds; empty means all. [`EntryKind::Other`]
    /// entries are never compared.
    pub kinds: Vec<EntryKind>,
}

impl RowFilter {
    /// Rows of the given categories.
    pub fn categories(categories: impl IntoIterator<Item = Category>) -> Self {
        Self {
            categories: categories.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Restricts to file names with `extension` (`"jpg"`, `".JPG"` and `"Jpg"` are the same).
    #[must_use]
    pub fn extension(mut self, extension: &str) -> Self {
        self.extension = Some(extension.trim_start_matches('.').to_lowercase());
        self
    }

    /// Restricts to rows whose largest file is within `min..=max` bytes. Rows without files
    /// (directories, symlinks) never match a size.
    #[must_use]
    pub fn size_range(mut self, min: Option<u64>, max: Option<u64>) -> Self {
        self.min_size = min;
        self.max_size = max;
        self
    }

    /// Restricts to rows at or under the relative path `prefix`.
    #[must_use]
    pub fn prefix(mut self, prefix: &str) -> Self {
        self.prefix = Some(prefix.to_owned());
        self
    }

    /// Restricts to rows present on exactly `sides`.
    #[must_use]
    pub fn presence(mut self, sides: SideSet) -> Self {
        self.presence = Some(sides);
        self
    }

    /// Restricts to rows holding an entry of one of `kinds`.
    #[must_use]
    pub fn kinds(mut self, kinds: impl IntoIterator<Item = EntryKind>) -> Self {
        self.kinds = kinds.into_iter().collect();
        self
    }
}

/// Rows and bytes matching a filter beneath a folder of the comparison tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FolderTotals {
    /// Matching rows anywhere beneath the folder.
    pub rows: u64,
    /// Bytes of the files in those rows.
    pub bytes: u64,
}

/// A child of a folder in [`Comparison::children`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TreeChild {
    /// A matching row with nothing matching beneath it.
    Row(Row),
    /// A path with matching rows beneath it.
    Folder {
        /// The folder's key, to pass as `parent` to [`Comparison::children`].
        key: String,
        /// The last part of the key, as stored (escaped and normalized like the key).
        name: String,
        /// The last part of the key for people to read ([`PathKey::display`]); under
        /// case-insensitive or NFC matching it is the normalized spelling.
        display: String,
        /// The row at this path when it matches too, such as a file on one side that is a
        /// directory on another.
        row: Option<Row>,
        /// What matches beneath it.
        matching: FolderTotals,
    },
}

/// Rows and bytes of one file extension in [`Comparison::by_extension`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExtensionTotals {
    /// The extension, lowercase and without the dot (`""` for names without one); `None` for
    /// the extensions beyond the requested limit, counted together.
    pub extension: Option<String>,
    /// Rows whose file name has the extension.
    pub rows: u64,
    /// Bytes of the files in those rows, all sides together.
    pub bytes: u64,
}

/// Upper bounds (exclusive) of the [`Comparison::size_histogram`] buckets: below 1 KiB, 1 MiB,
/// 10 MiB, 100 MiB and 1 GiB, and the rest.
pub const SIZE_BUCKETS: [Option<u64>; 6] = [
    Some(1 << 10),
    Some(1 << 20),
    Some(10 << 20),
    Some(100 << 20),
    Some(1 << 30),
    None,
];

/// Rows and bytes in one bucket of [`Comparison::size_histogram`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SizeBucket {
    /// Rows in the bucket have their largest file below this size; `None` for the last bucket.
    pub max_exclusive: Option<u64>,
    /// Rows in the bucket.
    pub rows: u64,
    /// Bytes of the files in those rows, all sides together.
    pub bytes: u64,
}

/// The order of [`Comparison::rows`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RowOrder {
    /// By path key.
    Path(Direction),
    /// By the size of the row's largest file, then by path key. Rows without files
    /// (directories, symlinks) come after all others in both directions.
    Size(Direction),
}

impl Default for RowOrder {
    fn default() -> Self {
        Self::Path(Direction::Asc)
    }
}

/// Ascending or descending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    /// Smallest first.
    #[default]
    Asc,
    /// Largest first.
    Desc,
}

/// Files with identical content within one side.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DuplicateSet {
    /// Index of the side.
    pub side: usize,
    /// The shared full digest.
    pub digest: Digest,
    /// Size of each copy.
    pub size: u64,
    /// The copies, ordered by path.
    pub cells: Vec<Cell>,
}

impl DuplicateSet {
    /// Bytes that removing all but one copy would free.
    pub fn redundant_bytes(&self) -> u64 {
        self.size * (self.cells.len() as u64).saturating_sub(1)
    }
}

/// Duplicate totals of one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct DuplicateTotals {
    /// Groups of identical files.
    pub groups: u64,
    /// Files in those groups.
    pub files: u64,
    /// Bytes that removing all but one copy per group would free.
    pub redundant_bytes: u64,
}

/// A comparison of two or more sides; see the module documentation.
#[derive(Debug)]
pub struct Comparison {
    catalog: Catalog,
    conn: Connection,
    sides: Vec<SideInfo>,
    options: CompareOptions,
}

impl Comparison {
    /// Compares `sides` as currently recorded in `catalog`.
    ///
    /// # Errors
    ///
    /// - [`CompareError::SideCount`] for fewer than 2 or more than [`MAX_SIDES`] sides; one
    ///   side is enough with [`CompareOptions::duplicates`], to find its duplicates.
    /// - [`CompareError::Catalog`] for unknown locations.
    /// - [`CompareError::AlgorithmMismatch`] if the locations use different hash algorithms.
    pub fn new(
        catalog: &Catalog,
        sides: &[Side],
        options: CompareOptions,
    ) -> Result<Self, CompareError> {
        let fewest = if options.duplicates { 1 } else { 2 };
        if !(fewest..=MAX_SIDES).contains(&sides.len()) {
            return Err(CompareError::SideCount(sides.len()));
        }
        let conn = catalog.connect()?;
        // A page cache for the temporary tables of large comparisons (a budget, not an
        // allocation).
        conn.pragma_update(Some("temp"), "cache_size", -131_072)?;
        sql::register_functions(&conn)?;
        let infos = sides
            .iter()
            .map(|side| sql::resolve_side(&conn, side))
            .collect::<Result<Vec<_>, _>>()?;
        let algos: Vec<_> = infos
            .iter()
            .map(|info| (info.side.label.clone(), info.hash_algo))
            .collect();
        if algos.iter().any(|(_, algo)| *algo != algos[0].1) {
            return Err(CompareError::AlgorithmMismatch(algos));
        }
        let mut comparison = Self {
            catalog: catalog.clone(),
            conn,
            sides: infos,
            options,
        };
        sql::create_tables(&comparison.conn)?;
        comparison.refresh()?;
        Ok(comparison)
    }

    /// The resolved sides, in the order given to [`Comparison::new`].
    pub fn sides(&self) -> &[SideInfo] {
        &self.sides
    }

    /// The options in use.
    pub fn options(&self) -> &CompareOptions {
        &self.options
    }

    /// Re-reads the catalog and reclassifies, for example after hashing.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn refresh(&mut self) -> Result<(), CompareError> {
        sql::load(&mut self.conn, &self.sides, &self.options)?;
        sql::classify(&self.conn, self.sides.len(), &self.options)?;
        Ok(())
    }

    /// Row, entry and byte totals per category.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn summary(&self) -> Result<Summary, CompareError> {
        self.filtered_summary(&RowFilter::default())
    }

    /// Totals per category of the rows matching `filter`.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn filtered_summary(&self, filter: &RowFilter) -> Result<Summary, CompareError> {
        sql::summary(&self.conn, &sql::FilterParams::new(filter, &self.options))
    }

    /// Duplicate groups within side `side` (an index into [`Comparison::sides`]), ordered by
    /// digest. Only files with full digests count; empty files are left out, since they are
    /// all trivially identical. Pages by cursor or by offset ([`Page::at`]).
    ///
    /// # Errors
    ///
    /// A database error, or [`CatalogError::Corrupt`] for an invalid cursor or a page with both a
    /// cursor and an offset.
    pub fn duplicates(
        &self,
        side: usize,
        page: &Page,
    ) -> Result<PageOf<DuplicateSet>, CompareError> {
        sql::duplicates(&self.conn, &self.sides, side, page)
    }

    /// Duplicate totals per side, indexed like [`Comparison::sides`].
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn duplicate_totals(&self) -> Result<Vec<DuplicateTotals>, CompareError> {
        sql::duplicate_totals(&self.conn, self.sides.len())
    }

    /// Rows and bytes matching `filter` per file extension: the `limit` extensions with the
    /// most bytes (ties by name), then one entry without an extension for all others if any
    /// remain. Rows without files are not counted.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn by_extension(
        &self,
        filter: &RowFilter,
        limit: u32,
    ) -> Result<Vec<ExtensionTotals>, CompareError> {
        sql::by_extension(
            &self.conn,
            &sql::FilterParams::new(filter, &self.options),
            limit,
        )
    }

    /// Rows and bytes matching `filter` by the size of their largest file, one entry per
    /// [`SIZE_BUCKETS`] bound in that order (empty buckets included). Rows without files are not
    /// counted.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn size_histogram(&self, filter: &RowFilter) -> Result<Vec<SizeBucket>, CompareError> {
        sql::size_histogram(&self.conn, &sql::FilterParams::new(filter, &self.options))
    }

    /// Number of rows matching `filter`.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn count(&self, filter: &RowFilter) -> Result<u64, CompareError> {
        sql::count(&self.conn, &sql::FilterParams::new(filter, &self.options))
    }

    /// The children of folder `parent` that match `filter` or have matching rows beneath them,
    /// ordered by name. `parent` is `""` for the root or a key from [`TreeChild::Folder`]. A
    /// moved or renamed row sits at its first placement, as in [`Row::key`]. Children are read
    /// by offset ([`Page::at`]); pages carry no cursor.
    ///
    /// # Errors
    ///
    /// A database error, or [`CatalogError::Corrupt`] for a cursor page.
    pub fn children(
        &self,
        filter: &RowFilter,
        parent: &str,
        page: &Page,
    ) -> Result<PageOf<TreeChild>, CompareError> {
        sql::children(
            &self.conn,
            &self.sides,
            &sql::FilterParams::new(filter, &self.options),
            parent,
            page,
        )
    }

    /// Number of children [`Comparison::children`] lists for `parent`.
    ///
    /// # Errors
    ///
    /// A database error.
    pub fn child_count(&self, filter: &RowFilter, parent: &str) -> Result<u64, CompareError> {
        sql::child_count(
            &self.conn,
            &sql::FilterParams::new(filter, &self.options),
            parent,
        )
    }

    /// A page of rows matching `filter`, in `order`. Cursor pages are only available in the
    /// default order (path, ascending); offset pages work in every order and stay valid until
    /// the next [`Comparison::refresh`].
    ///
    /// # Errors
    ///
    /// A database error, or [`CatalogError::Corrupt`] for an invalid cursor, a cursor with
    /// another order, or a page with both a cursor and an offset.
    pub fn rows(
        &self,
        filter: &RowFilter,
        order: RowOrder,
        page: &Page,
    ) -> Result<PageOf<Row>, CompareError> {
        sql::rows(
            &self.conn,
            &self.sides,
            &sql::FilterParams::new(filter, &self.options),
            order,
            page,
        )
    }
}

/// The cursor text for a row at `sort_key` with database id `id`.
fn row_cursor(sort_key: &str, id: i64) -> Cursor {
    Cursor::from_string(format!("{id}:{sort_key}"))
}

fn parse_row_cursor(cursor: &Cursor) -> Result<(String, i64), CatalogError> {
    let (id, key) = cursor
        .as_str()
        .split_once(':')
        .ok_or_else(|| CatalogError::Corrupt("invalid cursor".into()))?;
    let id = id
        .parse()
        .map_err(|_| CatalogError::Corrupt("invalid cursor".into()))?;
    Ok((key.to_owned(), id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_parse_labels_and_subpaths() {
        assert_eq!("laptop".parse::<Side>().unwrap(), Side::location("laptop"));
        let sub: Side = "disk:photos/2024".parse().unwrap();
        assert_eq!(
            (sub.label.as_str(), sub.subpath.as_str()),
            ("disk", "photos/2024")
        );
        assert_eq!(sub.to_string(), "disk:photos/2024");
        assert_eq!("disk:".parse::<Side>().unwrap(), Side::location("disk"));
        for bad in ["", ":photos", "disk:../x", "disk:/abs"] {
            assert!(bad.parse::<Side>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn side_sets_are_bitmasks() {
        let set = SideSet::from_bits(0b101);
        assert_eq!(
            (set.len(), set.contains(0), set.contains(1), set.contains(2)),
            (2, true, false, true)
        );
        assert_eq!(set.iter().collect::<Vec<_>>(), [0, 2]);
        assert!(SideSet::default().is_empty());
        assert!(!set.is_empty());
        let top = SideSet::from_bits(1 << 63);
        assert!(top.contains(63));
        assert!(!top.contains(64), "out-of-range indices are never members");
    }

    #[test]
    fn category_names_round_trip() {
        for category in Category::ALL {
            assert_eq!(category.name().parse::<Category>().unwrap(), category);
            assert_eq!(category.to_string(), category.name());
        }
    }

    #[test]
    fn row_cursors_round_trip_keys_with_colons() {
        let cursor = row_cursor("a:b/c", 42);
        assert_eq!(parse_row_cursor(&cursor).unwrap(), ("a:b/c".to_owned(), 42));
        assert!(parse_row_cursor(&Cursor::from_string("nope".into())).is_err());
    }
}
