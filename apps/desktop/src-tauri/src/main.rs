//! `carrova-desktop` binary: starts the Tauri shell.

// Release builds are a GUI app on Windows (no console window); ignored elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> tauri::Result<()> {
    carrova_desktop::run()
}
