fn main() {
    // Declaring the app's commands makes each one need an explicit permission in
    // capabilities/, so nothing is callable by accident.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "look_up",
        "suggest",
        "headwords_after",
        "set_allow_network",
        "library",
        "add_library_folder",
        "remove_library_folder",
        "rescan_library",
        "set_dictionary_enabled",
        "move_dictionary",
        "history",
        "record_lookup",
        "clear_history",
        "register_dictionary",
        "word_books",
        "toggle_word",
        "create_word_book",
        "rename_word_book",
        "delete_word_book",
        "choose_word_book",
        "remove_saved_word",
        "set_word_note",
        "export_word_book",
        "index_library",
        "cancel_indexing",
        "window_chrome",
        "mini_summons",
        "close_mini",
        "open_in_main",
        "mini_settings",
        "set_shortcut",
        "set_watch_clipboard",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
