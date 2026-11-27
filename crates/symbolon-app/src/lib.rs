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

#[cfg(all(feature = "mcp-bridge", not(debug_assertions)))]
compile_error!("the mcp-bridge feature is for debug builds only (ADR-0025)");

/// Grants the MCP bridge its commands; added only when the feature is on, since a static
/// capability naming an absent plugin fails the build.
#[cfg(feature = "mcp-bridge")]
const MCP_CAPABILITY: &str =
    r#"{"identifier":"mcp-bridge","windows":["main"],"permissions":["mcp-bridge:default"]}"#;

/// How long quitting waits for running scans to stop.
const EXIT_WAIT: Duration = Duration::from_secs(3);

/// Starts the desktop application and blocks until it exits.
///
/// # Errors
///
/// Returns an error if the webview runtime fails to start.
pub fn run() -> Result<(), tauri::Error> {
    let builder = bindings::builder();
    let app = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    #[cfg(feature = "mcp-bridge")]
    let app = app.plugin(tauri_plugin_mcp_bridge::init_with_config(
        tauri_plugin_mcp_bridge::Config::localhost_only(),
    ));
    let app = app
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            #[cfg(feature = "mcp-bridge")]
            app.add_capability(MCP_CAPABILITY)?;
            let resources = app.path().resource_dir().ok();
            let state = AppState::open_default(resources.as_deref()).map_err(|e| e.message)?;
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
