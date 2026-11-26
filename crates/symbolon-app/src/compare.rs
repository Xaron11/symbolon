//! Open comparisons: shown at once from the catalog, verified in the background (ADR-0024).

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta_typescript::Number;
use symbolon_core::catalog::Catalog;
use symbolon_core::compare::{
    Category, Cell, CompareError, CompareOptions, Comparison, Direction, DuplicateSet,
    ResolveEvent, Row, RowFilter, RowOrder, Side, SideSet, Summary, TreeChild,
};
use symbolon_core::pathkey::{KeyOptions, PathKey};
use symbolon_core::query::{HashState, Page};
use symbolon_core::scan::ScanOptions;
use symbolon_core::source::EntryKind;

use crate::dto::Num;
use crate::error::{ApiError, ErrorKind};
use crate::sink::Sink;

/// Least time between two verification progress updates.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Extensions the extension chart shows before folding the rest.
const TOP_EXTENSIONS: u32 = 10;

/// A comparison category, named as core names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CategoryName {
    Identical,
    Different,
    OnlyIn,
    Moved,
    Renamed,
    Collision,
    Unverified,
}

impl From<Category> for CategoryName {
    fn from(category: Category) -> Self {
        match category {
            Category::Identical => Self::Identical,
            Category::Different => Self::Different,
            Category::OnlyIn => Self::OnlyIn,
            Category::Moved => Self::Moved,
            Category::Renamed => Self::Renamed,
            Category::Collision => Self::Collision,
            Category::Unverified => Self::Unverified,
        }
    }
}

impl From<CategoryName> for Category {
    fn from(name: CategoryName) -> Self {
        match name {
            CategoryName::Identical => Self::Identical,
            CategoryName::Different => Self::Different,
            CategoryName::OnlyIn => Self::OnlyIn,
            CategoryName::Moved => Self::Moved,
            CategoryName::Renamed => Self::Renamed,
            CategoryName::Collision => Self::Collision,
            CategoryName::Unverified => Self::Unverified,
        }
    }
}

/// One side to compare: a location, or a folder within it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SideRequest {
    pub(crate) label: String,
    pub(crate) subpath: Option<String>,
}

/// The comparison picker's choices.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompareRequest {
    pub(crate) sides: Vec<SideRequest>,
    pub(crate) nfc: bool,
    pub(crate) case_insensitive: bool,
    /// Fast mode: files with equal size and modification times this close are assumed equal.
    pub(crate) fast_ms: Option<Num<u64>>,
    pub(crate) duplicates: bool,
}

/// Rows, entries and bytes of one category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CategoryTotals {
    pub(crate) category: CategoryName,
    #[specta(type = Number)]
    pub(crate) rows: u64,
    #[specta(type = Number)]
    pub(crate) entries: u64,
    #[specta(type = Number)]
    pub(crate) bytes: u64,
    #[specta(type = Number)]
    pub(crate) assumed_rows: u64,
}

/// Rows, entries and bytes of the only-in rows held by exactly `sides`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlyInTotals {
    pub(crate) sides: Vec<u32>,
    #[specta(type = Number)]
    pub(crate) rows: u64,
    #[specta(type = Number)]
    pub(crate) entries: u64,
    #[specta(type = Number)]
    pub(crate) bytes: u64,
    #[specta(type = Number)]
    pub(crate) assumed_rows: u64,
}

/// Totals by category, and of only-in rows by the sides holding them; empty groups are left
/// out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SummaryView {
    pub(crate) categories: Vec<CategoryTotals>,
    pub(crate) only_in: Vec<OnlyInTotals>,
}

impl From<Summary> for SummaryView {
    fn from(summary: Summary) -> Self {
        Self {
            only_in: summary
                .only_in
                .into_iter()
                .map(|(sides, totals)| OnlyInTotals {
                    sides: sides
                        .iter()
                        .filter_map(|side| u32::try_from(side).ok())
                        .collect(),
                    rows: totals.rows,
                    entries: totals.entries,
                    bytes: totals.bytes,
                    assumed_rows: totals.assumed_rows,
                })
                .collect(),
            categories: summary
                .categories
                .into_iter()
                .map(|(category, totals)| CategoryTotals {
                    category: category.into(),
                    rows: totals.rows,
                    entries: totals.entries,
                    bytes: totals.bytes,
                    assumed_rows: totals.assumed_rows,
                })
                .collect(),
        }
    }
}

/// An opened comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompareOpened {
    pub(crate) id: u32,
    pub(crate) sides: Vec<SideRequest>,
    pub(crate) summary: SummaryView,
}

/// Which rows to read; unset fields match everything.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ApiFilter {
    pub(crate) categories: Vec<CategoryName>,
    pub(crate) extension: Option<String>,
    pub(crate) min_size: Option<Num<u64>>,
    pub(crate) max_size: Option<Num<u64>>,
    /// Exactly these sides (indices into the comparison's sides) hold the row.
    pub(crate) presence: Option<Vec<u32>>,
}

impl From<ApiFilter> for RowFilter {
    fn from(filter: ApiFilter) -> Self {
        let mut row = Self::categories(filter.categories.into_iter().map(Category::from))
            .size_range(filter.min_size.map(|n| n.0), filter.max_size.map(|n| n.0));
        if let Some(extension) = filter.extension {
            row = row.extension(&extension);
        }
        if let Some(presence) = filter.presence {
            let bits = presence
                .into_iter()
                .filter(|&side| side < 64)
                .fold(0, |bits, side| bits | (1_u64 << side));
            row = row.presence(SideSet::from_bits(bits));
        }
        row
    }
}

/// What to sort rows by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
pub(crate) enum OrderBy {
    Path,
    Size,
}

/// A row order: by path or size, ascending unless `desc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
pub(crate) struct RowOrderView {
    pub(crate) by: OrderBy,
    pub(crate) desc: bool,
}

impl From<RowOrderView> for RowOrder {
    fn from(order: RowOrderView) -> Self {
        let direction = if order.desc {
            Direction::Desc
        } else {
            Direction::Asc
        };
        match order.by {
            OrderBy::Path => Self::Path(direction),
            OrderBy::Size => Self::Size(direction),
        }
    }
}

/// A file, folder or link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) enum KindView {
    File,
    Dir,
    Symlink,
    Other,
}

impl From<EntryKind> for KindView {
    fn from(kind: EntryKind) -> Self {
        match kind {
            EntryKind::File => Self::File,
            EntryKind::Dir => Self::Dir,
            EntryKind::Symlink => Self::Symlink,
            EntryKind::Other => Self::Other,
        }
    }
}

/// How much of a file's content is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) enum HashStateView {
    Unhashed,
    Partial,
    Verified,
}

impl From<HashState> for HashStateView {
    fn from(state: HashState) -> Self {
        match state {
            HashState::Unhashed => Self::Unhashed,
            HashState::Partial => Self::Partial,
            HashState::Verified => Self::Verified,
        }
    }
}

/// The first 12 hex digits of a digest, enough to tell digests apart by eye.
fn short_hex(hex: &str) -> String {
    hex.chars().take(12).collect()
}

/// One side's entry in a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CellView {
    pub(crate) side: u32,
    /// Path within the location, for display (lossy).
    pub(crate) path: String,
    pub(crate) kind: KindView,
    #[specta(type = Number)]
    pub(crate) size: u64,
    #[specta(type = Option<Number>)]
    pub(crate) mtime_ms: Option<i64>,
    pub(crate) hash_state: HashStateView,
    pub(crate) digest_short: Option<String>,
}

impl From<&Cell> for CellView {
    fn from(cell: &Cell) -> Self {
        Self {
            side: u32::try_from(cell.side).unwrap_or(u32::MAX),
            path: String::from_utf8_lossy(cell.path.as_bytes()).into_owned(),
            kind: cell.kind.into(),
            size: cell.size,
            mtime_ms: cell.mtime_ns.map(|ns| ns.div_euclid(1_000_000)),
            hash_state: cell.hash_state.into(),
            digest_short: cell.digest.as_ref().map(|d| short_hex(&d.to_hex())),
        }
    }
}

/// A classified row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompareRowView {
    /// The row's key, to identify it.
    pub(crate) key: String,
    /// The key for display (lossy).
    pub(crate) display: String,
    pub(crate) category: CategoryName,
    pub(crate) presence: Vec<u32>,
    /// Equal by size and time only (fast mode), not verified.
    pub(crate) assumed: bool,
    #[specta(type = Option<Number>)]
    pub(crate) size: Option<u64>,
    pub(crate) cells: Vec<CellView>,
}

impl From<Row> for CompareRowView {
    fn from(row: Row) -> Self {
        Self {
            display: PathKey::from_stored(row.key.clone()).display(),
            key: row.key,
            category: row.category.into(),
            presence: row
                .presence
                .iter()
                .filter_map(|side| u32::try_from(side).ok())
                .collect(),
            assumed: row.assumed,
            size: row.size,
            cells: row.cells.iter().map(CellView::from).collect(),
        }
    }
}

/// A child in the comparison tree: a folder with matches beneath it, or a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TreeNodeView {
    /// The key: a row's key, or the folder key to expand.
    pub(crate) key: String,
    /// The last part of the path, for display (lossy).
    pub(crate) name: String,
    pub(crate) folder: bool,
    pub(crate) row: Option<CompareRowView>,
    #[specta(type = Number)]
    pub(crate) matching_rows: u64,
    #[specta(type = Number)]
    pub(crate) matching_bytes: u64,
}

impl TreeNodeView {
    fn from_child(child: TreeChild) -> Option<Self> {
        Some(match child {
            TreeChild::Row(row) => {
                let row = CompareRowView::from(row);
                Self {
                    key: row.key.clone(),
                    name: row
                        .display
                        .rsplit('/')
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                    folder: false,
                    matching_rows: 0,
                    matching_bytes: 0,
                    row: Some(row),
                }
            }
            TreeChild::Folder {
                key,
                display,
                row,
                matching,
                ..
            } => Self {
                key,
                name: display,
                folder: true,
                row: row.map(CompareRowView::from),
                matching_rows: matching.rows,
                matching_bytes: matching.bytes,
            },
            _ => return None,
        })
    }
}

/// Rows and bytes of one extension; no extension means all the others together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtensionView {
    pub(crate) extension: Option<String>,
    #[specta(type = Number)]
    pub(crate) rows: u64,
    #[specta(type = Number)]
    pub(crate) bytes: u64,
}

/// Rows and bytes whose largest file is below `max_exclusive` (and at or above the bucket before).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SizeBucketView {
    #[specta(type = Option<Number>)]
    pub(crate) max_exclusive: Option<u64>,
    #[specta(type = Number)]
    pub(crate) rows: u64,
    #[specta(type = Number)]
    pub(crate) bytes: u64,
}

/// Files with the same content within one side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DuplicateSetView {
    pub(crate) digest_short: String,
    #[specta(type = Number)]
    pub(crate) size: u64,
    #[specta(type = Number)]
    pub(crate) redundant_bytes: u64,
    pub(crate) cells: Vec<CellView>,
}

impl From<DuplicateSet> for DuplicateSetView {
    fn from(set: DuplicateSet) -> Self {
        Self {
            digest_short: short_hex(&set.digest.to_hex()),
            size: set.size,
            redundant_bytes: set.redundant_bytes(),
            cells: set.cells.iter().map(CellView::from).collect(),
        }
    }
}

/// Duplicate totals of one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DuplicateTotalsView {
    #[specta(type = Number)]
    pub(crate) groups: u64,
    #[specta(type = Number)]
    pub(crate) files: u64,
    #[specta(type = Number)]
    pub(crate) redundant_bytes: u64,
}

/// Background verification of a comparison.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub(crate) enum VerifyUpdate {
    /// Hashing within one location, in round `round`.
    Progress {
        label: String,
        round: u32,
        #[specta(type = Number)]
        files: u64,
        #[specta(type = Number)]
        bytes: u64,
        #[specta(type = Number)]
        total_files: u64,
        #[specta(type = Number)]
        total_bytes: u64,
    },
    /// The results were refreshed with what has been hashed so far.
    ResultsChanged {
        summary: SummaryView,
    },
    Done,
    Cancelled,
    Failed {
        error: ApiError,
    },
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct OpenComparison {
    catalog: Catalog,
    sides: Vec<Side>,
    options: CompareOptions,
    /// The comparison reads are served from; refreshed after each hashing round.
    reading: Mutex<Comparison>,
    sink: Mutex<Box<dyn Sink<VerifyUpdate>>>,
    verifying: Mutex<Verifying>,
    cancel: AtomicBool,
    closed: AtomicBool,
}

impl OpenComparison {
    fn send(&self, update: VerifyUpdate) {
        lock(&self.sink).send(update);
    }

    /// Re-reads the catalog into the reading comparison and reports the new totals.
    fn refresh(&self) -> Result<(), CompareError> {
        if self.closed.load(Ordering::Relaxed) {
            return Ok(());
        }
        let summary = {
            let mut reading = lock(&self.reading);
            reading.refresh()?;
            reading.summary()?
        };
        self.send(VerifyUpdate::ResultsChanged {
            summary: summary.into(),
        });
        Ok(())
    }

    /// Hashes what decides unverified rows, refreshing after every round.
    fn verify(&self) -> VerifyUpdate {
        let mut resolver = match Comparison::new(&self.catalog, &self.sides, self.options.clone()) {
            Ok(resolver) => resolver,
            Err(error) => {
                return VerifyUpdate::Failed {
                    error: error.into(),
                };
            }
        };
        let mut round = 0;
        let mut totals = (0, 0);
        let mut last_progress: Option<Instant> = None;
        let mut failed = None;
        let result = resolver.resolve(&ScanOptions::default(), |event| {
            if self.cancel.load(Ordering::Relaxed) || self.closed.load(Ordering::Relaxed) {
                return ControlFlow::Break(());
            }
            match event {
                ResolveEvent::Hashing {
                    label,
                    round: this,
                    files,
                    bytes,
                } => {
                    if this > round
                        && round > 0
                        && let Err(error) = self.refresh()
                    {
                        failed = Some(error);
                        return ControlFlow::Break(());
                    }
                    round = this;
                    totals = (files, bytes);
                    self.send(VerifyUpdate::Progress {
                        label,
                        round,
                        files: 0,
                        bytes: 0,
                        total_files: files,
                        total_bytes: bytes,
                    });
                }
                ResolveEvent::Progress {
                    label,
                    files,
                    bytes,
                } if last_progress.is_none_or(|at| at.elapsed() >= PROGRESS_INTERVAL) => {
                    last_progress = Some(Instant::now());
                    self.send(VerifyUpdate::Progress {
                        label,
                        round,
                        files,
                        bytes,
                        total_files: totals.0,
                        total_bytes: totals.1,
                    });
                }
                _ => {}
            }
            ControlFlow::Continue(())
        });
        if let Some(error) = failed {
            return VerifyUpdate::Failed {
                error: error.into(),
            };
        }
        match result {
            Err(error) => VerifyUpdate::Failed {
                error: error.into(),
            },
            Ok(summary) => {
                if summary.hashed_files > 0
                    && let Err(error) = self.refresh()
                {
                    return VerifyUpdate::Failed {
                        error: error.into(),
                    };
                }
                if summary.cancelled {
                    VerifyUpdate::Cancelled
                } else {
                    VerifyUpdate::Done
                }
            }
        }
    }
}

/// Starts verification of `open` on its own thread unless it is already running.
/// Whether a verification thread runs, and whether it should run again when it ends.
#[derive(Debug, Default)]
struct Verifying {
    running: bool,
    restart: bool,
}

/// Starts verification of `open` on its own thread. While a cancelled run is still winding
/// down, the request is remembered and that thread runs again instead of reporting the cancel.
fn spawn_verify(open: &Arc<OpenComparison>) {
    {
        let mut verifying = lock(&open.verifying);
        if verifying.running {
            if open.cancel.load(Ordering::Relaxed) {
                verifying.restart = true;
            }
            return;
        }
        verifying.running = true;
    }
    open.cancel.store(false, Ordering::Relaxed);
    let open = Arc::clone(open);
    std::thread::spawn(move || {
        loop {
            let outcome = open.verify();
            let mut verifying = lock(&open.verifying);
            if verifying.restart && !open.closed.load(Ordering::Relaxed) {
                verifying.restart = false;
                open.cancel.store(false, Ordering::Relaxed);
                continue;
            }
            // The outcome goes out before the run counts as over, so a new run's updates
            // always follow it.
            if !open.closed.load(Ordering::Relaxed) {
                open.send(outcome);
            }
            verifying.running = false;
            break;
        }
    });
}

/// Comparisons the UI has open, by id.
#[derive(Default)]
pub(crate) struct CompareRegistry {
    next: AtomicU32,
    open: Mutex<HashMap<u32, Arc<OpenComparison>>>,
}

impl CompareRegistry {
    /// Builds the comparison, returns its totals at once, and starts verifying it.
    pub(crate) fn open(
        &self,
        catalog: &Catalog,
        request: CompareRequest,
        sink: Box<dyn Sink<VerifyUpdate>>,
    ) -> Result<CompareOpened, ApiError> {
        let sides = request
            .sides
            .iter()
            .map(|side| match &side.subpath {
                Some(subpath) if !subpath.is_empty() => {
                    Side::subdir(side.label.clone(), Path::new(subpath))
                }
                _ => Ok(Side::location(side.label.clone())),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut options = CompareOptions::default()
            .keys(KeyOptions {
                nfc: request.nfc,
                case_insensitive: request.case_insensitive,
            })
            .duplicates(request.duplicates);
        if let Some(fast) = request.fast_ms {
            options = options.assume_equal_within(Duration::from_millis(fast.0));
        }
        let reading = Comparison::new(catalog, &sides, options.clone())?;
        let summary = reading.summary()?;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let open = Arc::new(OpenComparison {
            catalog: catalog.clone(),
            sides,
            options,
            reading: Mutex::new(reading),
            sink: Mutex::new(sink),
            verifying: Mutex::new(Verifying::default()),
            cancel: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        });
        lock(&self.open).insert(id, Arc::clone(&open));
        spawn_verify(&open);
        Ok(CompareOpened {
            id,
            sides: request.sides,
            summary: summary.into(),
        })
    }

    fn get(&self, id: u32) -> Result<Arc<OpenComparison>, ApiError> {
        lock(&self.open).get(&id).cloned().ok_or_else(|| {
            ApiError::new(ErrorKind::NotFound, format!("comparison {id} is not open"))
        })
    }

    /// Runs `read` on comparison `id`, waiting while it refreshes.
    fn read<T>(
        &self,
        id: u32,
        read: impl FnOnce(&Comparison) -> Result<T, CompareError>,
    ) -> Result<T, ApiError> {
        let open = self.get(id)?;
        let reading = lock(&open.reading);
        Ok(read(&reading)?)
    }

    pub(crate) fn verify(&self, id: u32) -> Result<(), ApiError> {
        spawn_verify(&self.get(id)?);
        Ok(())
    }

    pub(crate) fn cancel_verify(&self, id: u32) -> Result<(), ApiError> {
        let open = self.get(id)?;
        lock(&open.verifying).restart = false;
        open.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Stops verification and drops the comparison and its temporary tables.
    pub(crate) fn close(&self, id: u32) -> Result<(), ApiError> {
        let open = lock(&self.open).remove(&id).ok_or_else(|| {
            ApiError::new(ErrorKind::NotFound, format!("comparison {id} is not open"))
        })?;
        open.closed.store(true, Ordering::Relaxed);
        open.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub(crate) fn close_all(&self) {
        for (_, open) in lock(&self.open).drain() {
            open.closed.store(true, Ordering::Relaxed);
            open.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn summary(&self, id: u32, filter: ApiFilter) -> Result<SummaryView, ApiError> {
        let filter = RowFilter::from(filter);
        self.read(id, |c| c.filtered_summary(&filter))
            .map(Into::into)
    }

    pub(crate) fn count(&self, id: u32, filter: ApiFilter) -> Result<u64, ApiError> {
        let filter = RowFilter::from(filter);
        self.read(id, |c| c.count(&filter))
    }

    pub(crate) fn rows(
        &self,
        id: u32,
        filter: ApiFilter,
        order: RowOrderView,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<CompareRowView>, ApiError> {
        let filter = RowFilter::from(filter);
        let page = self.read(id, |c| {
            c.rows(&filter, order.into(), &Page::at(offset, limit))
        })?;
        Ok(page.items.into_iter().map(CompareRowView::from).collect())
    }

    pub(crate) fn child_count(
        &self,
        id: u32,
        filter: ApiFilter,
        parent: &str,
    ) -> Result<u64, ApiError> {
        let filter = RowFilter::from(filter);
        self.read(id, |c| c.child_count(&filter, parent))
    }

    pub(crate) fn children(
        &self,
        id: u32,
        filter: ApiFilter,
        parent: &str,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<TreeNodeView>, ApiError> {
        let filter = RowFilter::from(filter);
        let page = self.read(id, |c| {
            c.children(&filter, parent, &Page::at(offset, limit))
        })?;
        Ok(page
            .items
            .into_iter()
            .filter_map(TreeNodeView::from_child)
            .collect())
    }

    pub(crate) fn by_extension(
        &self,
        id: u32,
        filter: ApiFilter,
    ) -> Result<Vec<ExtensionView>, ApiError> {
        let filter = RowFilter::from(filter);
        let totals = self.read(id, |c| c.by_extension(&filter, TOP_EXTENSIONS))?;
        Ok(totals
            .into_iter()
            .map(|t| ExtensionView {
                extension: t.extension,
                rows: t.rows,
                bytes: t.bytes,
            })
            .collect())
    }

    pub(crate) fn size_histogram(
        &self,
        id: u32,
        filter: ApiFilter,
    ) -> Result<Vec<SizeBucketView>, ApiError> {
        let filter = RowFilter::from(filter);
        let buckets = self.read(id, |c| c.size_histogram(&filter))?;
        Ok(buckets
            .into_iter()
            .map(|b| SizeBucketView {
                max_exclusive: b.max_exclusive,
                rows: b.rows,
                bytes: b.bytes,
            })
            .collect())
    }

    pub(crate) fn duplicates(
        &self,
        id: u32,
        side: u32,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<DuplicateSetView>, ApiError> {
        let page = self.read(id, |c| {
            c.duplicates(side as usize, &Page::at(offset, limit))
        })?;
        Ok(page.items.into_iter().map(DuplicateSetView::from).collect())
    }

    pub(crate) fn duplicate_totals(&self, id: u32) -> Result<Vec<DuplicateTotalsView>, ApiError> {
        let totals = self.read(id, Comparison::duplicate_totals)?;
        Ok(totals
            .into_iter()
            .map(|t| DuplicateTotalsView {
                groups: t.groups,
                files: t.files,
                redundant_bytes: t.redundant_bytes,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use symbolon_core::catalog::{HashPolicy, NewLocation};
    use symbolon_core::compare::Totals;
    use symbolon_core::hash::HashAlgo;
    use symbolon_core::scan::Scanner;

    use super::*;
    use crate::sink::testing::{Collect, Gone};
    use crate::state::testing::state;

    struct Fixture {
        dir: tempfile::TempDir,
        catalog: Catalog,
        registry: CompareRegistry,
    }

    /// `left` and `right`, lazily hashed: `same.txt` equal, `diff.txt` same size but different,
    /// plus `n` equal files in `bulk/`, all unverified until hashed.
    fn fixture(n: usize) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let s = state(dir.path(), Vec::new());
        for (label, diff) in [("left", "aaaa"), ("right", "bbbb")] {
            let root = dir.path().join(label);
            fs::create_dir_all(root.join("bulk")).unwrap();
            fs::write(root.join("same.txt"), "same").unwrap();
            fs::write(root.join("diff.txt"), diff).unwrap();
            for i in 0..n {
                fs::write(root.join(format!("bulk/f{i:04}")), vec![b'x'; 4096 + i]).unwrap();
            }
            s.catalog
                .add_location(NewLocation::local(label, &root).hash_policy(HashPolicy::Lazy))
                .unwrap();
            Scanner::start(&s.catalog, label, ScanOptions::default())
                .unwrap()
                .wait()
                .unwrap();
        }
        Fixture {
            dir,
            catalog: s.catalog.clone(),
            registry: CompareRegistry::default(),
        }
    }

    fn request(sides: &[&str]) -> CompareRequest {
        CompareRequest {
            sides: sides
                .iter()
                .map(|label| SideRequest {
                    label: (*label).into(),
                    subpath: None,
                })
                .collect(),
            nfc: false,
            case_insensitive: false,
            fast_ms: None,
            duplicates: false,
        }
    }

    fn rows_of(summary: &SummaryView, category: CategoryName) -> u64 {
        summary
            .categories
            .iter()
            .find(|t| t.category == category)
            .map_or(0, |t| t.rows)
    }

    fn wait_for(sink: &Collect<VerifyUpdate>, done: impl Fn(&VerifyUpdate) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !sink.all().iter().any(&done) {
            assert!(
                Instant::now() < deadline,
                "no final update: {:?}",
                sink.all()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn is_end(update: &VerifyUpdate) -> bool {
        matches!(
            update,
            VerifyUpdate::Done | VerifyUpdate::Cancelled | VerifyUpdate::Failed { .. }
        )
    }

    #[test]
    fn summary_names_the_sides_holding_only_in_rows() {
        let mut summary = Summary::default();
        let mut totals = Totals::default();
        totals.rows = 3;
        totals.entries = 4;
        totals.bytes = 50;
        summary.only_in.insert(SideSet::from_bits(0b101), totals);
        let view = SummaryView::from(summary);
        assert_eq!(
            view.only_in,
            [OnlyInTotals {
                sides: vec![0, 2],
                rows: 3,
                entries: 4,
                bytes: 50,
                assumed_rows: 0,
            }]
        );
    }

    #[test]
    fn open_returns_summary_with_unverified_before_hashing() {
        let f = fixture(0);
        let opened = f
            .registry
            .open(&f.catalog, request(&["left", "right"]), Box::new(Gone))
            .unwrap();
        assert_eq!(rows_of(&opened.summary, CategoryName::Unverified), 2);
        assert_eq!(opened.sides.len(), 2);
    }

    #[test]
    fn verification_reclassifies_and_reports_changes() {
        let f = fixture(0);
        let sink = Collect::default();
        let opened = f
            .registry
            .open(
                &f.catalog,
                request(&["left", "right"]),
                Box::new(sink.clone()),
            )
            .unwrap();
        wait_for(&sink, is_end);
        let updates = sink.all();
        assert_eq!(updates.last(), Some(&VerifyUpdate::Done), "{updates:?}");
        assert!(
            updates
                .iter()
                .any(|u| matches!(u, VerifyUpdate::ResultsChanged { .. }))
        );
        let summary = f.registry.summary(opened.id, ApiFilter::default()).unwrap();
        assert_eq!(rows_of(&summary, CategoryName::Unverified), 0);
        assert_eq!(rows_of(&summary, CategoryName::Identical), 1);
        assert_eq!(rows_of(&summary, CategoryName::Different), 1);
    }

    #[test]
    fn cancel_verify_stops_and_verify_restarts() {
        let f = fixture(400);
        let sink = Collect::default();
        let opened = f
            .registry
            .open(
                &f.catalog,
                request(&["left", "right"]),
                Box::new(sink.clone()),
            )
            .unwrap();
        f.registry.cancel_verify(opened.id).unwrap();
        wait_for(&sink, is_end);
        let first = sink.all().into_iter().find(is_end);
        // Verification may finish before the cancel lands on a fast machine.
        assert!(
            matches!(first, Some(VerifyUpdate::Cancelled | VerifyUpdate::Done)),
            "{first:?}"
        );
        f.registry.verify(opened.id).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while sink.all().iter().filter(|u| is_end(u)).count() < 2 {
            assert!(Instant::now() < deadline, "verify did not restart");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(sink.all().last(), Some(&VerifyUpdate::Done));
        let summary = f.registry.summary(opened.id, ApiFilter::default()).unwrap();
        assert_eq!(rows_of(&summary, CategoryName::Unverified), 0);
    }

    #[test]
    fn verify_right_after_cancel_restarts() {
        let f = fixture(400);
        let sink = Collect::default();
        let opened = f
            .registry
            .open(
                &f.catalog,
                request(&["left", "right"]),
                Box::new(sink.clone()),
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        f.registry.cancel_verify(opened.id).unwrap();
        f.registry.verify(opened.id).unwrap();
        wait_for(&sink, |u| *u == VerifyUpdate::Done);
        let summary = f.registry.summary(opened.id, ApiFilter::default()).unwrap();
        assert_eq!(rows_of(&summary, CategoryName::Unverified), 0);
        // A restart that took over the cancelled run reports no end of its own before Done.
        let ends: Vec<_> = sink.all().into_iter().filter(is_end).collect();
        assert_eq!(ends.last(), Some(&VerifyUpdate::Done));
    }

    #[test]
    fn close_all_closes_every_comparison() {
        let f = fixture(0);
        let a = f
            .registry
            .open(&f.catalog, request(&["left", "right"]), Box::new(Gone))
            .unwrap();
        let b = f
            .registry
            .open(&f.catalog, request(&["right", "left"]), Box::new(Gone))
            .unwrap();
        f.registry.close_all();
        for id in [a.id, b.id] {
            assert_eq!(
                f.registry.count(id, ApiFilter::default()).unwrap_err().kind,
                ErrorKind::NotFound
            );
        }
    }

    #[test]
    fn reads_after_close_are_not_found() {
        let f = fixture(0);
        let opened = f
            .registry
            .open(&f.catalog, request(&["left", "right"]), Box::new(Gone))
            .unwrap();
        f.registry.close(opened.id).unwrap();
        let err = f
            .registry
            .count(opened.id, ApiFilter::default())
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
        assert_eq!(
            f.registry.close(opened.id).unwrap_err().kind,
            ErrorKind::NotFound
        );
    }

    #[test]
    fn close_during_verification_returns_promptly() {
        let f = fixture(400);
        let opened = f
            .registry
            .open(&f.catalog, request(&["left", "right"]), Box::new(Gone))
            .unwrap();
        let started = Instant::now();
        f.registry.close(opened.id).unwrap();
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn algorithm_mismatch_is_reported() {
        let f = fixture(0);
        let root = f.dir.path().join("sha");
        fs::create_dir(&root).unwrap();
        f.catalog
            .add_location(NewLocation::local("sha", &root).hash_algo(HashAlgo::Sha256))
            .unwrap();
        let err = f
            .registry
            .open(&f.catalog, request(&["left", "sha"]), Box::new(Gone))
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::AlgorithmMismatch);
        assert!(err.message.contains("left") && err.message.contains("sha"));
    }

    #[test]
    fn rows_and_children_page_by_offset() {
        let f = fixture(30);
        let sink = Collect::default();
        let opened = f
            .registry
            .open(
                &f.catalog,
                request(&["left", "right"]),
                Box::new(sink.clone()),
            )
            .unwrap();
        wait_for(&sink, is_end);
        let all = ApiFilter::default();
        let order = RowOrderView {
            by: OrderBy::Size,
            desc: true,
        };
        let rows = f
            .registry
            .rows(opened.id, all.clone(), order, 0, 5)
            .unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].display, "bulk/f0029");
        assert_eq!(f.registry.count(opened.id, all.clone()).unwrap(), 32);
        assert_eq!(
            f.registry.child_count(opened.id, all.clone(), "").unwrap(),
            3
        );
        let bulk = f
            .registry
            .children(opened.id, all.clone(), "bulk", 10, 5)
            .unwrap();
        assert_eq!(bulk[0].name, "f0010");
        assert!(!bulk[0].folder);
        let root = f.registry.children(opened.id, all, "", 0, 10).unwrap();
        assert!(root[0].folder);
        assert_eq!(root[0].matching_rows, 30);
    }

    #[test]
    fn filters_select_categories_and_presence() {
        let f = fixture(0);
        let sink = Collect::default();
        let opened = f
            .registry
            .open(
                &f.catalog,
                request(&["left", "right"]),
                Box::new(sink.clone()),
            )
            .unwrap();
        wait_for(&sink, is_end);
        let different = ApiFilter {
            categories: vec![CategoryName::Different],
            ..ApiFilter::default()
        };
        assert_eq!(f.registry.count(opened.id, different).unwrap(), 1);
        let only_left = ApiFilter {
            presence: Some(vec![0]),
            ..ApiFilter::default()
        };
        assert_eq!(f.registry.count(opened.id, only_left).unwrap(), 0);
    }
}
