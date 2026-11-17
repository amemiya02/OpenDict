//! The OpenDict desktop shell: Tauri commands mapped thinly onto `opendict-core`.

mod commands;
mod entry_protocol;
mod state;

use tauri::Manager;

pub use state::{AppState, EntryView, LibraryStatus};

/// Environment variable that overrides the configured Library Folder (used by tests and development).
pub const LIBRARY_FOLDER_ENV: &str = "OPENDICT_LIBRARY_FOLDER";

/// Environment variable that overrides where the reader's data (preferences) is kept (used
/// by tests and development).
pub const DATA_DIR_ENV: &str = "OPENDICT_DATA_DIR";

pub fn run() {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(
            entry_protocol::SCHEME,
            |context, request, responder| {
                let app = context.app_handle().clone();
                std::thread::spawn(move || match app.try_state::<AppState>() {
                    Some(state) => responder.respond(entry_protocol::respond(&state, &request)),
                    None => responder.respond(entry_protocol::not_ready()),
                });
            },
        )
        .setup(|app| {
            let config_file = app.path().app_config_dir()?.join("config.json");
            let override_folder = std::env::var_os(LIBRARY_FOLDER_ENV).map(Into::into);
            let data_dir = match std::env::var_os(DATA_DIR_ENV) {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            app.manage(AppState::load(config_file, override_folder, data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::look_up,
            commands::library_status,
            commands::set_allow_network
        ])
        .run(tauri::generate_context!())
        .expect("OpenDict failed to start");
}
