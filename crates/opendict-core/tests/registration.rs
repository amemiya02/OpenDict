//! Encrypted Dictionaries: an encrypted key index (`Encrypted=2`) needs nothing from the
//! reader; a registration code (`Encrypted=1`) is entered once, checked, and kept.

use std::path::{Path, PathBuf};

use opendict_core::{Error, Library, OpenError, Registration};
use opendict_mdict_fixture::{Identity, MdxBuilder};

const EMAIL: &str = "reader@example.com";
const CODE: &str = "0123456789ABCDEF0123456789ABCDEF";
const OTHER_CODE: &str = "FEDCBA9876543210FEDCBA9876543210";

fn bought() -> MdxBuilder {
    MdxBuilder::new()
        .title("Bought")
        .entry("apple", "<p>apple in Bought (sample)</p>")
        .registered_to(EMAIL, CODE, Identity::Utf16)
}

fn put(folder: &Path, name: &str, dictionary: &MdxBuilder) -> PathBuf {
    let path = folder.join(name);
    dictionary.write_to(&path).unwrap();
    path
}

fn registration(email: &str, code: &str) -> Registration {
    Registration {
        email: email.to_owned(),
        code: code.to_owned(),
    }
}

fn load(data: &Path) -> Library {
    Library::load(&data.join("library.json")).unwrap()
}

fn titles(library: &Library) -> Vec<String> {
    library.dictionaries().map(|d| d.title.clone()).collect()
}

fn failure_of<'a>(library: &'a Library, path: &Path) -> Option<&'a OpenError> {
    library
        .failures()
        .iter()
        .find(|failure| failure.path == path)
        .map(|failure| &failure.reason)
}

#[test]
fn a_dictionary_with_an_encrypted_key_index_opens_with_no_input() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(
        folder.path(),
        "indexed.mdx",
        &MdxBuilder::new()
            .title("Indexed")
            .entry("apple", "<p>apple in Indexed (sample)</p>")
            .encrypt_key_index(),
    );

    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    assert_eq!(titles(&library), ["Indexed"]);
    assert_eq!(library.look_up("apple").unwrap().len(), 1);
}

#[test]
fn a_dictionary_that_needs_a_code_opens_once_the_code_and_email_are_entered() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "bought.mdx", &bought());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();
    assert_eq!(
        failure_of(&library, &path),
        Some(&OpenError::NeedsRegistrationCode)
    );

    library.register(&path, registration(EMAIL, CODE)).unwrap();

    assert_eq!(titles(&library), ["Bought"]);
    assert_eq!(failure_of(&library, &path), None);
    assert_eq!(
        library.look_up("apple").unwrap()[0].html,
        "<p>apple in Bought (sample)</p>"
    );
}

#[test]
fn the_registration_is_kept_so_the_dictionary_opens_on_the_next_start() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "bought.mdx", &bought());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();
    library.register(&path, registration(EMAIL, CODE)).unwrap();
    drop(library);

    let library = load(data.path());

    assert_eq!(titles(&library), ["Bought"]);
    // Kept apart from the Library record, which says nothing about the reader.
    let record = std::fs::read_to_string(data.path().join("library.json")).unwrap();
    assert!(!record.contains(EMAIL) && !record.contains(CODE));
}

#[test]
fn a_wrong_code_or_email_is_refused_and_not_kept() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "bought.mdx", &bought());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    for (email, code) in [
        (EMAIL, OTHER_CODE),
        ("someone@example.com", CODE),
        (EMAIL, "not a code"),
    ] {
        let result = library.register(&path, registration(email, code));
        assert!(
            matches!(&result, Err(Error::WrongRegistration(p)) if p == &path),
            "{email} {code}: {result:?}"
        );
    }

    assert_eq!(
        failure_of(&library, &path),
        Some(&OpenError::NeedsRegistrationCode)
    );
    drop(library);
    let library = load(data.path());
    assert_eq!(
        failure_of(&library, &path),
        Some(&OpenError::NeedsRegistrationCode)
    );
}

#[test]
fn a_kept_registration_that_stops_working_is_reported_as_wrong() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "bought.mdx", &bought());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();
    library.register(&path, registration(EMAIL, CODE)).unwrap();
    drop(library);
    // The file is replaced by an edition registered to someone else.
    put(
        folder.path(),
        "bought.mdx",
        &bought().registered_to("someone@example.com", CODE, Identity::Utf16),
    );

    let library = load(data.path());

    assert_eq!(
        failure_of(&library, &path),
        Some(&OpenError::WrongRegistrationCode)
    );
}

#[test]
fn the_email_alone_is_enough_when_the_code_is_in_a_key_file_beside_the_dictionary() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "bought.mdx", &bought());
    std::fs::write(folder.path().join("bought.key"), format!("{CODE}\r\n")).unwrap();
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    library.register(&path, registration(EMAIL, "")).unwrap();

    assert_eq!(titles(&library), ["Bought"]);
}

#[test]
fn without_a_code_anywhere_the_email_alone_is_not_enough() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "bought.mdx", &bought());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    let result = library.register(&path, registration(EMAIL, ""));

    assert!(
        matches!(&result, Err(Error::RegistrationCodeNeeded(p)) if p == &path),
        "{result:?}"
    );
}

#[test]
fn only_a_dictionary_waiting_for_a_code_can_be_registered() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let free = put(
        folder.path(),
        "free.mdx",
        &MdxBuilder::new().entry("apple", "<p>apple (sample)</p>"),
    );
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = put(elsewhere.path(), "outside.mdx", &bought());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    for path in [free, outside] {
        let result = library.register(&path, registration(EMAIL, CODE));
        assert!(
            matches!(result, Err(Error::UnknownDictionary(_))),
            "{path:?}: {result:?}"
        );
    }
}

#[test]
fn a_registration_never_shows_in_debug_output() {
    let shown = format!("{:?}", registration(EMAIL, CODE));
    assert!(!shown.contains(EMAIL) && !shown.contains(CODE), "{shown}");
}

#[test]
fn the_resources_of_a_registered_dictionary_open_with_the_same_registration() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "bought.mdx", &bought());
    opendict_mdict_fixture::MddBuilder::new()
        .resource(r"\img\a.png", b"\x89PNG sample picture")
        .registered_to(EMAIL, CODE, Identity::Utf16)
        .write_to(folder.path().join("bought.mdd"))
        .unwrap();
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();
    library.register(&path, registration(EMAIL, CODE)).unwrap();

    let id = library.dictionaries().next().unwrap().id.clone();
    assert_eq!(
        library.resource(&id, "img/a.png").unwrap().as_deref(),
        Some(&b"\x89PNG sample picture"[..])
    );
}

#[test]
fn a_damaged_registrations_file_is_reported_without_quoting_it() {
    let data = tempfile::tempdir().unwrap();
    std::fs::write(
        data.path().join("registrations.json"),
        format!(r#"{{"bought.mdx": "{EMAIL}""#),
    )
    .unwrap();

    let error = Library::load(&data.path().join("library.json"))
        .err()
        .expect("a damaged file is reported")
        .to_string();

    assert!(error.contains("registrations.json"), "{error}");
    assert!(!error.contains(EMAIL), "{error}");
}
