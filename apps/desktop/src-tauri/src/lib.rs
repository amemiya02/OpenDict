//! The OpenDict desktop shell: Tauri commands mapped thinly onto `opendict-core`.

mod commands;
mod state;

use tauri::Manager;

pub use state::{AppState, EntryView, LibraryStatus};

/// Environment variable that overrides the configured Library Folder (used by tests and development).
pub const LIBRARY_FOLDER_ENV: &str = "OPENDICT_LIBRARY_FOLDER";

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_file = app.path().app_config_dir()?.join("config.json");
            let override_folder = std::env::var_os(LIBRARY_FOLDER_ENV).map(Into::into);
            app.manage(AppState::load(config_file, override_folder));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::look_up,
            commands::library_status
        ])
        .run(tauri::generate_context!())
        .expect("OpenDict failed to start");
}
