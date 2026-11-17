//! History: the headwords the reader committed Lookups for, kept in a file the core owns.

use opendict_core::History;

#[test]
fn a_new_history_is_empty() {
    let folder = tempfile::tempdir().unwrap();

    let history = History::load(&folder.path().join("history.json")).unwrap();

    assert!(history.headwords().is_empty());
}

#[test]
fn lookups_are_listed_newest_first_and_kept_across_restarts() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("data").join("history.json");

    let mut history = History::load(&file).unwrap();
    history.record("apple").unwrap();
    history.record("serendipity").unwrap();
    history.record("食べる").unwrap();

    assert_eq!(history.headwords(), ["食べる", "serendipity", "apple"]);
    assert_eq!(
        History::load(&file).unwrap().headwords(),
        ["食べる", "serendipity", "apple"]
    );
}

#[test]
fn looking_a_word_up_again_moves_it_to_the_top_once() {
    let folder = tempfile::tempdir().unwrap();
    let mut history = History::load(&folder.path().join("history.json")).unwrap();

    history.record("apple").unwrap();
    history.record("serendipity").unwrap();
    history.record("apple").unwrap();

    assert_eq!(history.headwords(), ["apple", "serendipity"]);
}

#[test]
fn surrounding_space_and_empty_words_are_not_recorded() {
    let folder = tempfile::tempdir().unwrap();
    let mut history = History::load(&folder.path().join("history.json")).unwrap();

    history.record("  apple ").unwrap();
    history.record("   ").unwrap();

    assert_eq!(history.headwords(), ["apple"]);
}

#[test]
fn only_the_most_recent_lookups_are_kept() {
    let folder = tempfile::tempdir().unwrap();
    let mut history = History::load(&folder.path().join("history.json")).unwrap();

    for n in 0..History::LIMIT + 5 {
        history.record(&format!("word{n}")).unwrap();
    }

    assert_eq!(history.headwords().len(), History::LIMIT);
    assert_eq!(
        history.headwords()[0],
        format!("word{}", History::LIMIT + 4)
    );
    assert_eq!(history.headwords().last().unwrap(), "word5");
}

#[test]
fn clearing_history_forgets_every_lookup_for_good() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("history.json");
    let mut history = History::load(&file).unwrap();
    history.record("apple").unwrap();

    history.clear().unwrap();

    assert!(history.headwords().is_empty());
    assert!(History::load(&file).unwrap().headwords().is_empty());
}

#[test]
fn a_damaged_history_file_is_an_error() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("history.json");
    std::fs::write(&file, "[ not json").unwrap();

    assert!(History::load(&file).is_err());
}
