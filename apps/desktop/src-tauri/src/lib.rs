//! The OpenDict desktop shell: Tauri commands mapped thinly onto `opendict-core`.

mod commands;
mod entry_protocol;
mod state;

use tauri::Manager;

pub use state::{AppState, EntryView, LibraryView};

/// Environment variable naming one Library Folder to use instead of the reader's Library,
/// kept in memory only (used by tests and development).
pub const LIBRARY_FOLDER_ENV: &str = "OPENDICT_LIBRARY_FOLDER";

/// Environment variable that overrides the data directory, where the Library file and
/// preferences are kept (used by tests and development).
pub const DATA_DIR_ENV: &str = "OPENDICT_DATA_DIR";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            let override_folder = std::env::var_os(LIBRARY_FOLDER_ENV).map(Into::into);
            let data_dir = match std::env::var_os(DATA_DIR_ENV) {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            let library_file = data_dir.join("library.json");
            app.manage(AppState::load(&library_file, override_folder, &data_dir));
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
            commands::set_allow_network,
        ])
        .run(tauri::generate_context!())
        .expect("OpenDict failed to start");
}
