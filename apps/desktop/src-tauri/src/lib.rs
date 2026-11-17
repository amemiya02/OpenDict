//! The OpenDict desktop shell: Tauri commands mapped thinly onto `opendict-core`.

mod commands;
mod state;

use tauri::Manager;

pub use state::{AppState, EntryView, LibraryView};

/// Environment variable naming one Library Folder to use instead of the reader's Library,
/// kept in memory only (used by tests and development).
pub const LIBRARY_FOLDER_ENV: &str = "OPENDICT_LIBRARY_FOLDER";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let library_file = app.path().app_data_dir()?.join("library.json");
            let override_folder = std::env::var_os(LIBRARY_FOLDER_ENV).map(Into::into);
            app.manage(AppState::load(&library_file, override_folder));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::look_up,
            commands::library,
            commands::add_library_folder,
            commands::remove_library_folder,
            commands::rescan_library,
            commands::set_dictionary_enabled,
            commands::move_dictionary,
        ])
        .run(tauri::generate_context!())
        .expect("OpenDict failed to start");
}
