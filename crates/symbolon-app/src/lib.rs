//! Tauri backend: thin command wrappers over `symbolon-core` (ADR-0024).

mod bindings;
mod browse;
mod commands;
mod compare;
mod dto;
mod error;
mod locations;
mod scans;
mod sink;
mod state;

use std::sync::Arc;
use std::time::Duration;

use tauri::Manager as _;

use crate::state::AppState;

/// How long quitting waits for running scans to stop.
const EXIT_WAIT: Duration = Duration::from_secs(3);

/// Starts the desktop application and blocks until it exits.
///
/// # Errors
///
/// Returns an error if the webview runtime fails to start.
pub fn run() -> Result<(), tauri::Error> {
    let builder = bindings::builder();
    let app = tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            let state = AppState::open_default().map_err(|e| e.message)?;
            app.manage(Arc::new(state));
            Ok(())
        })
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        // Scans keep what they committed and are recorded as cancelled.
        if let tauri::RunEvent::Exit = event
            && let Some(state) = handle.try_state::<Arc<AppState>>()
        {
            state.comparisons.close_all();
            state.scans.cancel_all_and_wait(EXIT_WAIT);
        }
    });
    Ok(())
}
