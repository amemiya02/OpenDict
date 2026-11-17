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
    put(
        folder.path(),
        "sample.mdx",
        &dictionary("Sample", headwords),
    );
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
    assert_eq!(
        headwords(&suggestions.items),
        ["serendipity", "serene", "shadow"]
    );
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
            let found = library
                .suggest(form, 5)
                .unwrap()
                .lemma
                .map(|lemma| lemma.to);
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

#[test]
fn a_japanese_conjugated_form_finds_its_dictionary_form() {
    let (_folder, library) = library_of(&["食べる", "食べ物", "儚い", "儚さ"]);

    let ate = library.suggest("食べた", 10).unwrap();
    let fleetingly = library.suggest("儚く", 10).unwrap();

    assert_eq!(ate.lemma, lemma("食べた", "食べる"));
    assert_eq!(headwords(&ate.items)[0], "食べる");
    assert_eq!(fleetingly.lemma, lemma("儚く", "儚い"));
    assert_eq!(headwords(&fleetingly.items)[0], "儚い");
}

#[test]
fn japanese_rules_cover_verb_classes_adjectives_and_chained_endings() {
    let cases = [
        ("食べます", "食べる"),
        ("食べない", "食べる"),
        ("食べなかった", "食べる"),
        ("食べて", "食べる"),
        ("食べている", "食べる"),
        ("食べていました", "食べる"),
        ("食べられた", "食べる"),
        ("食べさせられなかった", "食べる"),
        ("たべた", "たべる"),
        ("書いた", "書く"),
        ("書かない", "書く"),
        ("書きたい", "書く"),
        ("書けば", "書く"),
        ("書こう", "書く"),
        ("泳いだ", "泳ぐ"),
        ("話して", "話す"),
        ("待った", "待つ"),
        ("死んだ", "死ぬ"),
        ("遊んで", "遊ぶ"),
        ("読みました", "読む"),
        ("帰らなかった", "帰る"),
        ("買って", "買う"),
        ("行った", "行く"),
        ("勉強した", "勉強する"),
        ("しない", "する"),
        ("来た", "来る"),
        ("こない", "くる"),
        ("儚かった", "儚い"),
        ("儚くない", "儚い"),
        ("儚くなかった", "儚い"),
        ("儚ければ", "儚い"),
        ("儚くて", "儚い"),
    ];
    assert_eq!(wrong_lemmas(&cases, &["書い", "食べ物"]), []);
}

#[test]
fn a_noun_that_took_suru_finds_the_noun() {
    let (_folder, library) = library_of(&["散歩", "散策"]);

    assert_eq!(
        library.suggest("散歩しました", 10).unwrap().lemma,
        lemma("散歩しました", "散歩")
    );
}
