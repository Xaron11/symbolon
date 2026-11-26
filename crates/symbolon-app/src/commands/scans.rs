//! Scans: start, follow, control, and their history.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]

use std::sync::Arc;

use tauri::ipc::Channel;

use super::{State, blocking};
use crate::dto::{Num, ScanBrief};
use crate::error::ApiError;
use crate::scans::{RecentScanView, ScanUpdate, WarningView};

#[tauri::command]
#[specta::specta]
pub(crate) async fn scan_start(
    state: State<'_>,
    label: String,
    on_update: Channel<ScanUpdate>,
) -> Result<(), ApiError> {
    blocking(&state, move |s| {
        Arc::clone(&s.scans).start(&s.catalog, &label, Box::new(on_update))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) fn scan_attach(
    state: State<'_>,
    label: String,
    on_update: Channel<ScanUpdate>,
) -> Result<(), ApiError> {
    state.scans.attach(&label, Box::new(on_update))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn scans_running(state: State<'_>) -> Vec<ScanUpdate> {
    state.scans.running()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn scan_pause(state: State<'_>, label: String) -> Result<(), ApiError> {
    state.scans.pause(&label)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn scan_resume(state: State<'_>, label: String) -> Result<(), ApiError> {
    state.scans.resume(&label)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn scan_cancel(state: State<'_>, label: String) -> Result<(), ApiError> {
    state.scans.cancel(&label)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn scans_recent(
    state: State<'_>,
    offset: Num<u64>,
    limit: u32,
) -> Result<Vec<RecentScanView>, ApiError> {
    blocking(&state, move |s| s.scans_recent(offset.0, limit)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn scans_recent_count(state: State<'_>) -> Result<Num<u64>, ApiError> {
    blocking(&state, |s| s.scans_recent_count().map(Num)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_scans(
    state: State<'_>,
    label: String,
    offset: Num<u64>,
    limit: u32,
) -> Result<Vec<ScanBrief>, ApiError> {
    blocking(&state, move |s| s.location_scans(&label, offset.0, limit)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_scan_count(
    state: State<'_>,
    label: String,
) -> Result<Num<u64>, ApiError> {
    blocking(&state, move |s| s.location_scan_count(&label).map(Num)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn scan_warnings(
    state: State<'_>,
    scan_id: Num<i64>,
    offset: Num<u64>,
    limit: u32,
) -> Result<Vec<WarningView>, ApiError> {
    blocking(&state, move |s| s.scan_warnings(scan_id.0, offset.0, limit)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn scan_warning_count(
    state: State<'_>,
    scan_id: Num<i64>,
) -> Result<Num<u64>, ApiError> {
    blocking(&state, move |s| s.scan_warning_count(scan_id.0).map(Num)).await
}
