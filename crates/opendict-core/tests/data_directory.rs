//! The data directory through the public interface of `opendict-core`: every file the
//! reader's data is kept in lives in one directory, which can be moved as a whole; the app
//! finds it in its new place from then on, and a move that fails or is interrupted never
//! loses or half-moves anything.

use std::ops::ControlFlow;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use opendict_core::{DataDirectory, History, Library, Preferences, SavedFrom, WordBooks};
use opendict_mdict_fixture::MdxBuilder;

fn sample() -> MdxBuilder {
    MdxBuilder::new()
        .title("Sample")
        .entry("apple", "<p>a round fruit (sample)</p>")
        .entry("bank", "<p>a river side (sample)</p>")
}

/// Fills `data` the way the app does: a Library with one indexed Dictionary from
/// `dictionaries`, History, a Word Book and a per-Dictionary preference.
fn fill(data: &DataDirectory, dictionaries: &Path) {
    sample().write_to(&dictionaries.join("sample.mdx")).unwrap();
    let mut library = Library::load(&data.library_file()).unwrap();
    library.add_folder(dictionaries).unwrap();
    library.build_indexes();
    let id = library.dictionaries().next().unwrap().id.clone();
    drop(library);

    let mut history = History::load(&data.history_file()).unwrap();
    history.record("apple").unwrap();
    history.record("bank").unwrap();

    let mut books = WordBooks::load(&data.word_books_file()).unwrap();
    let book = books.create("Reading").unwrap();
    let from = SavedFrom {
        dictionary_id: id.clone(),
        dictionary_title: "Sample".to_owned(),
    };
    books
        .toggle(book, "apple", from, UNIX_EPOCH + Duration::from_secs(1_700_000_000))
        .unwrap();

    let mut preferences = Preferences::load(&data.preferences_file()).unwrap();
    preferences.set_allows_network(&id, true).unwrap();
}

/// The names the reader's data is kept under.
const DATA: [&str; 5] = [
    "history.json",
    "indexes",
    "library.json",
    "preferences.json",
    "word-books.json",
];

fn assert_no_data_in(folder: &Path) {
    let left: Vec<String> = names(folder)
        .into_iter()
        .filter(|name| DATA.contains(&name.as_str()))
        .collect();
    assert!(left.is_empty(), "data left in {}: {left:?}", folder.display());
}

/// The names in `folder`, sorted.
fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_new_reader_keeps_all_their_data_in_one_directory_in_the_home_folder() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();

    let data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    assert_eq!(data.path(), home.path());
    assert!(data.is_default());
    assert_eq!(names(home.path()), DATA);
}

/// Moves `data` to `to` without watching progress.
fn relocate(data: &mut DataDirectory, to: &Path) -> opendict_core::Result<()> {
    data.relocate(to, &mut |_| ControlFlow::Continue(()))
        .map(|moved| assert!(moved.is_some(), "the move was not cancelled"))
}

/// What a reader would see of their data kept in `data`.
fn contents(data: &DataDirectory) -> (Vec<String>, Vec<String>, Vec<String>, bool) {
    let library = Library::load(&data.library_file()).unwrap();
    assert!(
        library.index_jobs().is_empty(),
        "the stored indexes are used, not rebuilt"
    );
    let html = library
        .look_up("apple")
        .unwrap()
        .into_iter()
        .map(|entry| entry.html)
        .collect();
    let id = library.dictionaries().next().unwrap().id.clone();
    let history = History::load(&data.history_file()).unwrap().headwords().to_vec();
    let books = WordBooks::load(&data.word_books_file()).unwrap();
    let words = books.books()[0]
        .words
        .iter()
        .map(|word| format!("{} in {}", word.headword, books.books()[0].name))
        .collect();
    let network = Preferences::load(&data.preferences_file())
        .unwrap()
        .allows_network(&id);
    (html, history, words, network)
}

fn expected() -> (Vec<String>, Vec<String>, Vec<String>, bool) {
    (
        vec!["<p>a round fruit (sample)</p>".to_owned()],
        vec!["bank".to_owned(), "apple".to_owned()],
        vec!["apple in Reading".to_owned()],
        true,
    )
}

#[test]
fn relocating_moves_every_file_and_the_app_finds_the_data_there_from_then_on() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let to = synced.path().join("OpenDict");
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    relocate(&mut data, &to).unwrap();

    assert_eq!(data.path(), to);
    assert!(!data.is_default());
    assert_eq!(names(&to), DATA);
    assert_eq!(contents(&data), expected());
    let after_restart = DataDirectory::locate(home.path()).unwrap();
    assert_eq!(after_restart.path(), to);
    assert_eq!(after_restart.library_file(), to.join("library.json"));
    assert_eq!(contents(&after_restart), expected());
    assert_no_data_in(home.path());
}
