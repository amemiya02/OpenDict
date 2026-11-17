//! The Library through the public interface of `opendict-core`: Library Folders the
//! reader adds and removes, the Dictionaries found in them, and what the Library keeps
//! about each Dictionary between starts.

use std::path::Path;

use opendict_core::Library;
use opendict_mdict_fixture::MdxBuilder;

fn dictionary(title: &str) -> MdxBuilder {
    MdxBuilder::new()
        .title(title)
        .entry("apple", &format!("<p>apple in {title} (sample)</p>"))
}

fn put(folder: &Path, relative: &str, dictionary: &MdxBuilder) {
    let path = folder.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    dictionary.write_to(path).unwrap();
}

fn titles(library: &Library) -> Vec<String> {
    library.dictionaries().map(|d| d.title.clone()).collect()
}

#[test]
fn a_new_library_is_empty() {
    let data = tempfile::tempdir().unwrap();

    let library = Library::load(&data.path().join("library.json")).unwrap();

    assert!(library.folders().is_empty());
    assert_eq!(library.dictionaries().count(), 0);
}

#[test]
fn every_dictionary_in_an_added_folder_and_its_subfolders_joins_the_library() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "top.mdx", &dictionary("Top"));
    put(folder.path(), "deep/er/nested.mdx", &dictionary("Nested"));

    let mut library = Library::load(&data.path().join("library.json")).unwrap();
    library.add_folder(folder.path()).unwrap();

    assert_eq!(titles(&library), ["Nested", "Top"]);
    assert_eq!(library.folders().len(), 1);
    assert_eq!(library.folders()[0].path, folder.path());
    assert_eq!(library.folders()[0].dictionary_count, 2);
}

#[test]
fn the_library_remembers_its_folders_between_starts() {
    let data = tempfile::tempdir().unwrap();
    let file = data.path().join("library.json");
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "one.mdx", &dictionary("One"));
    Library::load(&file)
        .unwrap()
        .add_folder(folder.path())
        .unwrap();

    let library = Library::load(&file).unwrap();

    assert_eq!(library.folders()[0].path, folder.path());
    assert_eq!(titles(&library), ["One"]);
}

#[test]
fn removing_a_folder_takes_its_dictionaries_out_of_the_library_and_leaves_the_files() {
    let data = tempfile::tempdir().unwrap();
    let file = data.path().join("library.json");
    let english = tempfile::tempdir().unwrap();
    let japanese = tempfile::tempdir().unwrap();
    put(english.path(), "en.mdx", &dictionary("English"));
    put(japanese.path(), "ja.mdx", &dictionary("Japanese"));
    let mut library = Library::load(&file).unwrap();
    library.add_folder(english.path()).unwrap();
    library.add_folder(japanese.path()).unwrap();

    library.remove_folder(english.path()).unwrap();

    assert_eq!(titles(&library), ["Japanese"]);
    assert_eq!(titles(&Library::load(&file).unwrap()), ["Japanese"]);
    assert!(english.path().join("en.mdx").is_file());
}

#[test]
fn a_rescan_finds_dictionaries_added_and_drops_dictionaries_removed_on_disk() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "old.mdx", &dictionary("Old"));
    let mut library = Library::load(&data.path().join("library.json")).unwrap();
    library.add_folder(folder.path()).unwrap();

    std::fs::remove_file(folder.path().join("old.mdx")).unwrap();
    put(folder.path(), "sub/new.mdx", &dictionary("New"));
    library.rescan().unwrap();

    assert_eq!(titles(&library), ["New"]);
}

#[test]
fn scanning_never_writes_to_a_library_folder() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "a/one.mdx", &dictionary("One"));
    std::fs::write(folder.path().join("a/one.mdd"), b"resources").unwrap();
    let before = snapshot(folder.path());

    let mut library = Library::load(&data.path().join("library.json")).unwrap();
    library.add_folder(folder.path()).unwrap();
    library.rescan().unwrap();
    let id = library.dictionaries().next().unwrap().id.clone();
    library.set_enabled(&id, false).unwrap();
    library.remove_folder(folder.path()).unwrap();

    assert_eq!(snapshot(folder.path()), before);
}

/// Every file under `folder` with its bytes and modification time.
fn snapshot(folder: &Path) -> Vec<(std::path::PathBuf, Vec<u8>, std::time::SystemTime)> {
    let mut files = Vec::new();
    let mut pending = vec![folder.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
                files.push((path.clone(), std::fs::read(&path).unwrap(), modified));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn overlapping_folders_list_each_dictionary_once() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "ja/one.mdx", &dictionary("One"));
    let mut library = Library::load(&data.path().join("library.json")).unwrap();

    library.add_folder(folder.path()).unwrap();
    library.add_folder(&folder.path().join("ja")).unwrap();

    assert_eq!(titles(&library), ["One"]);
    assert_eq!(library.folders()[1].dictionary_count, 1);
}

#[test]
fn a_folder_that_cannot_be_read_is_not_added() {
    let data = tempfile::tempdir().unwrap();
    let mut library = Library::load(&data.path().join("library.json")).unwrap();

    let result = library.add_folder(&data.path().join("no-such-folder"));

    assert!(result.is_err());
    assert!(library.folders().is_empty());
}

#[test]
fn a_library_folder_gone_missing_is_reported_and_the_others_still_answer() {
    let data = tempfile::tempdir().unwrap();
    let file = data.path().join("library.json");
    let kept = tempfile::tempdir().unwrap();
    let gone = tempfile::tempdir().unwrap();
    put(kept.path(), "kept.mdx", &dictionary("Kept"));
    put(gone.path(), "gone.mdx", &dictionary("Gone"));
    let mut library = Library::load(&file).unwrap();
    library.add_folder(gone.path()).unwrap();
    library.add_folder(kept.path()).unwrap();
    let gone_path = gone.path().to_owned();
    drop(gone);

    let library = Library::load(&file).unwrap();

    assert_eq!(library.folders()[0].path, gone_path);
    assert!(library.folders()[0].problem.is_some());
    assert_eq!(library.folders()[1].problem, None);
    assert_eq!(titles(&library), ["Kept"]);
}

#[test]
fn a_scan_runs_apart_from_the_library_and_is_put_in_place_after() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "old.mdx", &dictionary("Old"));
    let mut library = Library::load(&data.path().join("library.json")).unwrap();
    library.add_folder(folder.path()).unwrap();
    put(folder.path(), "new.mdx", &dictionary("New"));

    let scan = library.scan();
    let scanned = std::thread::spawn(move || scan.run()).join().unwrap();
    assert_eq!(
        titles(&library),
        ["Old"],
        "nothing changes until it is put in place"
    );

    assert!(library.finish_scan(scanned).unwrap());
    assert_eq!(titles(&library), ["Old", "New"]);
}

#[test]
fn a_scan_made_before_the_library_changed_is_not_put_in_place() {
    let data = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    put(first.path(), "first.mdx", &dictionary("First"));
    put(second.path(), "second.mdx", &dictionary("Second"));
    let mut library = Library::load(&data.path().join("library.json")).unwrap();
    library.add_folder(first.path()).unwrap();

    let stale = library.scan().run();
    library.add_folder(second.path()).unwrap();

    assert!(!library.finish_scan(stale).unwrap());
    assert_eq!(titles(&library), ["First", "Second"]);
    assert_eq!(library.folders().len(), 2);
}
