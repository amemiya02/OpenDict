//! Enabling, disabling and reordering Dictionaries, and how that shapes Lookup.

use std::path::Path;

use opendict_core::{Library, Move};
use opendict_mdict_fixture::MdxBuilder;

fn put(folder: &Path, relative: &str, title: &str) {
    MdxBuilder::new()
        .title(title)
        .entry("apple", &format!("<p>apple in {title} (sample)</p>"))
        .write_to(folder.join(relative))
        .unwrap();
}

/// A Library of three Dictionaries, A, B and C, all having "apple".
fn abc(data: &Path, folder: &Path) -> Library {
    put(folder, "a.mdx", "A");
    put(folder, "b.mdx", "B");
    put(folder, "c.mdx", "C");
    let mut library = Library::load(&data.join("library.json")).unwrap();
    library.add_folder(folder).unwrap();
    library
}

fn id_of(library: &Library, title: &str) -> String {
    library
        .dictionaries()
        .find(|d| d.title == title)
        .unwrap()
        .id
        .clone()
}

fn titles(library: &Library) -> Vec<String> {
    library.dictionaries().map(|d| d.title.clone()).collect()
}

fn answering(library: &Library) -> Vec<String> {
    library
        .look_up("apple")
        .unwrap()
        .into_iter()
        .map(|entry| entry.dictionary_title)
        .collect()
}

#[test]
fn a_disabled_dictionary_stays_in_the_library_but_does_not_answer() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());

    let b = id_of(&library, "B");
    library.set_enabled(&b, false).unwrap();

    assert_eq!(titles(&library), ["A", "B", "C"]);
    assert!(!library.dictionaries().nth(1).unwrap().enabled);
    assert_eq!(answering(&library), ["A", "C"]);
}

#[test]
fn enabled_state_is_remembered_between_starts() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let b = id_of(&library, "B");
    library.set_enabled(&b, false).unwrap();
    library.set_enabled(&b, true).unwrap();
    let c = id_of(&library, "C");
    library.set_enabled(&c, false).unwrap();
    drop(library);

    let library = Library::load(&data.path().join("library.json")).unwrap();

    assert_eq!(answering(&library), ["A", "B"]);
}

#[test]
fn moving_a_dictionary_changes_the_order_dictionaries_answer_in() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());

    let c = id_of(&library, "C");
    library.move_dictionary(&c, Move::Up).unwrap();
    let a = id_of(&library, "A");
    library.move_dictionary(&a, Move::Down).unwrap();

    assert_eq!(titles(&library), ["C", "A", "B"]);
    assert_eq!(answering(&library), ["C", "A", "B"]);
}

#[test]
fn moving_past_either_end_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());

    let a = id_of(&library, "A");
    library.move_dictionary(&a, Move::Up).unwrap();
    let c = id_of(&library, "C");
    library.move_dictionary(&c, Move::Down).unwrap();

    assert_eq!(titles(&library), ["A", "B", "C"]);
}

#[test]
fn library_order_is_remembered_and_newcomers_join_at_the_end() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let c = id_of(&library, "C");
    library.move_dictionary(&c, Move::Up).unwrap();
    library.move_dictionary(&c, Move::Up).unwrap();
    drop(library);

    put(folder.path(), "0-first-by-name.mdx", "Newcomer");
    let library = Library::load(&data.path().join("library.json")).unwrap();

    assert_eq!(titles(&library), ["C", "A", "B", "Newcomer"]);
}

#[test]
fn an_unknown_key_is_an_error() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());

    assert!(library.set_enabled("no-such-key", false).is_err());
    assert!(library.move_dictionary("no-such-key", Move::Up).is_err());
}
