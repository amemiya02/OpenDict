fn main() {
    // Declaring the app's commands makes each one need an explicit permission in
    // capabilities/, so nothing is callable by accident.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "look_up",
        "suggest",
        "headwords_after",
        "library",
        "add_library_folder",
        "remove_library_folder",
        "rescan_library",
        "set_dictionary_enabled",
        "move_dictionary",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
