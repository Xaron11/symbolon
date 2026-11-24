//! Tauri backend: thin command wrappers over `symbolon-core`.

#[tauri::command]
fn core_version() -> &'static str {
    symbolon_core::VERSION
}

/// Starts the desktop application and blocks until it exits.
///
/// # Errors
///
/// Returns an error if the webview runtime fails to start.
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
}
