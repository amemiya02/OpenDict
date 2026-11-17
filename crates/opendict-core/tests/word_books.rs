//! Word Books: named lists of saved Headwords, kept in a file the core owns, and exported
//! to CSV or TSV for Anki.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use opendict_core::{ExportFormat, SavedFrom, WordBook, WordBookId, WordBooks};

/// 2026-11-17T09:30:00Z
fn morning() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_907_800)
}

fn english() -> SavedFrom {
    SavedFrom {
        dictionary_id: "0123456789abcdef".to_owned(),
        dictionary_title: "Sample English Dictionary".to_owned(),
    }
}

fn file(folder: &tempfile::TempDir) -> PathBuf {
    folder.path().join("word-books.json")
}

fn headwords(book: &WordBook) -> Vec<&str> {
    book.words.iter().map(|w| w.headword.as_str()).collect()
}

#[test]
fn a_saved_word_remembers_its_dictionary_and_when_it_was_saved() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Reading").unwrap();

    let saved = books
        .toggle(book, "serendipity", english(), morning())
        .unwrap();

    assert!(saved, "the word was not in the Word Book, so it is added");
    let reloaded = WordBooks::load(&file(&folder)).unwrap();
    let word = &reloaded.book(book).unwrap().words[0];
    assert_eq!(word.headword, "serendipity");
    assert_eq!(word.saved_from, english());
    assert_eq!(word.added, morning());
    assert_eq!(word.note, None);
    assert_eq!(headwords(reloaded.current().unwrap()), ["serendipity"]);
}

#[test]
fn saving_a_word_that_is_already_there_removes_it() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Reading").unwrap();
    books.toggle(book, "apple", english(), morning()).unwrap();
    books
        .toggle(book, "serendipity", english(), morning())
        .unwrap();

    let saved = books.toggle(book, "apple", english(), morning()).unwrap();

    assert!(!saved);
    assert_eq!(
        headwords(WordBooks::load(&file(&folder)).unwrap().book(book).unwrap()),
        ["serendipity"]
    );
}

#[test]
fn the_reader_chooses_which_of_several_word_books_to_add_to() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let novel = books.create("The Remains of the Day").unwrap();
    let paper = books.create("Paper reading").unwrap();
    assert_eq!(
        books.current().unwrap().id,
        paper,
        "a new Word Book is chosen"
    );

    books.choose(novel).unwrap();
    let current = books.current().unwrap().id;
    books
        .toggle(current, "banter", english(), morning())
        .unwrap();

    let reloaded = WordBooks::load(&file(&folder)).unwrap();
    assert_eq!(reloaded.current().unwrap().name, "The Remains of the Day");
    let names: Vec<&str> = reloaded.books().iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["The Remains of the Day", "Paper reading"]);
    assert_eq!(headwords(reloaded.book(novel).unwrap()), ["banter"]);
    assert!(reloaded.book(paper).unwrap().words.is_empty());
}

#[test]
fn the_same_word_can_be_in_two_word_books() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let first = books.create("One").unwrap();
    let second = books.create("Two").unwrap();

    assert!(books.toggle(first, "apple", english(), morning()).unwrap());
    assert!(books.toggle(second, "apple", english(), morning()).unwrap());

    assert_eq!(headwords(books.book(first).unwrap()), ["apple"]);
    assert_eq!(headwords(books.book(second).unwrap()), ["apple"]);
}

#[test]
fn a_saved_word_can_carry_a_note_or_example_sentence() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Reading").unwrap();
    books.toggle(book, "banter", english(), morning()).unwrap();

    books
        .set_note(book, "banter", "  “Some banter at breakfast,” p. 12  ")
        .unwrap();

    let reloaded = WordBooks::load(&file(&folder)).unwrap();
    assert_eq!(
        reloaded.book(book).unwrap().words[0].note.as_deref(),
        Some("“Some banter at breakfast,” p. 12")
    );

    books.set_note(book, "banter", "   ").unwrap();
    assert_eq!(
        books.book(book).unwrap().words[0].note,
        None,
        "a blank note is no note"
    );
}

#[test]
fn a_note_needs_the_word_to_be_in_the_word_book() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Reading").unwrap();

    assert!(books.set_note(book, "banter", "a note").is_err());
}

#[test]
fn words_can_be_removed() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Reading").unwrap();
    books.toggle(book, "apple", english(), morning()).unwrap();
    books.toggle(book, "banter", english(), morning()).unwrap();

    books.remove(book, "apple").unwrap();

    let reloaded = WordBooks::load(&file(&folder)).unwrap();
    assert_eq!(headwords(reloaded.book(book).unwrap()), ["banter"]);
}

#[test]
fn word_books_can_be_renamed() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Reading").unwrap();

    books.rename(book, " Middlemarch ").unwrap();

    assert_eq!(
        WordBooks::load(&file(&folder))
            .unwrap()
            .book(book)
            .unwrap()
            .name,
        "Middlemarch"
    );
    assert!(
        books.rename(book, "  ").is_err(),
        "a Word Book needs a name"
    );
    assert!(books.create("").is_err());
}

#[test]
fn deleting_the_chosen_word_book_chooses_another() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let first = books.create("One").unwrap();
    let second = books.create("Two").unwrap();
    books.toggle(second, "apple", english(), morning()).unwrap();

    books.delete(second).unwrap();

    let reloaded = WordBooks::load(&file(&folder)).unwrap();
    assert!(reloaded.book(second).is_none());
    assert_eq!(reloaded.current().unwrap().id, first);

    books.delete(first).unwrap();
    assert!(books.books().is_empty() && books.current().is_none());
    let third = books.create("Three").unwrap();
    assert_ne!(third, first, "ids are not reused");
    assert_ne!(third, second);
}

#[test]
fn an_unknown_word_book_is_an_error() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("One").unwrap();
    books.delete(book).unwrap();

    assert!(books.toggle(book, "apple", english(), morning()).is_err());
    assert!(books.choose(book).is_err());
    assert!(books.rename(book, "Two").is_err());
    assert!(books.delete(book).is_err());
}

#[test]
fn a_damaged_word_books_file_is_an_error() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(file(&folder), "{ books: [").unwrap();

    assert!(WordBooks::load(&file(&folder)).is_err());
}

#[test]
fn there_are_no_word_books_until_the_reader_makes_one() {
    let folder = tempfile::tempdir().unwrap();

    let books = WordBooks::load(&file(&folder)).unwrap();

    assert!(books.books().is_empty());
    assert!(books.current().is_none());
    assert!(
        !file(&folder).exists(),
        "nothing is written until something changes"
    );
}

fn japanese() -> SavedFrom {
    SavedFrom {
        dictionary_id: "fedcba9876543210".to_owned(),
        dictionary_title: "サンプル国語辞典".to_owned(),
    }
}

/// A Word Book with a plain word, then (newer) one whose note needs quoting.
fn reading_list(folder: &tempfile::TempDir) -> (WordBooks, WordBookId) {
    let mut books = WordBooks::load(&file(folder)).unwrap();
    let book = books.create("Reading").unwrap();
    books.toggle(book, "食べる", japanese(), morning()).unwrap();
    let later = morning() + Duration::from_secs(90);
    books.toggle(book, "banter", english(), later).unwrap();
    books
        .set_note(
            book,
            "banter",
            "\"Some banter,\" he said,\tat breakfast\np. 12",
        )
        .unwrap();
    (books, book)
}

#[test]
fn a_word_book_exports_to_csv_with_a_header_row() {
    let folder = tempfile::tempdir().unwrap();
    let (books, book) = reading_list(&folder);
    let exported = folder.path().join("Reading.csv");

    books.export(book, ExportFormat::Csv, &exported).unwrap();

    assert_eq!(
        std::fs::read_to_string(&exported).unwrap(),
        concat!(
            "Headword,Note,Dictionary,Added\n",
            "banter,\"\"\"Some banter,\"\" he said,\tat breakfast\np. 12\",Sample English Dictionary,2026-11-17T09:31:30Z\n",
            "食べる,,サンプル国語辞典,2026-11-17T09:30:00Z\n",
        )
    );
}

#[test]
fn a_word_book_exports_to_tsv_with_a_header_row() {
    let folder = tempfile::tempdir().unwrap();
    let (books, book) = reading_list(&folder);
    let exported = folder.path().join("Reading.tsv");

    books.export(book, ExportFormat::Tsv, &exported).unwrap();

    assert_eq!(
        std::fs::read_to_string(&exported).unwrap(),
        concat!(
            "Headword\tNote\tDictionary\tAdded\n",
            "banter\t\"\"\"Some banter,\"\" he said,\tat breakfast\np. 12\"\tSample English Dictionary\t2026-11-17T09:31:30Z\n",
            "食べる\t\tサンプル国語辞典\t2026-11-17T09:30:00Z\n",
        )
    );
}

#[test]
fn an_empty_word_book_exports_its_header_row() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Empty").unwrap();
    let exported = folder.path().join("exports").join("Empty.csv");

    books.export(book, ExportFormat::Csv, &exported).unwrap();

    assert_eq!(
        std::fs::read_to_string(&exported).unwrap(),
        "Headword,Note,Dictionary,Added\n"
    );
}

#[test]
fn exported_times_are_calendar_dates_in_utc() {
    let folder = tempfile::tempdir().unwrap();
    let mut books = WordBooks::load(&file(&folder)).unwrap();
    let book = books.create("Dates").unwrap();
    let at = |seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
    books
        .toggle(book, "leap", english(), at(1_835_481_599))
        .unwrap();
    books
        .toggle(book, "millennium", english(), at(946_684_800))
        .unwrap();
    let exported = folder.path().join("Dates.tsv");

    books.export(book, ExportFormat::Tsv, &exported).unwrap();

    let text = std::fs::read_to_string(&exported).unwrap();
    let added: Vec<&str> = text
        .lines()
        .skip(1)
        .map(|line| line.rsplit('\t').next().unwrap())
        .collect();
    assert_eq!(added, ["2000-01-01T00:00:00Z", "2028-02-29T23:59:59Z"]);
}
