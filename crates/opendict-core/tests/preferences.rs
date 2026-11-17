//! Per-Dictionary preferences, kept in a file the core owns.

use opendict_core::Preferences;

#[test]
fn dictionaries_have_no_network_access_by_default() {
    let folder = tempfile::tempdir().unwrap();

    let preferences = Preferences::load(&folder.path().join("preferences.json")).unwrap();

    assert!(!preferences.allows_network("english/sample.mdx"));
}

#[test]
fn allowing_network_access_is_remembered_for_that_dictionary_only() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    preferences.set_allows_network("english/sample.mdx", true).unwrap();

    let reloaded = Preferences::load(&file).unwrap();
    assert!(reloaded.allows_network("english/sample.mdx"));
    assert!(!reloaded.allows_network("japanese/sample.mdx"));
}

#[test]
fn network_access_can_be_withdrawn() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("data").join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    preferences.set_allows_network("sample.mdx", true).unwrap();
    preferences.set_allows_network("sample.mdx", false).unwrap();

    assert!(!Preferences::load(&file).unwrap().allows_network("sample.mdx"));
}

#[test]
fn a_damaged_preferences_file_is_an_error() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");
    std::fs::write(&file, "{ not json").unwrap();

    assert!(Preferences::load(&file).is_err());
}
