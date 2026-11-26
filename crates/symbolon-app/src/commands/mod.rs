//! `#[tauri::command]` wrappers: each runs one [`AppState`] method on a blocking thread.

pub(crate) mod compare;
pub(crate) mod locations;
pub(crate) mod scans;

use std::sync::Arc;

use crate::error::ApiError;
use crate::state::AppState;

/// The managed state as commands receive it.
pub(crate) type State<'a> = tauri::State<'a, Arc<AppState>>;

/// Runs `work` on the blocking pool, so catalog and file system calls never hold up the
/// window's event loop.
pub(crate) async fn blocking<T: Send + 'static>(
    state: &Arc<AppState>,
    work: impl FnOnce(&AppState) -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    let state = Arc::clone(state);
    tauri::async_runtime::spawn_blocking(move || work(&state))
        .await
        .map_err(|e| ApiError::internal(format!("command stopped: {e}")))?
}
