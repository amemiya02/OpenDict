//! The persistent per-Dictionary index through the public interface of `opendict-core`:
//! built when a Dictionary is first found, reused on later starts, rebuilt when the file
//! changes, with progress, cancellation and failures that stay with their Dictionary.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use opendict_core::{IndexResult, IndexState, Library, Start};
use opendict_mdict_fixture::{Damage, MdxBuilder};

fn sample() -> MdxBuilder {
    MdxBuilder::new()
        .title("Sample")
        .entry("apple", "<p>a round fruit (sample)</p>")
        .entry("Apricot", "<p>an orange fruit (sample)</p>")
        .entry("bank", "<p>a river side (sample)</p>")
        .entry("bank", "<p>a place for money (sample)</p>")
        .link("colour", "color")
        .entry("color", "<p>what light shows (sample)</p>")
        .keys_per_block(2)
        .records_per_block(2)
}

fn put(folder: &Path, relative: &str, dictionary: &MdxBuilder) -> PathBuf {
    let path = folder.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    dictionary.write_to(&path).unwrap();
    path
}

fn load(data: &Path) -> Library {
    Library::load(&data.join("library.json")).unwrap()
}

fn states(library: &Library) -> Vec<(String, IndexState)> {
    library
        .dictionaries()
        .map(|d| (d.title.clone(), d.index.clone()))
        .collect()
}

fn html(library: &Library, headword: &str) -> Vec<String> {
    library
        .look_up(headword)
        .unwrap()
        .into_iter()
        .map(|entry| entry.html)
        .collect()
}

#[test]
fn a_dictionary_is_indexed_when_first_found_and_answers_only_once_indexed() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    assert_eq!(
        states(&library),
        [("Sample".to_owned(), IndexState::Pending)]
    );
    assert_eq!(html(&library, "apple"), Vec::<String>::new());

    let jobs = library.index_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].title(), "Sample");
    assert_eq!(jobs[0].entry_count(), 6);
    for job in jobs {
        let outcome = job.run(&mut |_| ControlFlow::Continue(()));
        library.finish_index(outcome);
    }

    assert_eq!(states(&library), [("Sample".to_owned(), IndexState::Ready)]);
    assert_eq!(html(&library, "apple"), ["<p>a round fruit (sample)</p>"]);
    assert_eq!(
        html(&library, "bank"),
        ["<p>a river side (sample)</p>\n<p>a place for money (sample)</p>"]
    );
    assert_eq!(
        html(&library, "colour"),
        ["<p>what light shows (sample)</p>"]
    );
    assert!(library.index_jobs().is_empty());
}

#[test]
fn the_index_is_stored_in_the_data_directory_and_reused_on_later_starts() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample());
    {
        let mut library = load(data.path());
        library.add_folder(folder.path()).unwrap();
        library.build_indexes();
    }
    let stored: Vec<_> = std::fs::read_dir(data.path().join("indexes"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(stored.len(), 1, "{stored:?}");

    let library = load(data.path());

    assert_eq!(states(&library), [("Sample".to_owned(), IndexState::Ready)]);
    assert!(library.index_jobs().is_empty());
    assert_eq!(
        html(&library, "Apricot"),
        ["<p>an orange fruit (sample)</p>"]
    );
}

#[test]
fn a_dictionary_whose_file_changes_is_indexed_again() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = put(folder.path(), "sample.mdx", &sample());
    {
        let mut library = load(data.path());
        library.add_folder(folder.path()).unwrap();
        library.build_indexes();
    }

    // A new edition: same header, different headwords.
    put(
        folder.path(),
        "sample.mdx",
        &sample().entry("zebra", "<p>a striped horse (sample)</p>"),
    );
    let mut library = load(data.path());
    assert_eq!(
        states(&library),
        [("Sample".to_owned(), IndexState::Pending)]
    );
    library.build_indexes();
    assert_eq!(html(&library, "zebra"), ["<p>a striped horse (sample)</p>"]);
    drop(library);

    // The same bytes written again: only the modification time changes.
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(60))
        .unwrap();
    let library = load(data.path());
    assert_eq!(
        states(&library),
        [("Sample".to_owned(), IndexState::Pending)]
    );
}

#[test]
fn indexing_reports_progress_up_to_the_entry_count() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    let mut reported = Vec::new();
    let job = library.index_jobs().remove(0);
    let outcome = job.run(&mut |done| {
        reported.push(done);
        ControlFlow::Continue(())
    });
    library.finish_index(outcome);

    assert!(reported.len() > 1, "{reported:?}");
    assert!(reported.is_sorted(), "{reported:?}");
    assert_eq!(reported.last(), Some(&6));
}

#[test]
fn indexing_can_be_cancelled_and_leaves_the_dictionary_waiting() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    // The reader cancels as soon as the first progress report arrives.
    let job = library.index_jobs().remove(0);
    let outcome = job.run(&mut |_| ControlFlow::Break(()));
    assert!(matches!(outcome.result, IndexResult::Cancelled));
    library.finish_index(outcome);

    assert_eq!(
        states(&library),
        [("Sample".to_owned(), IndexState::Pending)]
    );
    assert_eq!(library.index_jobs().len(), 1);
    let stored = std::fs::read_dir(data.path().join("indexes"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(stored, 0, "a cancelled index leaves no file behind");
}

#[test]
fn a_dictionary_that_fails_to_index_does_not_hold_up_the_others() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "a-good.mdx", &sample().title("Good"));
    std::fs::write(
        folder.path().join("b-damaged.mdx"),
        sample().title("Damaged").build_damaged(Damage::KeyBlock),
    )
    .unwrap();
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();

    library.build_indexes();

    let states = states(&library);
    assert_eq!(states[0], ("Good".to_owned(), IndexState::Ready));
    assert_eq!(states[1].0, "Damaged");
    assert!(
        matches!(&states[1].1, IndexState::Failed(reason) if !reason.is_empty()),
        "{states:?}"
    );
    assert_eq!(html(&library, "apple"), ["<p>a round fruit (sample)</p>"]);
}

#[test]
fn the_index_answers_prefix_queries_in_index_order() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample());
    let mut library = load(data.path());
    library.add_folder(folder.path()).unwrap();
    library.build_indexes();
    let id = library.dictionaries().next().unwrap().id.clone();
    let index = library.index_of(&id).unwrap();

    let headwords = |start, limit| -> Vec<String> {
        index
            .headwords(start, limit)
            .unwrap()
            .into_iter()
            .map(|item| item.headword)
            .collect()
    };

    // Keys ignore letter case; each headword is listed once, then the column continues.
    assert_eq!(
        headwords(Start::AtKey("ap"), 3),
        ["apple", "Apricot", "bank"]
    );
    assert_eq!(
        headwords(Start::AtKey("b"), 10),
        ["bank", "color", "colour"]
    );
    assert_eq!(headwords(Start::AtKey(""), 2), ["apple", "Apricot"]);
    assert_eq!(headwords(Start::AtKey("zzz"), 5), Vec::<String>::new());
    let bank = index.headwords(Start::AtKey("bank"), 1).unwrap().remove(0);
    assert_eq!(headwords(Start::After(&bank), 5), ["color", "colour"]);
}

#[test]
fn an_in_memory_library_is_indexed_before_it_is_returned() {
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample());

    let library = Library::open(folder.path()).unwrap();

    assert_eq!(states(&library), [("Sample".to_owned(), IndexState::Ready)]);
    assert_eq!(html(&library, "apple"), ["<p>a round fruit (sample)</p>"]);
}

#[test]
fn a_damaged_stored_index_is_rebuilt_not_trusted() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    put(folder.path(), "sample.mdx", &sample());
    {
        let mut library = load(data.path());
        library.add_folder(folder.path()).unwrap();
        library.build_indexes();
    }
    for entry in std::fs::read_dir(data.path().join("indexes")).unwrap() {
        let path = entry.unwrap().path();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.truncate(bytes.len() / 2);
        std::fs::write(&path, bytes).unwrap();
    }

    let mut library = load(data.path());
    library.build_indexes();

    assert_eq!(states(&library), [("Sample".to_owned(), IndexState::Ready)]);
    assert_eq!(
        html(&library, "color"),
        ["<p>what light shows (sample)</p>"]
    );
}
