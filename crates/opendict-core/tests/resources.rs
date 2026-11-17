//! Entry resources: the files a Dictionary's Entries refer to, resolved through the Library.

use std::path::Path;

use opendict_core::Library;
use opendict_mdict_fixture::{MddBuilder, MdxBuilder};

const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\n\0sample picture";
const SOUND: &[u8] = b"OggS\0sample sound";

fn dictionary() -> MdxBuilder {
    MdxBuilder::new().title("Illustrated").entry(
        "cat",
        r#"<link rel="stylesheet" href="illustrated.css"><img src="img/cat.png">"#,
    )
}

fn write(folder: &Path, relative: &str, bytes: &[u8]) {
    let path = folder.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn mdd(resources: &[(&str, &[u8])]) -> Vec<u8> {
    resources
        .iter()
        .fold(MddBuilder::new(), |b, (path, bytes)| {
            b.resource(path, bytes)
        })
        .build()
}

#[test]
fn resolves_resources_from_the_dictionarys_mdd_files() {
    let folder = tempfile::tempdir().unwrap();
    write(folder.path(), "pets/illustrated.mdx", &dictionary().build());
    write(
        folder.path(),
        "pets/illustrated.mdd",
        &mdd(&[("\\img\\cat.png", PICTURE)]),
    );
    write(
        folder.path(),
        "pets/illustrated.1.mdd",
        &mdd(&[("\\cat.ogg", SOUND)]),
    );

    let library = Library::open(folder.path()).unwrap();
    let id = &library.dictionaries().next().unwrap().id;

    assert_eq!(
        library.resource(id, "img/cat.png").unwrap().as_deref(),
        Some(PICTURE)
    );
    assert_eq!(
        library.resource(id, "cat.ogg").unwrap().as_deref(),
        Some(SOUND)
    );
    assert_eq!(library.resource(id, "img/dog.png").unwrap(), None);
}

#[test]
fn resolves_stylesheets_and_scripts_beside_the_mdx() {
    let folder = tempfile::tempdir().unwrap();
    write(folder.path(), "illustrated.mdx", &dictionary().build());
    write(folder.path(), "illustrated.css", b"img { width: 4em }");
    write(folder.path(), "js/fold.js", b"// sample script");

    let library = Library::open(folder.path()).unwrap();
    let id = &library.dictionaries().next().unwrap().id;

    assert_eq!(
        library.resource(id, "illustrated.css").unwrap().as_deref(),
        Some(&b"img { width: 4em }"[..])
    );
    assert_eq!(
        library.resource(id, "js/fold.js").unwrap().as_deref(),
        Some(&b"// sample script"[..])
    );
}

#[test]
fn a_file_beside_the_mdx_wins_over_the_same_path_in_the_mdd() {
    let folder = tempfile::tempdir().unwrap();
    write(folder.path(), "illustrated.mdx", &dictionary().build());
    write(
        folder.path(),
        "illustrated.mdd",
        &mdd(&[("\\illustrated.css", b"from the mdd")]),
    );
    write(folder.path(), "illustrated.css", b"beside the mdx");

    let library = Library::open(folder.path()).unwrap();
    let id = &library.dictionaries().next().unwrap().id;

    assert_eq!(
        library.resource(id, "illustrated.css").unwrap().as_deref(),
        Some(&b"beside the mdx"[..])
    );
}

#[test]
fn never_serves_files_outside_the_dictionary_or_other_dictionaries() {
    let folder = tempfile::tempdir().unwrap();
    write(
        folder.path(),
        "secret.css",
        b"outside the dictionary's folder",
    );
    write(
        folder.path(),
        "dicts/illustrated.mdx",
        &dictionary().build(),
    );
    write(
        folder.path(),
        "dicts/other.mdx",
        &MdxBuilder::new().entry("dog", "<p>dog</p>").build(),
    );
    write(
        folder.path(),
        "dicts/other.mdd",
        &mdd(&[("\\dog.png", PICTURE)]),
    );

    let library = Library::open(folder.path()).unwrap();
    let id = library
        .dictionaries()
        .find(|d| d.title == "Illustrated")
        .unwrap()
        .id
        .clone();

    for path in [
        "../secret.css",
        "..\\secret.css",
        "img/../../secret.css",
        "/secret.css",
        "other.mdx",
        "other.mdd",
        "illustrated.mdx",
        "dog.png",
    ] {
        assert_eq!(library.resource(&id, path).unwrap(), None, "{path}");
    }
    let absolute = folder.path().join("secret.css");
    assert_eq!(
        library.resource(&id, &absolute.to_string_lossy()).unwrap(),
        None
    );
}

/// The id of the Dictionary titled `title`.
fn id_of(library: &Library, title: &str) -> String {
    library
        .dictionaries()
        .find(|d| d.title == title)
        .unwrap()
        .id
        .clone()
}

#[test]
fn dictionaries_sharing_a_folder_get_only_the_loose_files_named_after_them() {
    let folder = tempfile::tempdir().unwrap();
    write(folder.path(), "illustrated.mdx", &dictionary().build());
    write(
        folder.path(),
        "other.mdx",
        &MdxBuilder::new()
            .title("Other")
            .entry("dog", "<p>dog</p>")
            .build(),
    );
    write(folder.path(), "illustrated.css", b"illustrated's style");
    write(
        folder.path(),
        "Illustrated.Night.css",
        b"illustrated's night",
    );
    write(
        folder.path(),
        "illustrated/fold.js",
        b"illustrated's script",
    );
    write(folder.path(), "other.css", b"other's style");
    write(folder.path(), "other/fold.js", b"other's script");
    write(folder.path(), "jquery.js", b"whose is it?");
    write(folder.path(), "img/cat.png", PICTURE);

    let library = Library::open(folder.path()).unwrap();
    let illustrated = id_of(&library, "Illustrated");
    let other = id_of(&library, "Other");

    let served = |id: &str, path: &str| library.resource(id, path).unwrap();
    assert_eq!(
        served(&illustrated, "illustrated.css").as_deref(),
        Some(&b"illustrated's style"[..])
    );
    assert_eq!(
        served(&illustrated, "Illustrated.Night.css").as_deref(),
        Some(&b"illustrated's night"[..])
    );
    assert_eq!(
        served(&illustrated, "illustrated/fold.js").as_deref(),
        Some(&b"illustrated's script"[..])
    );
    assert_eq!(
        served(&other, "other.css").as_deref(),
        Some(&b"other's style"[..])
    );
    for path in [
        "other.css",
        "other/fold.js",
        "jquery.js",
        "img/cat.png",
        "illustratedx.css",
    ] {
        assert_eq!(served(&illustrated, path), None, "{path}");
    }
    for path in ["illustrated.css", "illustrated/fold.js", "jquery.js"] {
        assert_eq!(served(&other, path), None, "{path}");
    }
}

#[test]
fn a_dictionary_alone_in_its_folder_gets_its_subfolders_but_not_one_holding_another_dictionary() {
    let folder = tempfile::tempdir().unwrap();
    write(folder.path(), "illustrated.mdx", &dictionary().build());
    write(folder.path(), "img/cat.png", PICTURE);
    write(folder.path(), "jquery.js", b"0;");
    write(
        folder.path(),
        "nested/deeper/other.mdx",
        &MdxBuilder::new()
            .title("Other")
            .entry("dog", "<p>dog</p>")
            .build(),
    );
    write(folder.path(), "nested/deeper/other.css", b"other's style");
    write(folder.path(), "nested/deeper/img/dog.png", PICTURE);

    let library = Library::open(folder.path()).unwrap();
    let illustrated = id_of(&library, "Illustrated");
    let other = id_of(&library, "Other");

    assert_eq!(
        library
            .resource(&illustrated, "img/cat.png")
            .unwrap()
            .as_deref(),
        Some(PICTURE)
    );
    assert_eq!(
        library
            .resource(&illustrated, "jquery.js")
            .unwrap()
            .as_deref(),
        Some(&b"0;"[..])
    );
    for path in ["nested/deeper/other.css", "nested/deeper/img/dog.png"] {
        assert_eq!(
            library.resource(&illustrated, path).unwrap(),
            None,
            "{path}"
        );
    }
    assert_eq!(
        library.resource(&other, "img/dog.png").unwrap().as_deref(),
        Some(PICTURE)
    );
}

#[test]
fn an_unknown_dictionary_has_no_resources() {
    let folder = tempfile::tempdir().unwrap();
    write(folder.path(), "illustrated.mdx", &dictionary().build());
    write(folder.path(), "illustrated.css", b"img { width: 4em }");

    let library = Library::open(folder.path()).unwrap();

    assert_eq!(
        library
            .resource("no-such-dictionary", "illustrated.css")
            .unwrap(),
        None
    );
}

#[test]
fn returns_one_dictionarys_entry_by_its_id() {
    let folder = tempfile::tempdir().unwrap();
    write(
        folder.path(),
        "a.mdx",
        &MdxBuilder::new()
            .title("A")
            .entry("cat", "<p>a</p>")
            .build(),
    );
    write(
        folder.path(),
        "b.mdx",
        &MdxBuilder::new()
            .title("B")
            .entry("cat", "<p>b</p>")
            .build(),
    );

    let library = Library::open(folder.path()).unwrap();
    let b = library
        .dictionaries()
        .find(|d| d.title == "B")
        .unwrap()
        .id
        .clone();

    let entry = library.entry(&b, "cat").unwrap().unwrap();
    assert_eq!(entry.dictionary_title, "B");
    assert_eq!(entry.html, "<p>b</p>");
    assert_eq!(library.entry(&b, "dog").unwrap(), None);
    assert_eq!(library.entry("no-such-dictionary", "cat").unwrap(), None);
}
