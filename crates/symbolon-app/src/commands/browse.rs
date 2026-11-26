//! Browsing a location's catalog.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]

use tauri::ipc::Channel;

use super::{State, blocking};
use crate::browse::EntryView;
use crate::dto::Num;
use crate::error::ApiError;
use crate::scans::BrowseChange;

#[tauri::command]
#[specta::specta]
pub(crate) async fn browse_child_count(
    state: State<'_>,
    label: String,
    parent: String,
) -> Result<Num<u64>, ApiError> {
    blocking(&state, move |s| {
        s.browse_child_count(&label, &parent).map(Num)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn browse_children(
    state: State<'_>,
    label: String,
    parent: String,
    offset: Num<u64>,
    limit: u32,
) -> Result<Vec<EntryView>, ApiError> {
    blocking(&state, move |s| {
        s.browse_children(&label, &parent, offset.0, limit)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) fn browse_subscribe(state: State<'_>, label: String, on_change: Channel<BrowseChange>) {
    state.scans.subscribe_browse(&label, Box::new(on_change));
}
