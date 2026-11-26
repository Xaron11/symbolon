//! Locations and devices.

use super::{State, blocking};
use crate::dto::{DeviceView, HashPolicyView, LocationView, NewLocationRequest, StatsView};
use crate::error::ApiError;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub(crate) async fn locations_list(state: State<'_>) -> Result<Vec<LocationView>, ApiError> {
    blocking(&state, AppState::locations_list).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_add(
    state: State<'_>,
    request: NewLocationRequest,
) -> Result<LocationView, ApiError> {
    blocking(&state, move |s| s.location_add(request)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_rename(
    state: State<'_>,
    label: String,
    new_label: String,
) -> Result<LocationView, ApiError> {
    blocking(&state, move |s| s.location_rename(&label, &new_label)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_remove(state: State<'_>, label: String) -> Result<(), ApiError> {
    blocking(&state, move |s| s.location_remove(&label)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_set_hash_policy(
    state: State<'_>,
    label: String,
    policy: HashPolicyView,
) -> Result<LocationView, ApiError> {
    blocking(&state, move |s| s.location_set_hash_policy(&label, policy)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_follow_move(
    state: State<'_>,
    label: String,
    choice: Option<u32>,
) -> Result<LocationView, ApiError> {
    blocking(&state, move |s| s.location_follow_move(&label, choice)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn location_stats(state: State<'_>, label: String) -> Result<StatsView, ApiError> {
    blocking(&state, move |s| s.location_stats(&label)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn devices_list(state: State<'_>) -> Result<Vec<DeviceView>, ApiError> {
    blocking(&state, AppState::devices_list).await
}
