//! Configuration: for now, it names the one Library Folder.

use opendict_core::Config;

#[test]
fn reads_the_library_folder_from_the_configuration_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.json");
    std::fs::write(&file, r#"{ "libraryFolder": "D:/Dictionaries" }"#).unwrap();

    let config = Config::load(&file).unwrap();

    assert_eq!(
        config.library_folder.as_deref(),
        Some(std::path::Path::new("D:/Dictionaries"))
    );
}

#[test]
fn a_missing_configuration_file_means_no_library_folder_yet() {
    let dir = tempfile::tempdir().unwrap();

    let config = Config::load(&dir.path().join("config.json")).unwrap();

    assert_eq!(config.library_folder, None);
}

#[test]
fn a_malformed_configuration_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.json");
    std::fs::write(&file, "{ libraryFolder: ").unwrap();

    assert!(Config::load(&file).is_err());
}
