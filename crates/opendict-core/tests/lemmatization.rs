//! The public interface of `opendict-core`: Lemmatization in the headword column.

use std::path::Path;

use opendict_core::{Lemma, Library, Suggestion};
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

fn library_of(headwords: &[&str]) -> (tempfile::TempDir, Library) {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &dictionary("Sample", headwords));
    let library = Library::open(folder.path()).unwrap();
    (folder, library)
}

fn headwords(items: &[Suggestion]) -> Vec<&str> {
    items.iter().map(|item| item.headword.as_str()).collect()
}

fn lemma(from: &str, to: &str) -> Option<Lemma> {
    Some(Lemma {
        from: from.to_owned(),
        to: to.to_owned(),
    })
}

#[test]
fn an_inflected_form_finds_its_base_headword_first_with_a_note() {
    let (_folder, library) = library_of(&["apple", "serendipity", "serene", "shadow"]);

    let suggestions = library.suggest("serendipities", 10).unwrap();

    assert_eq!(suggestions.lemma, lemma("serendipities", "serendipity"));
    assert_eq!(headwords(&suggestions.items), ["serendipity", "serene", "shadow"]);
    assert!(!suggestions.miss && !suggestions.near && !suggestions.browse);
}

#[test]
fn a_verb_form_finds_its_verb() {
    let (_folder, library) = library_of(&["rumour", "run", "rune", "runner", "rush"]);

    let suggestions = library.suggest("running", 3).unwrap();

    assert_eq!(suggestions.lemma, lemma("running", "run"));
    assert_eq!(headwords(&suggestions.items), ["run", "rune", "runner"]);
}

#[test]
fn a_query_that_is_itself_a_headword_stays_first() {
    let (_folder, library) = library_of(&["run", "runs", "runway"]);

    let suggestions = library.suggest("runs", 10).unwrap();

    assert_eq!(headwords(&suggestions.items), ["runs", "run", "runway"]);
    assert_eq!(suggestions.lemma, lemma("runs", "run"));
}

#[test]
fn the_base_form_is_offered_only_when_an_enabled_dictionary_files_it() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "a.mdx", &dictionary("A", &["sea", "shadow"]));
    put(folder.path(), "b.mdx", &dictionary("B", &["serendipity"]));
    let mut library = Library::open(folder.path()).unwrap();
    let b = library
        .dictionaries()
        .find(|d| d.title == "B")
        .unwrap()
        .id
        .clone();

    library.set_enabled(&b, false).unwrap();
    let suggestions = library.suggest("serendipities", 10).unwrap();

    assert_eq!(suggestions.lemma, None);
    assert!(suggestions.miss);
    assert_eq!(headwords(&suggestions.items), ["shadow"]);
}

/// Looks each form up in a Library that files every expected base form (and some
/// tempting wrong ones) and none of the forms, and returns the forms that did not find
/// their base form.
fn wrong_lemmas<'a>(cases: &[(&'a str, &'a str)], distractors: &[&str]) -> Vec<(&'a str, String)> {
    let mut filed: Vec<&str> = cases.iter().map(|(_, base)| *base).collect();
    filed.extend(distractors);
    let (_folder, library) = library_of(&filed);
    cases
        .iter()
        .filter_map(|&(form, base)| {
            let found = library.suggest(form, 5).unwrap().lemma.map(|lemma| lemma.to);
            (found.as_deref() != Some(base)).then(|| (form, format!("{found:?}, not {base}")))
        })
        .collect()
}

#[test]
fn english_rules_cover_plurals_verb_forms_comparison_and_irregular_forms() {
    let cases = [
        ("cats", "cat"),
        ("boxes", "box"),
        ("cities", "city"),
        ("knives", "knife"),
        ("wolves", "wolf"),
        ("gives", "give"),
        ("goes", "go"),
        ("reader's", "reader"),
        ("stopped", "stop"),
        ("hoped", "hope"),
        ("carried", "carry"),
        ("agreed", "agree"),
        ("walking", "walk"),
        ("making", "make"),
        ("singing", "sing"),
        ("lying", "lie"),
        ("bigger", "big"),
        ("larger", "large"),
        ("happiest", "happy"),
        ("children", "child"),
        ("mice", "mouse"),
        ("went", "go"),
        ("written", "write"),
        ("better", "good"),
        ("Running", "run"),
    ];
    assert_eq!(wrong_lemmas(&cases, &["gif", "hop", "singe", "stopp"]), []);
}

#[test]
fn an_ending_common_in_base_forms_is_not_undone_when_the_query_is_a_headword() {
    let (_folder, library) = library_of(&["numb", "number", "numbest"]);

    assert_eq!(library.suggest("number", 10).unwrap().lemma, None);
    assert_eq!(
        library.suggest("numbers", 10).unwrap().lemma,
        lemma("numbers", "number")
    );
}
