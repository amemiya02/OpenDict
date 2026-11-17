//! The public interface of `opendict-core`: the headword column's suggestions.

use std::path::Path;

use opendict_core::{Library, Suggestion};
use opendict_mdict_fixture::MdxBuilder;

fn put(folder: &Path, relative: &str, dictionary: &MdxBuilder) {
    dictionary.write_to(folder.join(relative)).unwrap();
}

fn dictionary(title: &str, headwords: &[&str]) -> MdxBuilder {
    headwords
        .iter()
        .fold(MdxBuilder::new().title(title), |builder, headword| {
            builder.entry(headword, &format!("<p>{headword} ({title})</p>"))
        })
}

fn headwords(items: &[Suggestion]) -> Vec<&str> {
    items.iter().map(|item| item.headword.as_str()).collect()
}

#[test]
fn prefix_matches_come_first_and_the_column_continues_through_the_index() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "sample.mdx",
        &dictionary(
            "Sample",
            &["sequel", "apple", "serene", "serendipity", "shadow", "sea"],
        ),
    );
    let library = Library::open(folder.path()).unwrap();

    let suggestions = library.suggest("ser", 10).unwrap();

    assert_eq!(
        headwords(&suggestions.items),
        ["serendipity", "serene", "shadow"]
    );
    assert!(!suggestions.miss && !suggestions.browse && !suggestions.near);
    assert_eq!(suggestions.lemma, None);
}

#[test]
fn each_suggestion_counts_the_dictionaries_that_file_it() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "a.mdx",
        &dictionary("A", &["run", "runner", "rune"]),
    );
    put(folder.path(), "b.mdx", &dictionary("B", &["run", "rung"]));
    put(folder.path(), "c.mdx", &dictionary("C", &["run", "runner"]));
    let library = Library::open(folder.path()).unwrap();

    let suggestions = library.suggest("run", 10).unwrap();

    let counted: Vec<(&str, usize)> = suggestions
        .items
        .iter()
        .map(|item| (item.headword.as_str(), item.count))
        .collect();
    assert_eq!(
        counted,
        [("run", 3), ("rune", 1), ("rung", 1), ("runner", 2)]
    );
}

#[test]
fn a_disabled_dictionary_neither_suggests_nor_counts() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "a.mdx", &dictionary("A", &["run"]));
    put(folder.path(), "b.mdx", &dictionary("B", &["run", "rung"]));
    let mut library = Library::open(folder.path()).unwrap();
    let b = library
        .dictionaries()
        .find(|d| d.title == "B")
        .unwrap()
        .id
        .clone();

    library.set_enabled(&b, false).unwrap();
    let suggestions = library.suggest("ru", 10).unwrap();

    assert_eq!(headwords(&suggestions.items), ["run"]);
    assert_eq!(suggestions.items[0].count, 1);
}

#[test]
fn letter_case_does_not_separate_headwords_in_index_order() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "sample.mdx",
        &dictionary("Sample", &["banana", "Apple", "apricot", "Avocado"]),
    );
    let library = Library::open(folder.path()).unwrap();

    let suggestions = library.suggest("a", 10).unwrap();

    assert_eq!(
        headwords(&suggestions.items),
        ["Apple", "apricot", "Avocado", "banana"]
    );
    assert_eq!(
        headwords(&library.suggest("AP", 1).unwrap().items),
        ["Apple"]
    );
}

#[test]
fn an_empty_query_browses_the_index_from_its_start() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "sample.mdx",
        &dictionary("Sample", &["cherry", "apple", "banana"]),
    );
    let library = Library::open(folder.path()).unwrap();

    let suggestions = library.suggest("  ", 10).unwrap();

    assert!(suggestions.browse && !suggestions.miss);
    assert_eq!(headwords(&suggestions.items), ["apple", "banana", "cherry"]);
}

#[test]
fn a_miss_offers_the_headwords_after_where_the_query_would_stand() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "sample.mdx",
        &dictionary("Sample", &["apple", "banana", "cherry"]),
    );
    let library = Library::open(folder.path()).unwrap();

    let between = library.suggest("bz", 10).unwrap();
    let beyond = library.suggest("zebra", 10).unwrap();

    assert!(between.miss && !between.browse);
    assert_eq!(headwords(&between.items), ["cherry"]);
    assert!(beyond.miss);
    assert_eq!(beyond.items, []);
}

#[test]
fn the_column_stops_at_the_limit_and_continues_after_its_last_headword() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "a.mdx",
        &dictionary("A", &["a1", "a2", "a3", "a5"]),
    );
    put(
        folder.path(),
        "b.mdx",
        &dictionary("B", &["a2", "a4", "a6"]),
    );
    let library = Library::open(folder.path()).unwrap();

    let first = library.suggest("a", 3).unwrap();
    let next = library.headwords_after("a3", 2).unwrap();
    let rest = library.headwords_after("a5", 10).unwrap();

    assert_eq!(headwords(&first.items), ["a1", "a2", "a3"]);
    assert_eq!(first.items[1].count, 2);
    assert_eq!(headwords(&next), ["a4", "a5"]);
    assert_eq!(headwords(&rest), ["a6"]);
    assert_eq!(library.headwords_after("a6", 10).unwrap(), []);
}

#[test]
fn a_headword_filed_twice_in_one_dictionary_counts_once() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "sample.mdx",
        &MdxBuilder::new()
            .entry("bank", "<p>a river's edge</p>")
            .entry("bank", "<p>a place for money</p>"),
    );
    let library = Library::open(folder.path()).unwrap();

    let suggestions = library.suggest("bank", 10).unwrap();

    assert_eq!(headwords(&suggestions.items), ["bank"]);
    assert_eq!(suggestions.items[0].count, 1);
}
