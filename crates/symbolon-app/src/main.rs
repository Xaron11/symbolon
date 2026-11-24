//! Desktop entry point; all setup lives in [`symbolon_app::run`].

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<(), tauri::Error> {
    symbolon_app::run()
}
