//! Groups: named, ordered subsets of the Library, and the Current Group every Lookup
//! runs against.

use std::path::Path;

use opendict_core::{Library, Move, Suggestion};
use opendict_mdict_fixture::MdxBuilder;

fn put(folder: &Path, relative: &str, title: &str, headwords: &[&str]) {
    headwords
        .iter()
        .fold(MdxBuilder::new().title(title), |builder, headword| {
            builder.entry(headword, &format!("<p>{headword} in {title} (sample)</p>"))
        })
        .write_to(folder.join(relative))
        .unwrap();
}

/// A Library of three Dictionaries, A, B and C, all having "apple", indexed. A and B are
/// English; C alone has the Japanese word 食べる.
fn abc(data: &Path, folder: &Path) -> Library {
    put(folder, "a.mdx", "A", &["apple", "apples", "serendipity"]);
    put(folder, "b.mdx", "B", &["apple", "banana"]);
    put(folder, "c.mdx", "C", &["apple", "食べる"]);
    let mut library = Library::load(&data.join("library.json")).unwrap();
    library.add_folder(folder).unwrap();
    library.build_indexes();
    library
}

fn id_of(library: &Library, title: &str) -> String {
    library
        .dictionaries()
        .find(|d| d.title == title)
        .unwrap()
        .id
        .clone()
}

fn answering(library: &Library, headword: &str) -> Vec<String> {
    library
        .look_up(headword)
        .unwrap()
        .into_iter()
        .map(|entry| entry.dictionary_title)
        .collect()
}

fn member_titles(library: &Library, group: &str) -> Vec<String> {
    let group = library
        .groups()
        .into_iter()
        .find(|g| g.id == group)
        .unwrap();
    group
        .dictionaries
        .iter()
        .map(|id| {
            library
                .dictionaries()
                .find(|d| &d.id == id)
                .unwrap()
                .title
                .clone()
        })
        .collect()
}

#[test]
fn without_a_current_group_every_enabled_dictionary_answers_in_library_order() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let library = abc(data.path(), folder.path());

    assert_eq!(library.current_group(), None);
    assert!(library.groups().is_empty());
    assert_eq!(answering(&library, "apple"), ["A", "B", "C"]);
}

#[test]
fn a_lookup_runs_against_the_current_group_in_its_own_order() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let reading = library.create_group("Close reading").unwrap();
    library
        .set_in_group(&reading, &id_of(&library, "B"), true)
        .unwrap();
    library
        .set_in_group(&reading, &id_of(&library, "A"), true)
        .unwrap();

    library.set_current_group(Some(&reading)).unwrap();

    assert_eq!(library.current_group(), Some(reading.as_str()));
    assert_eq!(answering(&library, "apple"), ["B", "A"]);
    assert_eq!(answering(&library, "食べる"), Vec::<String>::new());
}

#[test]
fn a_dictionary_belongs_to_several_groups_with_its_own_place_in_each() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let (a, b, c) = (
        id_of(&library, "A"),
        id_of(&library, "B"),
        id_of(&library, "C"),
    );
    let english = library.create_group("English").unwrap();
    let everything = library.create_group("Everything").unwrap();
    for id in [&a, &b] {
        library.set_in_group(&english, id, true).unwrap();
    }
    for id in [&c, &b, &a] {
        library.set_in_group(&everything, id, true).unwrap();
    }

    library.move_in_group(&english, &b, Move::Up).unwrap();
    library.move_in_group(&everything, &b, Move::Down).unwrap();

    assert_eq!(member_titles(&library, &english), ["B", "A"]);
    assert_eq!(member_titles(&library, &everything), ["C", "A", "B"]);
    let library_order: Vec<String> = library.dictionaries().map(|d| d.title.clone()).collect();
    assert_eq!(library_order, ["A", "B", "C"], "Library order is untouched");
    library.set_current_group(Some(&everything)).unwrap();
    assert_eq!(answering(&library, "apple"), ["C", "A", "B"]);
}

#[test]
fn moving_past_either_end_of_a_group_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let (a, b) = (id_of(&library, "A"), id_of(&library, "B"));
    let english = library.create_group("English").unwrap();
    library.set_in_group(&english, &a, true).unwrap();
    library.set_in_group(&english, &b, true).unwrap();

    library.move_in_group(&english, &a, Move::Up).unwrap();
    library.move_in_group(&english, &b, Move::Down).unwrap();

    assert_eq!(member_titles(&library, &english), ["A", "B"]);
    let c = id_of(&library, "C");
    assert!(
        library.move_in_group(&english, &c, Move::Up).is_err(),
        "C is not a member"
    );
}

#[test]
fn a_dictionary_taken_out_of_the_current_group_no_longer_answers() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let (a, b) = (id_of(&library, "A"), id_of(&library, "B"));
    let english = library.create_group("English").unwrap();
    library.set_in_group(&english, &a, true).unwrap();
    library.set_in_group(&english, &b, true).unwrap();
    library.set_in_group(&english, &b, true).unwrap();
    library.set_current_group(Some(&english)).unwrap();

    library.set_in_group(&english, &a, false).unwrap();

    assert_eq!(member_titles(&library, &english), ["B"]);
    assert_eq!(answering(&library, "apple"), ["B"]);
}

#[test]
fn a_disabled_dictionary_does_not_answer_in_any_group() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let (a, b) = (id_of(&library, "A"), id_of(&library, "B"));
    let english = library.create_group("English").unwrap();
    library.set_in_group(&english, &a, true).unwrap();
    library.set_in_group(&english, &b, true).unwrap();
    library.set_current_group(Some(&english)).unwrap();

    library.set_enabled(&a, false).unwrap();

    assert_eq!(
        member_titles(&library, &english),
        ["A", "B"],
        "still a member"
    );
    assert_eq!(answering(&library, "apple"), ["B"]);
}

#[test]
fn groups_are_renamed_and_deleted_and_need_names_of_their_own() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let english = library.create_group("English").unwrap();
    let japanese = library.create_group("  日本語 ").unwrap();

    assert!(library.create_group("English").is_err());
    assert!(library.create_group("   ").is_err());
    assert!(library.rename_group(&japanese, "English").is_err());
    library.rename_group(&english, "English").unwrap();
    library.rename_group(&english, "Close reading").unwrap();

    let names: Vec<String> = library.groups().into_iter().map(|g| g.name).collect();
    assert_eq!(names, ["Close reading", "日本語"]);

    library.delete_group(&english).unwrap();

    let ids: Vec<String> = library.groups().into_iter().map(|g| g.id).collect();
    assert_eq!(ids, [japanese.as_str()]);
    assert!(library.rename_group(&english, "Gone").is_err());
    let another = library.create_group("English").unwrap();
    assert_ne!(another, japanese, "a new Group gets a key of its own");
}

#[test]
fn deleting_the_current_group_returns_lookups_to_the_whole_library() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let english = library.create_group("English").unwrap();
    library
        .set_in_group(&english, &id_of(&library, "A"), true)
        .unwrap();
    library.set_current_group(Some(&english)).unwrap();

    library.delete_group(&english).unwrap();

    assert_eq!(library.current_group(), None);
    assert_eq!(answering(&library, "apple"), ["A", "B", "C"]);
    assert_eq!(library.dictionaries().count(), 3, "Dictionaries stay");
}

#[test]
fn an_unknown_group_cannot_be_made_current() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());

    assert!(library.set_current_group(Some("nowhere")).is_err());
    assert_eq!(library.current_group(), None);
}

#[test]
fn groups_and_the_current_group_are_remembered_between_starts() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let (b, c) = (id_of(&library, "B"), id_of(&library, "C"));
    let mixed = library.create_group("Mixed").unwrap();
    library.set_in_group(&mixed, &c, true).unwrap();
    library.set_in_group(&mixed, &b, true).unwrap();
    library.set_current_group(Some(&mixed)).unwrap();
    drop(library);

    let mut library = Library::load(&data.path().join("library.json")).unwrap();
    library.build_indexes();

    assert_eq!(library.current_group(), Some(mixed.as_str()));
    assert_eq!(member_titles(&library, &mixed), ["C", "B"]);
    assert_eq!(answering(&library, "apple"), ["C", "B"]);
}

#[test]
fn a_dictionary_missing_for_a_while_keeps_its_place_in_its_groups() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let (a, b, c) = (
        id_of(&library, "A"),
        id_of(&library, "B"),
        id_of(&library, "C"),
    );
    let group = library.create_group("All three").unwrap();
    for id in [&c, &b, &a] {
        library.set_in_group(&group, id, true).unwrap();
    }
    drop(library);
    let away = data.path().join("b.mdx.away");
    std::fs::rename(folder.path().join("b.mdx"), &away).unwrap();
    let mut library = Library::load(&data.path().join("library.json")).unwrap();

    assert_eq!(member_titles(&library, &group), ["C", "A"]);
    library.move_in_group(&group, &a, Move::Up).unwrap();
    drop(library);
    std::fs::rename(&away, folder.path().join("b.mdx")).unwrap();
    let library = Library::load(&data.path().join("library.json")).unwrap();

    assert_eq!(member_titles(&library, &group), ["A", "B", "C"]);
}

#[test]
fn suggestions_and_their_counts_follow_the_current_group() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let japanese = library.create_group("日本語").unwrap();
    library
        .set_in_group(&japanese, &id_of(&library, "C"), true)
        .unwrap();
    let count = |items: &[Suggestion], headword: &str| {
        items
            .iter()
            .find(|i| i.headword == headword)
            .map(|i| i.count)
    };
    let everything = library.suggest("", 10).unwrap();
    assert_eq!(count(&everything.items, "apple"), Some(3));
    assert_eq!(count(&everything.items, "banana"), Some(1));

    library.set_current_group(Some(&japanese)).unwrap();

    let browsed = library.suggest("", 10).unwrap();
    let headwords: Vec<&str> = browsed.items.iter().map(|i| i.headword.as_str()).collect();
    assert_eq!(headwords, ["apple", "食べる"]);
    assert_eq!(count(&browsed.items, "apple"), Some(1));
    assert!(library.suggest("banana", 10).unwrap().miss);
    assert_eq!(
        library.headwords_after("apple", 10).unwrap()[0].headword,
        "食べる"
    );
}

#[test]
fn a_base_form_is_offered_only_when_the_current_group_has_it() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let japanese = library.create_group("日本語").unwrap();
    library
        .set_in_group(&japanese, &id_of(&library, "C"), true)
        .unwrap();
    let lemma = library.suggest("serendipities", 10).unwrap().lemma.unwrap();
    assert_eq!(lemma.to, "serendipity");

    library.set_current_group(Some(&japanese)).unwrap();

    assert_eq!(library.suggest("serendipities", 10).unwrap().lemma, None);
    assert_eq!(
        library.suggest("食べた", 10).unwrap().lemma.unwrap().to,
        "食べる"
    );
}

#[test]
fn a_group_switch_keeps_the_word_when_the_new_group_has_it() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let english = library.create_group("English").unwrap();
    let japanese = library.create_group("日本語").unwrap();
    for title in ["A", "B"] {
        library
            .set_in_group(&english, &id_of(&library, title), true)
            .unwrap();
    }
    library
        .set_in_group(&japanese, &id_of(&library, "C"), true)
        .unwrap();
    library.set_current_group(Some(&english)).unwrap();
    assert_eq!(answering(&library, "apple"), ["A", "B"]);

    library.set_current_group(Some(&japanese)).unwrap();

    assert_eq!(
        answering(&library, "apple"),
        ["C"],
        "apple stays on the page"
    );
    assert_eq!(answering(&library, "banana"), Vec::<String>::new());
}

#[test]
fn a_miss_points_to_another_group_that_has_the_word() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut library = abc(data.path(), folder.path());
    let english = library.create_group("English").unwrap();
    let japanese = library.create_group("日本語").unwrap();
    library
        .set_in_group(&english, &id_of(&library, "A"), true)
        .unwrap();
    library
        .set_in_group(&japanese, &id_of(&library, "C"), true)
        .unwrap();
    library.set_current_group(Some(&english)).unwrap();
    assert!(library.suggest("食べる", 10).unwrap().miss);

    let found = library.elsewhere("食べる").unwrap().unwrap();
    assert_eq!(found.group.as_deref(), Some(japanese.as_str()));
    assert_eq!(found.headword, "食べる");
    // An inflected form leads to its base form there.
    assert_eq!(
        library.elsewhere("食べた").unwrap().unwrap().headword,
        "食べる"
    );
    // Only the whole Library has "banana": B is in no Group.
    let banana = library.elsewhere("banana").unwrap().unwrap();
    assert_eq!((banana.group, banana.headword.as_str()), (None, "banana"));
    // A headword that starts with it counts; a close spelling does not.
    assert_eq!(
        library.elsewhere("bana").unwrap().unwrap().headword,
        "banana"
    );
    assert_eq!(library.elsewhere("bananna").unwrap(), None);
    // Nothing anywhere, a pattern, an empty query: nowhere to point to.
    assert_eq!(library.elsewhere("zzzz").unwrap(), None);
    assert_eq!(library.elsewhere("ban*").unwrap(), None);
    assert_eq!(library.elsewhere("").unwrap(), None);

    library.set_current_group(None).unwrap();
    assert_eq!(
        library.elsewhere("banana").unwrap(),
        None,
        "the whole Library is not elsewhere from itself"
    );
}

#[test]
fn a_current_group_the_library_file_does_not_hold_falls_back_to_the_whole_library() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    drop(abc(data.path(), folder.path()));
    let file = data.path().join("library.json");
    let mut record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    record["currentGroup"] = serde_json::json!("g9");
    std::fs::write(&file, record.to_string()).unwrap();

    let mut library = Library::load(&file).unwrap();
    library.build_indexes();

    assert_eq!(library.current_group(), None);
    assert_eq!(answering(&library, "apple"), ["A", "B", "C"]);
}
