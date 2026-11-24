#[tauri::command]
fn core_version() -> &'static str {
    symbolon_core::VERSION
}

pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
}
