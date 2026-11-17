//! What the Library knows about each Dictionary: the files that make it up, its key,
//! its Entry count and size, and whether it opened.

use std::path::{Path, PathBuf};

use opendict_core::{Library, OpenError};
use opendict_mdict_fixture::MdxBuilder;

fn dictionary(title: &str) -> MdxBuilder {
    MdxBuilder::new()
        .title(title)
        .entry("apple", &format!("<p>apple in {title} (sample)</p>"))
}

fn put(folder: &Path, relative: &str, dictionary: &MdxBuilder) -> PathBuf {
    let path = folder.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    dictionary.write_to(&path).unwrap();
    path
}

fn touch(folder: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
    let path = folder.join(relative);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn library_of(folder: &Path, data: &Path) -> Library {
    let mut library = Library::load(&data.join("library.json")).unwrap();
    library.add_folder(folder).unwrap();
    library
}

#[test]
fn a_dictionary_is_its_mdx_with_its_mdd_files_and_the_styles_and_scripts_beside_it() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mdx = put(folder.path(), "oxford.mdx", &dictionary("Oxford"));
    let mdd = touch(folder.path(), "oxford.mdd", b"0");
    let mdd1 = touch(folder.path(), "oxford.1.mdd", b"1");
    let mdd2 = touch(folder.path(), "Oxford.2.MDD", b"2");
    let mdd10 = touch(folder.path(), "oxford.10.mdd", b"10");
    let css = touch(folder.path(), "oxford.css", b"p{}");
    let js = touch(folder.path(), "jquery.js", b"0;");
    touch(folder.path(), "oxfordx.mdd", b"another dictionary's");
    touch(folder.path(), "readme.txt", b"not part of it");

    let library = library_of(folder.path(), data.path());
    let oxford = library.dictionaries().next().unwrap();

    assert_eq!(oxford.path, mdx);
    assert_eq!(oxford.resources, [mdd, mdd1, mdd2, mdd10]);
    let mut companions = oxford.companions.clone();
    companions.sort();
    assert_eq!(companions, [js, css]);
}

#[test]
fn shows_the_entry_count_and_the_size_of_all_its_files() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mdx = put(
        folder.path(),
        "pocket.mdx",
        &MdxBuilder::new()
            .title("Pocket")
            .entry("apple", "<p>a fruit</p>")
            .entry("pear", "<p>another fruit</p>")
            .link("apples", "apple"),
    );
    touch(folder.path(), "pocket.mdd", &[0; 1000]);
    touch(folder.path(), "pocket.css", &[0; 24]);

    let library = library_of(folder.path(), data.path());
    let pocket = library.dictionaries().next().unwrap();

    assert_eq!(pocket.entry_count, 3);
    assert_eq!(
        pocket.size,
        std::fs::metadata(mdx).unwrap().len() + 1000 + 24
    );
}

#[test]
fn a_dictionary_keeps_its_key_across_rescans_and_restarts() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "one.mdx", &dictionary("One"));
    let mut library = library_of(folder.path(), data.path());
    let key = library.dictionaries().next().unwrap().id.clone();

    library.rescan().unwrap();
    let after_rescan = library.dictionaries().next().unwrap().id.clone();
    let reloaded = Library::load(&data.path().join("library.json")).unwrap();
    let after_restart = reloaded.dictionaries().next().unwrap().id.clone();

    assert_eq!(after_rescan, key);
    assert_eq!(after_restart, key);
}

#[test]
fn dictionaries_with_the_same_content_in_different_places_have_different_keys() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "a/same.mdx", &dictionary("Same"));
    put(folder.path(), "b/same.mdx", &dictionary("Same"));

    let library = library_of(folder.path(), data.path());
    let keys: Vec<_> = library.dictionaries().map(|d| d.id.clone()).collect();

    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn a_different_dictionary_put_at_the_same_path_is_a_new_dictionary() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "current.mdx", &dictionary("First edition"));
    let mut library = library_of(folder.path(), data.path());
    let first = library.dictionaries().next().unwrap().id.clone();
    library.set_enabled(&first, false).unwrap();
    drop(library);

    put(folder.path(), "current.mdx", &dictionary("Second edition"));
    let library = Library::load(&data.path().join("library.json")).unwrap();
    let second = library.dictionaries().next().unwrap();

    assert_ne!(second.id, first);
    assert!(
        second.enabled,
        "the new Dictionary does not inherit the old one's state"
    );
}

#[test]
fn a_dictionary_that_fails_to_open_is_listed_with_a_plain_reason() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "good.mdx", &dictionary("Good"));
    touch(
        folder.path(),
        "notes.mdx",
        b"these are my notes, not a dictionary",
    );
    let mut damaged = dictionary("Damaged").build();
    damaged.truncate(damaged.len() - 8);
    touch(folder.path(), "damaged.mdx", &damaged);

    let library = library_of(folder.path(), data.path());
    let mut failures: Vec<_> = library
        .failures()
        .iter()
        .map(|failure| {
            (
                failure
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                failure.reason.clone(),
            )
        })
        .collect();
    failures.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(
        failures,
        [
            ("damaged.mdx".to_owned(), OpenError::Damaged),
            ("notes.mdx".to_owned(), OpenError::NotADictionary),
        ]
    );
    assert_eq!(library.folders()[0].dictionary_count, 3);
    assert_eq!(library.dictionaries().count(), 1);
}
