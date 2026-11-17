//! The OpenDict desktop shell: Tauri commands mapped thinly onto `opendict-core`.

mod commands;
mod entry_protocol;
mod mini;
mod state;
mod window;

use tauri::Manager;

pub use state::{AppState, EntryView, LibraryView};

/// Environment variable naming one Library Folder to use instead of the reader's Library,
/// kept in memory only (used by tests and development).
pub const LIBRARY_FOLDER_ENV: &str = "OPENDICT_LIBRARY_FOLDER";

/// Environment variable that overrides the app's home folder: where the reader's data
/// starts, and where a moved data directory is recorded (used by tests and development).
pub const DATA_DIR_ENV: &str = "OPENDICT_DATA_DIR";

/// Environment variable naming a folder that Word Books are exported into without asking
/// the reader where (used by tests).
pub const EXPORT_DIR_ENV: &str = "OPENDICT_EXPORT_DIR";

/// Environment variable naming the folder the reader is taken to have chosen when asked
/// for a folder for their data, without asking (used by tests).
pub const PICK_DATA_FOLDER_ENV: &str = "OPENDICT_PICK_DATA_FOLDER";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(mini::shortcut_plugin())
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
            // Updates come from GitHub Releases, checked by the interface (see UpdateNotice.tsx).
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            let override_folder = std::env::var_os(LIBRARY_FOLDER_ENV).map(Into::into);
            let home = match std::env::var_os(DATA_DIR_ENV) {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            let state = AppState::load(&home, override_folder);
            let appearance = state.appearance();
            let data_dir = state.data_directory().path;
            commands::start_indexing(app.handle(), &state);
            app.manage(state);
            let chrome = window::build_main(app)?;
            app.manage(chrome);
            mini::set_up(app, &data_dir)?;
            // The reader's theme and zoom from the last session, as soon as the windows are
            // there (the Mini Window too): their frames and Mica follow the theme.
            commands::apply_theme(app.handle(), appearance.theme);
            commands::apply_zoom(app.handle(), appearance.zoom);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::look_up,
            commands::suggest,
            commands::headwords_after,
            commands::library,
            commands::add_library_folder,
            commands::remove_library_folder,
            commands::rescan_library,
            commands::set_dictionary_enabled,
            commands::move_dictionary,
            commands::register_dictionary,
            commands::index_library,
            commands::cancel_indexing,
            commands::set_allow_network,
            commands::appearance,
            commands::set_theme,
            commands::set_language,
            commands::set_as_authored,
            commands::set_all_as_authored,
            commands::set_text_size,
            commands::set_zoom,
            commands::word_books,
            commands::toggle_word,
            commands::create_word_book,
            commands::rename_word_book,
            commands::delete_word_book,
            commands::choose_word_book,
            commands::remove_saved_word,
            commands::set_word_note,
            commands::export_word_book,
            commands::history,
            commands::record_lookup,
            commands::clear_history,
            commands::create_group,
            commands::rename_group,
            commands::delete_group,
            commands::set_in_group,
            commands::move_in_group,
            commands::set_current_group,
            commands::elsewhere,
            commands::data_directory,
            commands::choose_data_folder,
            commands::move_data_directory,
            commands::use_data_directory,
            window::window_chrome,
            commands::mini_summons,
            commands::close_mini,
            commands::open_in_main,
            commands::mini_settings,
            commands::set_shortcut,
            commands::set_watch_clipboard,
        ])
        .run(tauri::generate_context!())
        .expect("OpenDict failed to start");
}
