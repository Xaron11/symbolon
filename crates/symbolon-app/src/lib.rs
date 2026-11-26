//! Tauri backend: thin command wrappers over `symbolon-core` (ADR-0024).

mod bindings;
mod commands;
mod dto;
mod error;
mod locations;
mod state;

use std::sync::Arc;

use crate::state::AppState;

/// Starts the desktop application and blocks until it exits.
///
/// # Errors
///
/// Returns an error if the webview runtime fails to start.
pub fn run() -> Result<(), tauri::Error> {
    let builder = bindings::builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            let state = AppState::open_default().map_err(|e| e.message)?;
            tauri::Manager::manage(app, Arc::new(state));
            Ok(())
        })
        .run(tauri::generate_context!())
}
