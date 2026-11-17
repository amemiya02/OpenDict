//! The public interface of `opendict-core`: a Library read from one Library Folder.

use std::path::Path;

use opendict_core::Library;
use opendict_mdict_fixture::MdxBuilder;

fn sample_dictionary() -> MdxBuilder {
    MdxBuilder::new()
        .title("Sample English Dictionary")
        .entry("apple", "<p>a round fruit (sample)</p>")
        .entry("serendipity", "<p>a happy accident (sample)</p>")
}

fn put(folder: &Path, relative: &str, dictionary: &MdxBuilder) {
    let path = folder.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    dictionary.write_to(path).unwrap();
}

#[test]
fn returns_the_entry_for_an_exact_headword() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "english/sample.mdx", &sample_dictionary());

    let library = Library::open(folder.path()).unwrap();
    let entries = library.look_up("serendipity").unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].dictionary_title, "Sample English Dictionary");
    assert_eq!(entries[0].html, "<p>a happy accident (sample)</p>");
}

#[test]
fn finds_nothing_for_a_headword_no_dictionary_has() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample_dictionary());

    let library = Library::open(folder.path()).unwrap();

    assert_eq!(library.look_up("pear").unwrap(), []);
}

#[test]
fn every_dictionary_with_the_headword_answers_in_library_order() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "b-second.mdx", &MdxBuilder::new().title("Second").entry("apple", "<p>second</p>"));
    put(folder.path(), "a-first.mdx", &MdxBuilder::new().title("First").entry("apple", "<p>first</p>"));
    put(folder.path(), "c-other.mdx", &MdxBuilder::new().title("Other").entry("pear", "<p>other</p>"));

    let library = Library::open(folder.path()).unwrap();
    let entries = library.look_up("apple").unwrap();

    let answered: Vec<_> = entries.iter().map(|e| (e.dictionary_title.as_str(), e.html.as_str())).collect();
    assert_eq!(answered, [("First", "<p>first</p>"), ("Second", "<p>second</p>")]);
}

#[test]
fn follows_redirects_and_survives_a_redirect_cycle() {
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "sample.mdx",
        &MdxBuilder::new()
            .entry("colour", "<p>the property of reflecting light</p>")
            .link("color", "colour")
            .link("colr", "color")
            .link("ping", "pong")
            .link("pong", "ping"),
    );

    let library = Library::open(folder.path()).unwrap();

    assert_eq!(library.look_up("colr").unwrap()[0].html, "<p>the property of reflecting light</p>");
    assert_eq!(library.look_up("ping").unwrap(), []);
}

#[test]
fn a_dictionary_that_fails_to_open_does_not_stop_the_others() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "good.mdx", &sample_dictionary());
    std::fs::write(folder.path().join("broken.mdx"), b"not a dictionary").unwrap();

    let library = Library::open(folder.path()).unwrap();

    assert_eq!(library.dictionaries().count(), 1);
    assert_eq!(library.failures().len(), 1);
    assert!(library.failures()[0].path.ends_with("broken.mdx"));
    assert_eq!(library.look_up("apple").unwrap().len(), 1);
}

#[test]
fn names_an_untitled_dictionary_after_its_file() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "pocket.mdx", &MdxBuilder::new().title("").entry("apple", "<p>a fruit</p>"));

    let library = Library::open(folder.path()).unwrap();

    assert_eq!(library.look_up("apple").unwrap()[0].dictionary_title, "pocket");
}
