//! The public interface of `opendict-core`: a Lookup that forgives how the word was typed.

use std::path::Path;

use opendict_core::{Library, Suggestion};
use opendict_mdict_fixture::MdxBuilder;

fn library_of(headwords: &[&str]) -> (tempfile::TempDir, Library) {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", "Sample", headwords);
    let library = Library::open(folder.path()).unwrap();
    (folder, library)
}

fn put(folder: &Path, relative: &str, title: &str, headwords: &[&str]) {
    headwords
        .iter()
        .fold(MdxBuilder::new().title(title), |builder, headword| {
            builder.entry(headword, &format!("<p>{headword}</p>"))
        })
        .write_to(folder.join(relative))
        .unwrap();
}

fn headwords(items: &[Suggestion]) -> Vec<&str> {
    items.iter().map(|item| item.headword.as_str()).collect()
}

#[test]
fn case_and_diacritics_do_not_matter() {
    let (_folder, library) = library_of(&["Café", "cable", "naïve"]);

    let cafe = library.suggest("cafe", 10).unwrap();
    let naive = library.suggest("NAIVE", 10).unwrap();

    assert_eq!(headwords(&cafe.items), ["Café", "naïve"]);
    assert!(!cafe.miss && !cafe.near);
    assert_eq!(headwords(&naive.items), ["naïve"]);
    assert!(!naive.miss);
}

#[test]
fn full_width_half_width_katakana_and_hiragana_do_not_matter() {
    let (_folder, library) = library_of(&["カフェ", "ＤＶＤ", "ガラス"]);

    for query in ["かふぇ", "ｶﾌｪ", "カフェ"] {
        let suggestions = library.suggest(query, 1).unwrap();
        assert_eq!(headwords(&suggestions.items), ["カフェ"], "query {query}");
        assert!(!suggestions.miss, "query {query}");
    }
    assert_eq!(
        headwords(&library.suggest("dvd", 1).unwrap().items),
        ["ＤＶＤ"]
    );
    assert_eq!(
        headwords(&library.suggest("ｶﾞﾗｽ", 1).unwrap().items),
        ["ガラス"]
    );
    // The voicing mark is part of the word, not a diacritic: からす only spells ガラス nearly.
    assert!(library.suggest("からす", 1).unwrap().near);
}

#[test]
fn traditional_and_simplified_characters_do_not_matter() {
    let (_folder, library) = library_of(&["学习", "東京"]);

    assert_eq!(
        headwords(&library.suggest("學習", 1).unwrap().items),
        ["学习"]
    );
    assert_eq!(
        headwords(&library.suggest("东京", 1).unwrap().items),
        ["東京"]
    );
    assert!(!library.suggest("學", 1).unwrap().miss);
}

#[test]
fn star_stands_for_any_run_of_letters_and_question_mark_for_one() {
    let (_folder, library) = library_of(&[
        "glasses", "sea", "seas", "series", "serious", "serves", "settle", "sew",
    ]);

    let any_run = library.suggest("se*s", 10).unwrap();
    let one = library.suggest("se?", 10).unwrap();

    // Only the matches: a pattern's column does not carry on through the index.
    assert_eq!(
        headwords(&any_run.items),
        ["seas", "series", "serious", "serves"]
    );
    assert!(!any_run.miss && !any_run.near && !any_run.browse);
    assert_eq!(headwords(&one.items), ["sea", "sew"]);
    assert_eq!(
        headwords(&library.suggest("*SES", 10).unwrap().items),
        ["glasses"]
    );
}

#[test]
fn a_pattern_that_matches_nothing_is_a_miss_where_its_letters_would_stand() {
    let (_folder, library) = library_of(&["apple", "banana", "cherry"]);

    let between = library.suggest("bx*z", 10).unwrap();
    let beyond = library.suggest("z?", 10).unwrap();

    assert!(between.miss && !between.near);
    assert_eq!(headwords(&between.items), ["cherry"]);
    assert!(beyond.miss);
    assert_eq!(beyond.items, []);
}

#[test]
fn the_column_continues_with_what_the_query_offers() {
    let (_folder, library) = library_of(&["a1", "a2", "a3", "a4", "a5", "b1"]);

    let first = library.suggest("a*", 2).unwrap();

    assert_eq!(headwords(&first.items), ["a1", "a2"]);
    assert_eq!(
        headwords(&library.suggest_after("a*", "a2", 2).unwrap()),
        ["a3", "a4"]
    );
    assert_eq!(
        headwords(&library.suggest_after("a*", "a4", 10).unwrap()),
        ["a5"]
    );
    assert_eq!(library.suggest_after("a*", "a5", 10).unwrap(), []);
    // Without wildcards, or after a miss, the column is the index itself.
    assert_eq!(
        headwords(&library.suggest_after("a", "a5", 10).unwrap()),
        ["b1"]
    );
    assert_eq!(
        headwords(&library.suggest_after("ax*", "a5", 10).unwrap()),
        ["b1"]
    );
}

#[test]
fn a_typo_offers_close_spellings_nearest_first() {
    let (_folder, library) = library_of(&[
        "serendipity",
        "serene",
        "series",
        "serious",
        "the",
        "then",
        "zebra",
    ]);

    let typo = library.suggest("serendipty", 10).unwrap();
    let two_ways = library.suggest("serius", 10).unwrap();
    let swapped = library.suggest("teh", 10).unwrap();

    assert!(typo.near && !typo.miss);
    assert_eq!(headwords(&typo.items), ["serendipity"]);
    assert!(two_ways.near);
    assert_eq!(headwords(&two_ways.items), ["series", "serious"]);
    // Short words allow one slip: a swapped pair counts as one.
    assert_eq!(headwords(&swapped.items), ["the"]);
}

#[test]
fn a_pattern_is_not_lemmatized() {
    // Without the pattern, `a*s` would lemmatize to the filed headword `a*`.
    let (_folder, library) = library_of(&["a*", "apples", "bus"]);

    let suggestions = library.suggest("a*s", 10).unwrap();

    assert_eq!(suggestions.lemma, None);
    assert_eq!(headwords(&suggestions.items), ["apples"]);
}

#[test]
fn a_base_form_comes_before_close_spellings() {
    let (_folder, library) = library_of(&["apple", "apply"]);

    let suggestions = library.suggest("applies", 10).unwrap();

    assert_eq!(
        suggestions.lemma.map(|lemma| lemma.to).as_deref(),
        Some("apply")
    );
    assert!(!suggestions.near && !suggestions.miss);
    assert_eq!(headwords(&suggestions.items)[0], "apply");
}
