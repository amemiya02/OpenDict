//! Per-Dictionary preferences, kept in a file the core owns.

use opendict_core::{Language, Preferences, Theme};

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
    preferences
        .set_allows_network("english/sample.mdx", true)
        .unwrap();

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

    assert!(
        !Preferences::load(&file)
            .unwrap()
            .allows_network("sample.mdx")
    );
}

#[test]
fn a_damaged_preferences_file_is_an_error() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");
    std::fs::write(&file, "{ not json").unwrap();

    assert!(Preferences::load(&file).is_err());
}

#[test]
fn the_theme_follows_the_system_until_the_reader_picks_one_and_is_remembered() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    assert_eq!(preferences.theme(), Theme::System);
    preferences.set_theme(Theme::Dark).unwrap();

    assert_eq!(Preferences::load(&file).unwrap().theme(), Theme::Dark);
}

#[test]
fn entries_are_recoloured_in_the_dark_theme_unless_shown_as_authored() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    assert!(!preferences.as_authored("maps.mdx"));
    preferences.set_as_authored("maps.mdx", true).unwrap();

    let reloaded = Preferences::load(&file).unwrap();
    assert!(reloaded.as_authored("maps.mdx"));
    assert!(!reloaded.as_authored("english.mdx"));
}

#[test]
fn every_dictionary_can_be_shown_as_authored_at_once_including_ones_added_later() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    preferences.set_all_as_authored(true).unwrap();

    let mut reloaded = Preferences::load(&file).unwrap();
    assert!(reloaded.all_as_authored());
    assert!(reloaded.as_authored("added-later.mdx"));

    // One Dictionary can still be recoloured on its own.
    reloaded.set_as_authored("maps.mdx", false).unwrap();
    assert!(!reloaded.as_authored("maps.mdx"));
    assert!(reloaded.as_authored("english.mdx"));
}

#[test]
fn the_global_switch_overrides_earlier_choices_for_single_dictionaries() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    preferences.set_as_authored("maps.mdx", true).unwrap();
    preferences.set_all_as_authored(false).unwrap();

    assert!(!Preferences::load(&file).unwrap().as_authored("maps.mdx"));
}

#[test]
fn text_size_and_page_zoom_start_at_the_defaults_and_are_remembered() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    assert_eq!(preferences.text_size(), 16);
    assert_eq!(preferences.zoom(), 100);
    assert_eq!(preferences.set_text_size(20).unwrap(), 20);
    assert_eq!(preferences.set_zoom(125).unwrap(), 125);

    let reloaded = Preferences::load(&file).unwrap();
    assert_eq!(reloaded.text_size(), 20);
    assert_eq!(reloaded.zoom(), 125);
}

#[test]
fn text_size_and_page_zoom_stay_within_readable_limits() {
    let folder = tempfile::tempdir().unwrap();
    let mut preferences = Preferences::load(&folder.path().join("preferences.json")).unwrap();

    assert_eq!(preferences.set_text_size(2).unwrap(), 10);
    assert_eq!(preferences.set_text_size(400).unwrap(), 32);
    assert_eq!(preferences.set_zoom(5).unwrap(), 50);
    assert_eq!(preferences.set_zoom(1000).unwrap(), 200);
}

#[test]
fn reading_preferences_and_network_access_live_side_by_side() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    preferences.set_allows_network("maps.mdx", true).unwrap();
    preferences.set_as_authored("maps.mdx", true).unwrap();
    preferences.set_allows_network("maps.mdx", false).unwrap();

    let reloaded = Preferences::load(&file).unwrap();
    assert!(reloaded.as_authored("maps.mdx"));
    assert!(!reloaded.allows_network("maps.mdx"));
}

#[test]
fn the_interface_language_follows_the_system_until_the_reader_picks_one_and_is_remembered() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    assert_eq!(preferences.language(), None);
    preferences.set_language(Some(Language::Ja)).unwrap();
    assert_eq!(
        Preferences::load(&file).unwrap().language(),
        Some(Language::Ja)
    );

    preferences.set_language(None).unwrap();
    assert_eq!(Preferences::load(&file).unwrap().language(), None);
}

#[test]
fn the_interface_language_is_stored_as_its_language_tag() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");

    let mut preferences = Preferences::load(&file).unwrap();
    preferences.set_language(Some(Language::ZhCn)).unwrap();

    let text = std::fs::read_to_string(&file).unwrap();
    let stored: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(stored["language"], "zh-CN");
    assert_eq!(serde_json::to_value(Language::En).unwrap(), "en");
}

#[test]
fn a_language_the_app_does_not_have_falls_back_to_the_system() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("preferences.json");
    std::fs::write(&file, r#"{ "theme": "dark", "language": "fr" }"#).unwrap();

    let preferences = Preferences::load(&file).unwrap();

    assert_eq!(preferences.language(), None);
    assert_eq!(preferences.theme(), Theme::Dark);
}
