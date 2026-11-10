//! `carrova-desktop` binary: starts the Tauri shell.

fn main() -> tauri::Result<()> {
    carrova_desktop::run()
}
