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
