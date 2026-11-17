fn main() {
    // Declaring the app's commands makes each one need an explicit permission in
    // capabilities/, so nothing is callable by accident.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "look_up",
        "library_status",
        "set_allow_network",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
