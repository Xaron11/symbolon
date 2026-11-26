//! Comparisons: open, verify, read, close.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]

use tauri::ipc::Channel;

use super::{State, blocking};
use crate::compare::{
    ApiFilter, CompareOpened, CompareRequest, CompareRowView, DuplicateSetView,
    DuplicateTotalsView, ExtensionView, RowOrderView, SizeBucketView, SummaryView, TreeNodeView,
    VerifyUpdate,
};
use crate::dto::Num;
use crate::error::ApiError;

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_open(
    state: State<'_>,
    request: CompareRequest,
    on_update: Channel<VerifyUpdate>,
) -> Result<CompareOpened, ApiError> {
    blocking(&state, move |s| {
        s.comparisons.open(&s.catalog, request, Box::new(on_update))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) fn compare_close(state: State<'_>, id: u32) -> Result<(), ApiError> {
    state.comparisons.close(id)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn compare_verify(state: State<'_>, id: u32) -> Result<(), ApiError> {
    state.comparisons.verify(id)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn compare_cancel_verify(state: State<'_>, id: u32) -> Result<(), ApiError> {
    state.comparisons.cancel_verify(id)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_summary(
    state: State<'_>,
    id: u32,
    filter: ApiFilter,
) -> Result<SummaryView, ApiError> {
    blocking(&state, move |s| s.comparisons.summary(id, filter)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_count(
    state: State<'_>,
    id: u32,
    filter: ApiFilter,
) -> Result<Num<u64>, ApiError> {
    blocking(&state, move |s| s.comparisons.count(id, filter).map(Num)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_rows(
    state: State<'_>,
    id: u32,
    filter: ApiFilter,
    order: RowOrderView,
    offset: Num<u64>,
    limit: u32,
) -> Result<Vec<CompareRowView>, ApiError> {
    blocking(&state, move |s| {
        s.comparisons.rows(id, filter, order, offset.0, limit)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_child_count(
    state: State<'_>,
    id: u32,
    filter: ApiFilter,
    parent: String,
) -> Result<Num<u64>, ApiError> {
    blocking(&state, move |s| {
        s.comparisons.child_count(id, filter, &parent).map(Num)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_children(
    state: State<'_>,
    id: u32,
    filter: ApiFilter,
    parent: String,
    offset: Num<u64>,
    limit: u32,
) -> Result<Vec<TreeNodeView>, ApiError> {
    blocking(&state, move |s| {
        s.comparisons.children(id, filter, &parent, offset.0, limit)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_by_extension(
    state: State<'_>,
    id: u32,
    filter: ApiFilter,
) -> Result<Vec<ExtensionView>, ApiError> {
    blocking(&state, move |s| s.comparisons.by_extension(id, filter)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_size_histogram(
    state: State<'_>,
    id: u32,
    filter: ApiFilter,
) -> Result<Vec<SizeBucketView>, ApiError> {
    blocking(&state, move |s| s.comparisons.size_histogram(id, filter)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_duplicates(
    state: State<'_>,
    id: u32,
    side: u32,
    offset: Num<u64>,
    limit: u32,
) -> Result<Vec<DuplicateSetView>, ApiError> {
    blocking(&state, move |s| {
        s.comparisons.duplicates(id, side, offset.0, limit)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn compare_duplicate_totals(
    state: State<'_>,
    id: u32,
) -> Result<Vec<DuplicateTotalsView>, ApiError> {
    blocking(&state, move |s| s.comparisons.duplicate_totals(id)).await
}
