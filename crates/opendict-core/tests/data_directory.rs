//! The data directory through the public interface of `opendict-core`: every file the
//! reader's data is kept in lives in one directory, which can be moved as a whole; the app
//! finds it in its new place from then on, and a move that fails or is interrupted never
//! loses or half-moves anything.

use std::ops::ControlFlow;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use opendict_core::{DataDirectory, History, Library, Preferences, SavedFrom, WordBooks};
use opendict_mdict_fixture::MdxBuilder;

fn sample() -> MdxBuilder {
    MdxBuilder::new()
        .title("Sample")
        .entry("apple", "<p>a round fruit (sample)</p>")
        .entry("bank", "<p>a river side (sample)</p>")
}

/// Fills `data` the way the app does: a Library with one indexed Dictionary from
/// `dictionaries`, History, a Word Book and a per-Dictionary preference.
fn fill(data: &DataDirectory, dictionaries: &Path) {
    sample().write_to(&dictionaries.join("sample.mdx")).unwrap();
    let mut library = Library::load(&data.library_file()).unwrap();
    library.add_folder(dictionaries).unwrap();
    library.build_indexes();
    let id = library.dictionaries().next().unwrap().id.clone();
    drop(library);

    let mut history = History::load(&data.history_file()).unwrap();
    history.record("apple").unwrap();
    history.record("bank").unwrap();

    let mut books = WordBooks::load(&data.word_books_file()).unwrap();
    let book = books.create("Reading").unwrap();
    let from = SavedFrom {
        dictionary_id: id.clone(),
        dictionary_title: "Sample".to_owned(),
    };
    books
        .toggle(book, "apple", from, UNIX_EPOCH + Duration::from_secs(1_700_000_000))
        .unwrap();

    let mut preferences = Preferences::load(&data.preferences_file()).unwrap();
    preferences.set_allows_network(&id, true).unwrap();
}

/// The names the reader's data is kept under.
const DATA: [&str; 5] = [
    "history.json",
    "indexes",
    "library.json",
    "preferences.json",
    "word-books.json",
];

fn assert_no_data_in(folder: &Path) {
    let left: Vec<String> = names(folder)
        .into_iter()
        .filter(|name| DATA.contains(&name.as_str()))
        .collect();
    assert!(left.is_empty(), "data left in {}: {left:?}", folder.display());
}

/// The names in `folder`, sorted.
fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_new_reader_keeps_all_their_data_in_one_directory_in_the_home_folder() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();

    let data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    assert_eq!(data.path(), home.path());
    assert!(data.is_default());
    assert_eq!(names(home.path()), DATA);
}

/// Moves `data` to `to` without watching progress.
fn relocate(data: &mut DataDirectory, to: &Path) -> opendict_core::Result<()> {
    data.relocate(to, &mut |_| ControlFlow::Continue(()))
        .map(|moved| assert!(moved.is_some(), "the move was not cancelled"))
}

/// What a reader would see of their data kept in `data`.
fn contents(data: &DataDirectory) -> (Vec<String>, Vec<String>, Vec<String>, bool) {
    let library = Library::load(&data.library_file()).unwrap();
    assert!(
        library.index_jobs().is_empty(),
        "the stored indexes are used, not rebuilt"
    );
    let html = library
        .look_up("apple")
        .unwrap()
        .into_iter()
        .map(|entry| entry.html)
        .collect();
    let id = library.dictionaries().next().unwrap().id.clone();
    let history = History::load(&data.history_file()).unwrap().headwords().to_vec();
    let books = WordBooks::load(&data.word_books_file()).unwrap();
    let words = books.books()[0]
        .words
        .iter()
        .map(|word| format!("{} in {}", word.headword, books.books()[0].name))
        .collect();
    let network = Preferences::load(&data.preferences_file())
        .unwrap()
        .allows_network(&id);
    (html, history, words, network)
}

fn expected() -> (Vec<String>, Vec<String>, Vec<String>, bool) {
    (
        vec!["<p>a round fruit (sample)</p>".to_owned()],
        vec!["bank".to_owned(), "apple".to_owned()],
        vec!["apple in Reading".to_owned()],
        true,
    )
}

#[test]
fn relocating_moves_every_file_and_the_app_finds_the_data_there_from_then_on() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let to = synced.path().join("OpenDict");
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    relocate(&mut data, &to).unwrap();

    assert_eq!(data.path(), to);
    assert!(!data.is_default());
    assert_eq!(names(&to), DATA);
    assert_eq!(contents(&data), expected());
    let after_restart = DataDirectory::locate(home.path()).unwrap();
    assert_eq!(after_restart.path(), to);
    assert_eq!(after_restart.library_file(), to.join("library.json"));
    assert_eq!(contents(&after_restart), expected());
    assert_no_data_in(home.path());
}

#[test]
fn a_folder_that_already_holds_opendict_data_is_not_overwritten() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());
    std::fs::write(other.path().join("history.json"), r#"{"headwords":["theirs"]}"#).unwrap();
    std::fs::write(other.path().join("notes.txt"), "not OpenDict's").unwrap();

    let error = relocate(&mut data, other.path()).unwrap_err();

    assert!(
        matches!(&error, opendict_core::Error::DataDirectoryHoldsData(path) if path == other.path()),
        "{error:?}"
    );
    assert_eq!(names(other.path()), ["history.json", "notes.txt"]);
    assert_eq!(
        std::fs::read_to_string(other.path().join("history.json")).unwrap(),
        r#"{"headwords":["theirs"]}"#
    );
    assert_eq!(data.path(), home.path());
    assert_eq!(DataDirectory::locate(home.path()).unwrap().path(), home.path());
    assert_eq!(contents(&data), expected());
}

#[test]
fn a_folder_with_other_files_in_it_takes_the_data_beside_them() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    std::fs::write(synced.path().join("notes.txt"), "not OpenDict's").unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    relocate(&mut data, synced.path()).unwrap();

    assert_eq!(contents(&data), expected());
    assert_eq!(
        std::fs::read_to_string(synced.path().join("notes.txt")).unwrap(),
        "not OpenDict's"
    );
}

#[test]
fn the_data_can_move_again_and_back_home() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    relocate(&mut data, first.path()).unwrap();
    relocate(&mut data, second.path()).unwrap();
    assert_no_data_in(first.path());
    assert_eq!(DataDirectory::locate(home.path()).unwrap().path(), second.path());

    relocate(&mut data, home.path()).unwrap();
    assert_no_data_in(second.path());
    let after_restart = DataDirectory::locate(home.path()).unwrap();
    assert!(after_restart.is_default());
    assert_eq!(names(home.path()), DATA, "only the reader's data is in the home folder");
    assert_eq!(contents(&after_restart), expected());
}

#[test]
fn moving_the_data_to_where_it_already_is_changes_nothing() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    relocate(&mut data, home.path()).unwrap();

    assert!(data.is_default());
    assert_eq!(contents(&data), expected());
}

#[test]
fn the_data_cannot_move_into_a_folder_inside_itself() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    let error = relocate(&mut data, &home.path().join("indexes").join("more")).unwrap_err();

    assert!(
        matches!(&error, opendict_core::Error::DataDirectory { .. }),
        "{error:?}"
    );
    assert_eq!(names(home.path()), DATA);
    assert_eq!(contents(&data), expected());
}

/// Keeps another program's hold on a file, so it cannot be read, until dropped.
struct Locked {
    #[cfg(windows)]
    _file: std::fs::File,
    #[cfg(unix)]
    path: std::path::PathBuf,
}

/// Locks `path` against reading, as another program can; `None` if this platform (or a
/// root user) cannot be kept from reading it.
fn lock(path: &Path) -> Option<Locked> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .unwrap();
        Some(Locked { _file: file })
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(path).is_ok() {
            return None;
        }
        Some(Locked {
            path: path.to_owned(),
        })
    }
}

#[cfg(unix)]
impl Drop for Locked {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o644));
    }
}

#[test]
fn a_move_that_fails_part_way_leaves_the_data_where_it_was_and_nothing_in_the_new_place() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let to = synced.path().join("OpenDict");
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());
    let Some(locked) = lock(&data.history_file()) else {
        return;
    };

    let error = relocate(&mut data, &to).unwrap_err();
    drop(locked);

    assert!(matches!(&error, opendict_core::Error::DataDirectory { .. }), "{error:?}");
    assert!(!to.exists(), "the half copy is cleared away");
    assert_eq!(data.path(), home.path());
    assert_eq!(DataDirectory::locate(home.path()).unwrap().path(), home.path());
    assert_eq!(names(home.path()), DATA);
    assert_eq!(contents(&data), expected());

    relocate(&mut data, &to).unwrap();
    assert_eq!(contents(&data), expected());
}

#[test]
fn a_cancelled_move_leaves_the_data_where_it_was() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());
    let mut heard = Vec::new();

    let moved = data
        .relocate(synced.path(), &mut |progress| {
            heard.push(progress);
            if heard.len() == 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .unwrap();

    assert!(moved.is_none());
    assert_eq!(heard.len(), 2);
    assert!(heard[0].done > 0 && heard[0].done < heard[1].done && heard[1].done < heard[1].total);
    assert!(synced.path().exists(), "a folder that was there stays");
    assert_eq!(names(synced.path()), Vec::<String>::new());
    assert_eq!(data.path(), home.path());
    assert_eq!(DataDirectory::locate(home.path()).unwrap().path(), home.path());
    assert_eq!(contents(&data), expected());
}

#[test]
fn a_move_cut_short_by_a_crash_leaves_the_data_where_it_was_and_can_be_tried_again() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());

    for crash_after in 1..=4 {
        let mut copied = 0;
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            data.relocate(synced.path(), &mut |_| {
                copied += 1;
                if copied == crash_after {
                    panic!("the power went off");
                }
                ControlFlow::Continue(())
            })
        }));
        assert!(crashed.is_err());

        // What the app finds when it starts again.
        let after_restart = DataDirectory::locate(home.path()).unwrap();
        assert_eq!(after_restart.path(), home.path());
        assert_eq!(names(home.path()), DATA);
        assert_eq!(contents(&after_restart), expected());
        data = after_restart;
    }

    relocate(&mut data, synced.path()).unwrap();
    assert_eq!(contents(&data), expected());
    assert_eq!(DataDirectory::locate(home.path()).unwrap().path(), synced.path());
    assert_eq!(names(synced.path()), DATA);
}

/// Keeps the files in `folder` from being removed (but not from being read), as another
/// program can, until dropped; `None` where that cannot be done.
fn hold_against_removal(folder: &Path) -> Option<Box<dyn std::any::Any>> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        let file = std::fs::read_dir(folder).unwrap().next().unwrap().unwrap().path();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(file)
            .unwrap();
        struct Held(#[allow(dead_code)] std::fs::File);
        impl Drop for Held {
            fn drop(&mut self) {}
        }
        Some(Box::new(Held(held)))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        struct Held(std::path::PathBuf);
        impl Drop for Held {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o555)).unwrap();
        let held = Held(folder.to_owned());
        let probe = folder.join("probe");
        if std::fs::write(&probe, "").is_ok() {
            let _ = std::fs::remove_file(probe);
            return None;
        }
        Some(Box::new(held))
    }
}

#[test]
fn data_that_cannot_be_removed_from_the_old_place_is_reported_and_does_not_block_a_move_back() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());
    let Some(held) = hold_against_removal(&home.path().join("indexes")) else {
        return;
    };

    let moved = data
        .relocate(synced.path(), &mut |_| ControlFlow::Continue(()))
        .unwrap()
        .unwrap();
    drop(held);

    assert_eq!(moved.left_behind, [home.path().join("indexes")]);
    assert_eq!(DataDirectory::locate(home.path()).unwrap().path(), synced.path());
    assert_eq!(contents(&data), expected());

    relocate(&mut data, home.path()).unwrap();
    assert!(data.is_default());
    assert_eq!(names(home.path()), DATA);
    assert_eq!(contents(&data), expected());
}

#[test]
fn a_second_computer_can_use_the_data_already_in_a_synced_folder() {
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let first_home = tempfile::tempdir().unwrap();
    let mut first = DataDirectory::locate(first_home.path()).unwrap();
    fill(&first, dictionaries.path());
    relocate(&mut first, synced.path()).unwrap();
    let second_home = tempfile::tempdir().unwrap();
    let second = DataDirectory::locate(second_home.path()).unwrap();
    History::load(&second.history_file())
        .unwrap()
        .record("own")
        .unwrap();

    let second = DataDirectory::use_folder(second_home.path(), synced.path()).unwrap();

    assert_eq!(second.path(), synced.path());
    assert_eq!(contents(&second), expected());
    assert_eq!(
        DataDirectory::locate(second_home.path()).unwrap().path(),
        synced.path()
    );
    assert_eq!(
        History::load(&second_home.path().join("history.json"))
            .unwrap()
            .headwords(),
        ["own"],
        "the data that was in use is left as it was"
    );
}

#[test]
fn a_data_directory_that_has_gone_is_reported_and_another_can_be_chosen() {
    let home = tempfile::tempdir().unwrap();
    let dictionaries = tempfile::tempdir().unwrap();
    let synced = tempfile::tempdir().unwrap();
    let to = synced.path().join("OpenDict");
    let mut data = DataDirectory::locate(home.path()).unwrap();
    fill(&data, dictionaries.path());
    relocate(&mut data, &to).unwrap();
    std::fs::rename(&to, synced.path().join("Elsewhere")).unwrap();

    let error = DataDirectory::locate(home.path()).unwrap_err();

    assert!(
        matches!(&error, opendict_core::Error::DataDirectoryMissing(path) if path == &to),
        "{error:?}"
    );
    let data = DataDirectory::use_folder(home.path(), &synced.path().join("Elsewhere")).unwrap();
    assert_eq!(contents(&data), expected());
    let fresh = DataDirectory::use_folder(home.path(), home.path()).unwrap();
    assert!(fresh.is_default());
    assert!(DataDirectory::locate(home.path()).unwrap().is_default());
}

#[test]
fn on_first_start_the_home_folder_need_not_exist_yet() {
    let parent = tempfile::tempdir().unwrap();
    let home = parent.path().join("io.github.amemiya02.opendict");

    let data = DataDirectory::locate(&home).unwrap();

    assert!(data.is_default());
    assert_eq!(data.history_file(), home.join("history.json"));
}
