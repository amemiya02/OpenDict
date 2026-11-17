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
    assert_eq!(headwords(&library.suggest("dvd", 1).unwrap().items), ["ＤＶＤ"]);
    assert_eq!(headwords(&library.suggest("ｶﾞﾗｽ", 1).unwrap().items), ["ガラス"]);
    // The voicing mark is part of the word, not a diacritic.
    assert!(library.suggest("からす", 1).unwrap().miss);
}

#[test]
fn traditional_and_simplified_characters_do_not_matter() {
    let (_folder, library) = library_of(&["学习", "東京"]);

    assert_eq!(headwords(&library.suggest("學習", 1).unwrap().items), ["学习"]);
    assert_eq!(headwords(&library.suggest("东京", 1).unwrap().items), ["東京"]);
    assert!(!library.suggest("學", 1).unwrap().miss);
}
