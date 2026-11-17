use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use std::time::{SystemTime, UNIX_EPOCH};

use opendict_core::{
    Error as CoreError, ExportFormat, Library, Move, OpenError, Preferences, Registration,
    SavedFrom, Suggestion, WordBookId, WordBooks,
};
use serde::{Deserialize, Serialize};

use crate::entry_protocol;

/// What the shell holds between commands: the Library, or why it could not be loaded, and
/// the reader's preferences and Word Books, or why they could not be read.
pub struct AppState {
    library: Mutex<Result<Library, String>>,
    preferences: Mutex<Result<Preferences, String>>,
    word_books: Mutex<Result<WordBooks, String>>,
}

/// An Entry as the interface receives it. Its HTML never passes through the interface:
/// the frame loads it from the Entry scheme at `src`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub dictionary_id: String,
    pub dictionary_title: String,
    /// The Entry document's URL, for a sandboxed frame.
    pub src: String,
    /// Whether the reader allowed this Dictionary network access.
    pub allow_network: bool,
    /// How the headword is said, where the Entry shows that plainly. Plain text.
    pub pronunciation: Option<String>,
}

/// What the headword column offers for a query: the Lookup result shape settled by the
/// prototype (`design/prototype/core.js`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionsView {
    pub items: Vec<SuggestionView>,
    pub lemma: Option<LemmaView>,
    pub near: bool,
    pub miss: bool,
    pub browse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionView {
    pub headword: String,
    /// How many enabled Dictionaries file the headword.
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LemmaView {
    pub from: String,
    pub to: String,
}

/// The Library as the Library view shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub folders: Vec<FolderView>,
    /// Every Dictionary, enabled or not, in Library order.
    pub dictionaries: Vec<DictionaryView>,
    pub failures: Vec<FailureView>,
    /// Why the Library could not be loaded at all, if it could not.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderView {
    pub path: PathBuf,
    pub dictionary_count: usize,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryView {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub entry_count: usize,
    /// Bytes, all of the Dictionary's files together.
    pub size: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureView {
    pub path: PathBuf,
    pub reason: FailureReason,
}

/// Why a Dictionary did not open, as a kind the interface words in the reader's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum FailureReason {
    Unreadable(String),
    NotADictionary,
    UnsupportedVersion(String),
    Damaged,
    NeedsRegistrationCode,
    WrongRegistrationCode,
    Unsupported(String),
}

/// Why a registration code was not accepted, as a kind the interface words in the
/// reader's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum RegisterError {
    /// The code or the e-mail does not unlock the Dictionary.
    WrongCode,
    /// Only an e-mail was entered, and the Dictionary does not carry its code.
    CodeNeeded,
    /// Anything else (the Library could not be saved, the file went away...).
    Failed(String),
}

/// Every Word Book, and which one new words go into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordBooksView {
    /// In the order they were made.
    pub books: Vec<WordBookView>,
    pub current: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordBookView {
    pub id: u64,
    pub name: String,
    /// Newest first.
    pub words: Vec<SavedWordView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedWordView {
    pub headword: String,
    pub dictionary_id: String,
    pub dictionary_title: String,
    pub note: Option<String>,
    /// Milliseconds since 1970-01-01 UTC.
    pub added: u64,
}

/// A step in Library order, as the interface sends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Up,
    Down,
}

impl AppState {
    /// Loads the Library recorded in `library_file`. With `override_folder` (from the
    /// environment, for tests and development) the Library is that one folder instead,
    /// kept only in memory, so the reader's own Library is never touched. Preferences
    /// live in `data_dir`.
    pub fn load(
        library_file: &Path,
        override_folder: Option<PathBuf>,
        data_dir: &Path,
    ) -> AppState {
        let library = match override_folder {
            Some(folder) => Library::open(&folder),
            None => Library::load(library_file),
        };
        let preferences = Preferences::load(&data_dir.join("preferences.json"));
        let word_books = WordBooks::load(&data_dir.join("word-books.json"));
        AppState {
            library: Mutex::new(library.map_err(|error| error.to_string())),
            preferences: Mutex::new(preferences.map_err(|error| error.to_string())),
            word_books: Mutex::new(word_books.map_err(|error| error.to_string())),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Result<Library, String>> {
        // A panic while holding the lock leaves the Library as it was; keep serving it.
        self.library
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Applies `change` to the Library and returns the Library as it is afterwards.
    fn change(
        &self,
        change: impl FnOnce(&mut Library) -> opendict_core::Result<()>,
    ) -> Result<LibraryView, String> {
        let mut guard = self.lock();
        let library = guard.as_mut().map_err(|problem| problem.clone())?;
        change(library).map_err(|error| error.to_string())?;
        Ok(view(&guard))
    }

    /// The Entries for exactly `headword`, from enabled Dictionaries in Library order.
    /// Without a Library there are none.
    pub fn look_up(&self, headword: &str) -> Result<Vec<EntryView>, String> {
        let guard = self.lock();
        let Ok(library) = &*guard else {
            return Ok(Vec::new());
        };
        let entries = library
            .look_up(headword)
            .map_err(|error| error.to_string())?;
        Ok(entries
            .into_iter()
            .map(|entry| EntryView {
                src: entry_protocol::entry_url(&entry.dictionary_id, headword),
                allow_network: self.allows_network(&entry.dictionary_id),
                pronunciation: opendict_core::pronunciation(&entry.html),
                dictionary_id: entry.dictionary_id,
                dictionary_title: entry.dictionary_title,
            })
            .collect())
    }

    /// Up to `limit` suggestions for `query`. Without a Library there are none.
    pub fn suggest(&self, query: &str, limit: usize) -> Result<SuggestionsView, String> {
        let guard = self.lock();
        let Ok(library) = &*guard else {
            return Ok(SuggestionsView {
                items: Vec::new(),
                lemma: None,
                near: false,
                miss: !query.trim().is_empty(),
                browse: query.trim().is_empty(),
            });
        };
        let suggestions = library
            .suggest(query, limit)
            .map_err(|error| error.to_string())?;
        Ok(SuggestionsView {
            items: suggestions.items.into_iter().map(suggestion).collect(),
            lemma: suggestions.lemma.map(|lemma| LemmaView {
                from: lemma.from,
                to: lemma.to,
            }),
            near: suggestions.near,
            miss: suggestions.miss,
            browse: suggestions.browse,
        })
    }

    /// Up to `limit` headwords after `headword`: the headword column, continued.
    pub fn headwords_after(
        &self,
        headword: &str,
        limit: usize,
    ) -> Result<Vec<SuggestionView>, String> {
        let guard = self.lock();
        let Ok(library) = &*guard else {
            return Ok(Vec::new());
        };
        let items = library
            .headwords_after(headword, limit)
            .map_err(|error| error.to_string())?;
        Ok(items.into_iter().map(suggestion).collect())
    }

    /// The HTML of one Dictionary's Entry for exactly `headword`.
    pub fn entry_html(
        &self,
        dictionary_id: &str,
        headword: &str,
    ) -> Result<Option<String>, String> {
        let guard = self.lock();
        let Ok(library) = &*guard else {
            return Ok(None);
        };
        library
            .entry(dictionary_id, headword)
            .map(|entry| entry.map(|e| e.html))
            .map_err(|error| error.to_string())
    }

    /// A file one Dictionary's Entries refer to.
    pub fn resource(&self, dictionary_id: &str, path: &str) -> Result<Option<Vec<u8>>, String> {
        let guard = self.lock();
        let Ok(library) = &*guard else {
            return Ok(None);
        };
        library
            .resource(dictionary_id, path)
            .map_err(|error| error.to_string())
    }

    /// Whether the reader allowed the Dictionary network access. If the preferences
    /// cannot be read, no Dictionary has it.
    pub fn allows_network(&self, dictionary_id: &str) -> bool {
        let preferences = self.preferences.lock().unwrap_or_else(|e| e.into_inner());
        preferences
            .as_ref()
            .is_ok_and(|p| p.allows_network(dictionary_id))
    }

    pub fn set_allow_network(&self, dictionary_id: &str, allow: bool) -> Result<(), String> {
        let mut preferences = self.preferences.lock().unwrap_or_else(|e| e.into_inner());
        match preferences.as_mut() {
            Ok(p) => p
                .set_allows_network(dictionary_id, allow)
                .map_err(|e| e.to_string()),
            Err(reason) => Err(reason.clone()),
        }
    }

    /// Applies `change` to the Word Books and returns them as they are afterwards. A
    /// Word Books file that could not be read is reported, never overwritten.
    fn change_word_books(
        &self,
        change: impl FnOnce(&mut WordBooks) -> opendict_core::Result<()>,
    ) -> Result<WordBooksView, String> {
        let mut guard = self.word_books.lock().unwrap_or_else(|e| e.into_inner());
        let books = guard.as_mut().map_err(|problem| problem.clone())?;
        change(books).map_err(|error| error.to_string())?;
        Ok(word_books_view(books))
    }

    pub fn word_books(&self) -> Result<WordBooksView, String> {
        self.change_word_books(|_| Ok(()))
    }

    /// Saves `headword` to the chosen Word Book, or takes it out if it is there. With no
    /// Word Book yet, one called `new_book_name` is made for it.
    pub fn toggle_word(
        &self,
        headword: &str,
        dictionary_id: &str,
        dictionary_title: &str,
        new_book_name: &str,
    ) -> Result<WordBooksView, String> {
        self.change_word_books(|books| {
            let book = match books.current() {
                Some(book) => book.id,
                None => books.create(new_book_name)?,
            };
            let saved_from = SavedFrom {
                dictionary_id: dictionary_id.to_owned(),
                dictionary_title: dictionary_title.to_owned(),
            };
            books
                .toggle(book, headword, saved_from, SystemTime::now())
                .map(|_| ())
        })
    }

    pub fn create_word_book(&self, name: &str) -> Result<WordBooksView, String> {
        self.change_word_books(|books| books.create(name).map(|_| ()))
    }

    pub fn rename_word_book(&self, id: u64, name: &str) -> Result<WordBooksView, String> {
        self.change_word_books(|books| books.rename(WordBookId(id), name))
    }

    pub fn delete_word_book(&self, id: u64) -> Result<WordBooksView, String> {
        self.change_word_books(|books| books.delete(WordBookId(id)))
    }

    pub fn choose_word_book(&self, id: u64) -> Result<WordBooksView, String> {
        self.change_word_books(|books| books.choose(WordBookId(id)))
    }

    pub fn remove_saved_word(&self, id: u64, headword: &str) -> Result<WordBooksView, String> {
        self.change_word_books(|books| books.remove(WordBookId(id), headword))
    }

    pub fn set_word_note(
        &self,
        id: u64,
        headword: &str,
        note: &str,
    ) -> Result<WordBooksView, String> {
        self.change_word_books(|books| books.set_note(WordBookId(id), headword, note))
    }

    /// Writes the Word Book to `file` as CSV or TSV.
    pub fn export_word_book(
        &self,
        id: u64,
        format: ExportFormat,
        file: &Path,
    ) -> Result<(), String> {
        let guard = self.word_books.lock().unwrap_or_else(|e| e.into_inner());
        let books = guard.as_ref().map_err(|problem| problem.clone())?;
        books
            .export(WordBookId(id), format, file)
            .map_err(|error| error.to_string())
    }

    /// The Word Book's name, to suggest a name for the file it is exported to.
    pub fn word_book_name(&self, id: u64) -> Option<String> {
        let guard = self.word_books.lock().unwrap_or_else(|e| e.into_inner());
        let books = guard.as_ref().ok()?;
        books.book(WordBookId(id)).map(|book| book.name.clone())
    }

    pub fn library(&self) -> LibraryView {
        view(&self.lock())
    }

    pub fn add_folder(&self, folder: &Path) -> Result<LibraryView, String> {
        self.change(|library| library.add_folder(folder))
    }

    pub fn remove_folder(&self, folder: &Path) -> Result<LibraryView, String> {
        self.change(|library| library.remove_folder(folder))
    }

    pub fn rescan(&self) -> Result<LibraryView, String> {
        self.change(Library::rescan)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<LibraryView, String> {
        self.change(|library| library.set_enabled(id, enabled))
    }

    /// Unlocks the Dictionary at `path` with the registration code and e-mail the reader
    /// entered (the code may be empty when the Dictionary carries its own). They are kept
    /// only when they work. Neither is ever logged or put into an error message.
    pub fn register(
        &self,
        path: &Path,
        email: &str,
        code: &str,
    ) -> Result<LibraryView, RegisterError> {
        let registration = Registration {
            email: email.trim().to_owned(),
            code: code.trim().to_owned(),
        };
        let mut guard = self.lock();
        let library = guard
            .as_mut()
            .map_err(|problem| RegisterError::Failed(problem.clone()))?;
        library
            .register(path, registration)
            .map_err(|error| match error {
                CoreError::WrongRegistration(_) => RegisterError::WrongCode,
                CoreError::RegistrationCodeNeeded(_) => RegisterError::CodeNeeded,
                other => RegisterError::Failed(other.to_string()),
            })?;
        Ok(view(&guard))
    }

    pub fn move_dictionary(&self, id: &str, direction: Direction) -> Result<LibraryView, String> {
        let step = match direction {
            Direction::Up => Move::Up,
            Direction::Down => Move::Down,
        };
        self.change(|library| library.move_dictionary(id, step))
    }
}

fn view(library: &Result<Library, String>) -> LibraryView {
    let library = match library {
        Ok(library) => library,
        Err(problem) => {
            return LibraryView {
                folders: Vec::new(),
                dictionaries: Vec::new(),
                failures: Vec::new(),
                problem: Some(problem.clone()),
            };
        }
    };
    LibraryView {
        folders: library
            .folders()
            .iter()
            .map(|folder| FolderView {
                path: folder.path.clone(),
                dictionary_count: folder.dictionary_count,
                problem: folder.problem.clone(),
            })
            .collect(),
        dictionaries: library
            .dictionaries()
            .map(|dictionary| DictionaryView {
                id: dictionary.id.clone(),
                title: dictionary.title.clone(),
                path: dictionary.path.clone(),
                entry_count: dictionary.entry_count,
                size: dictionary.size,
                enabled: dictionary.enabled,
            })
            .collect(),
        failures: library
            .failures()
            .iter()
            .map(|failure| FailureView {
                path: failure.path.clone(),
                reason: reason(&failure.reason),
            })
            .collect(),
        problem: None,
    }
}

fn word_books_view(books: &WordBooks) -> WordBooksView {
    WordBooksView {
        books: books
            .books()
            .iter()
            .map(|book| WordBookView {
                id: book.id.0,
                name: book.name.clone(),
                words: book
                    .words
                    .iter()
                    .map(|word| SavedWordView {
                        headword: word.headword.clone(),
                        dictionary_id: word.saved_from.dictionary_id.clone(),
                        dictionary_title: word.saved_from.dictionary_title.clone(),
                        note: word.note.clone(),
                        added: word.added.duration_since(UNIX_EPOCH).map_or(0, |since| {
                            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
                        }),
                    })
                    .collect(),
            })
            .collect(),
        current: books.current().map(|book| book.id.0),
    }
}

fn suggestion(item: Suggestion) -> SuggestionView {
    SuggestionView {
        headword: item.headword,
        count: item.count,
    }
}

fn reason(error: &OpenError) -> FailureReason {
    match error {
        OpenError::Unreadable(detail) => FailureReason::Unreadable(detail.clone()),
        OpenError::NotADictionary => FailureReason::NotADictionary,
        OpenError::UnsupportedVersion(version) => {
            FailureReason::UnsupportedVersion(version.clone())
        }
        OpenError::Damaged => FailureReason::Damaged,
        OpenError::NeedsRegistrationCode => FailureReason::NeedsRegistrationCode,
        OpenError::WrongRegistrationCode => FailureReason::WrongRegistrationCode,
        OpenError::Unsupported(feature) => FailureReason::Unsupported(feature.clone()),
        other => FailureReason::Unsupported(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use opendict_mdict_fixture::{Identity, MdxBuilder};
    use serde_json::json;

    use super::*;

    fn library_folder() -> tempfile::TempDir {
        let folder = tempfile::tempdir().unwrap();
        MdxBuilder::new()
            .title("Sample Dictionary")
            .entry("serendipity", "<p>a happy accident (sample)</p>")
            .write_to(folder.path().join("sample.mdx"))
            .unwrap();
        std::fs::write(folder.path().join("broken.mdx"), b"not a dictionary").unwrap();
        folder
    }

    const EMAIL: &str = "reader@example.com";
    const CODE: &str = "0123456789ABCDEF0123456789ABCDEF";

    #[test]
    fn a_dictionary_that_needs_a_code_opens_once_the_reader_enters_it() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let bought = folder.path().join("bought.mdx");
        MdxBuilder::new()
            .title("Bought")
            .entry("serendipity", "<p>a happy accident (sample)</p>")
            .registered_to(EMAIL, CODE, Identity::Utf16)
            .write_to(&bought)
            .unwrap();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());
        let view = serde_json::to_value(state.add_folder(folder.path()).unwrap()).unwrap();
        assert_eq!(
            view["failures"],
            json!([{ "path": bought, "reason": { "kind": "needsRegistrationCode" } }])
        );

        let wrong = state.register(&bought, EMAIL, "FEDCBA9876543210FEDCBA9876543210");
        assert_eq!(
            serde_json::to_value(wrong.unwrap_err()).unwrap(),
            json!({ "kind": "wrongCode" })
        );
        let no_code = state.register(&bought, EMAIL, "");
        assert_eq!(
            serde_json::to_value(no_code.unwrap_err()).unwrap(),
            json!({ "kind": "codeNeeded" })
        );

        let view = state.register(&bought, EMAIL, CODE).unwrap();

        assert_eq!(view.dictionaries[0].title, "Bought");
        assert!(view.failures.is_empty());
        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
        let kept = AppState::load(&data.path().join("library.json"), None, data.path());
        assert_eq!(kept.library().dictionaries.len(), 1, "kept for next time");
    }

    #[test]
    fn a_kept_code_that_no_longer_works_is_its_own_reason() {
        assert_eq!(
            serde_json::to_value(reason(&OpenError::WrongRegistrationCode)).unwrap(),
            json!({ "kind": "wrongRegistrationCode" })
        );
    }

    #[test]
    fn a_new_library_is_empty() {
        let data = tempfile::tempdir().unwrap();

        let state = AppState::load(&data.path().join("library.json"), None, data.path());

        assert_eq!(
            serde_json::to_value(state.library()).unwrap(),
            json!({ "folders": [], "dictionaries": [], "failures": [], "problem": null })
        );
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }

    #[test]
    fn an_added_folder_is_shown_and_answers_lookups() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());

        let view = serde_json::to_value(state.add_folder(folder.path()).unwrap()).unwrap();

        assert_eq!(
            view["folders"],
            json!([{ "path": folder.path(), "dictionaryCount": 2, "problem": null }])
        );
        let dictionary = &view["dictionaries"][0];
        assert_eq!(dictionary["title"], "Sample Dictionary");
        assert_eq!(dictionary["entryCount"], 1);
        assert_eq!(dictionary["enabled"], true);
        assert!(dictionary["size"].as_u64().unwrap() > 0);
        assert_eq!(
            view["failures"],
            json!([{ "path": folder.path().join("broken.mdx"), "reason": { "kind": "notADictionary" } }])
        );
        let id = dictionary["id"].as_str().unwrap();
        assert_eq!(
            serde_json::to_value(state.look_up("serendipity").unwrap()).unwrap(),
            json!([{
                "dictionaryId": id,
                "dictionaryTitle": "Sample Dictionary",
                "src": entry_protocol::entry_url(id, "serendipity"),
                "allowNetwork": false,
                "pronunciation": null
            }])
        );
    }

    #[test]
    fn suggestions_reach_the_interface_in_the_lookup_result_shape() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());
        state.add_folder(folder.path()).unwrap();

        assert_eq!(
            serde_json::to_value(state.suggest("ser", 50).unwrap()).unwrap(),
            json!({
                "items": [{ "headword": "serendipity", "count": 1 }],
                "lemma": null,
                "near": false,
                "miss": false,
                "browse": false
            })
        );
        assert_eq!(
            serde_json::to_value(state.headwords_after("apple", 50).unwrap()).unwrap(),
            json!([{ "headword": "serendipity", "count": 1 }])
        );
    }

    #[test]
    fn without_a_library_there_is_nothing_to_suggest() {
        let data = tempfile::tempdir().unwrap();

        let state = AppState::load(&data.path().join("library.json"), None, data.path());

        let suggestions = state.suggest("", 50).unwrap();
        assert!(suggestions.browse && suggestions.items.is_empty());
        assert_eq!(state.headwords_after("a", 50).unwrap(), []);
    }

    #[test]
    fn a_disabled_dictionary_no_longer_answers() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());
        let id = state.add_folder(folder.path()).unwrap().dictionaries[0]
            .id
            .clone();

        let view = state.set_enabled(&id, false).unwrap();

        assert!(!view.dictionaries[0].enabled);
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }

    #[test]
    fn the_interface_sends_directions_as_words() {
        assert_eq!(
            serde_json::from_value::<Direction>(json!("up")).unwrap(),
            Direction::Up
        );
        assert_eq!(
            serde_json::from_value::<Direction>(json!("down")).unwrap(),
            Direction::Down
        );
    }

    #[test]
    fn the_environment_override_is_a_library_of_that_folder_alone() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let file = data.path().join("library.json");

        let state = AppState::load(&file, Some(folder.path().to_owned()), data.path());
        state.rescan().unwrap();

        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
        assert!(!file.exists(), "the reader's Library file is not touched");
    }

    #[test]
    fn a_saved_word_is_kept_in_the_data_directory_with_where_it_came_from() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());

        let books = state
            .toggle_word(
                "serendipity",
                "0123",
                "Sample English Dictionary",
                "Word Book",
            )
            .unwrap();

        let book = &books.books[0];
        assert_eq!(books.current, Some(book.id));
        assert_eq!(book.name, "Word Book", "the first word makes a Word Book");
        let word = serde_json::to_value(&book.words[0]).unwrap();
        assert_eq!(word["headword"], "serendipity");
        assert_eq!(word["dictionaryId"], "0123");
        assert_eq!(word["dictionaryTitle"], "Sample English Dictionary");
        assert_eq!(word["note"], json!(null));
        assert!(
            word["added"].as_u64().unwrap() > 1_700_000_000_000,
            "milliseconds since 1970"
        );
        let restarted = AppState::load(&data.path().join("library.json"), None, data.path());
        assert_eq!(restarted.word_books().unwrap(), books);

        let books = state
            .toggle_word(
                "serendipity",
                "0123",
                "Sample English Dictionary",
                "Word Book",
            )
            .unwrap();
        assert!(
            books.books[0].words.is_empty(),
            "saving it again removes it"
        );
    }

    #[test]
    fn word_books_are_managed_and_exported_through_the_state() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());
        let first = state.create_word_book("Novel").unwrap().current.unwrap();
        let second = state.create_word_book("Paper").unwrap().current.unwrap();
        state.choose_word_book(first).unwrap();
        state
            .toggle_word("banter", "0123", "Sample", "unused")
            .unwrap();
        state
            .toggle_word("apple", "0123", "Sample", "unused")
            .unwrap();
        state
            .set_word_note(first, "banter", "at breakfast")
            .unwrap();
        state.remove_saved_word(first, "apple").unwrap();
        state.rename_word_book(first, "Middlemarch").unwrap();
        let books = state.delete_word_book(second).unwrap();

        assert_eq!(books.books.len(), 1);
        assert_eq!(books.books[0].name, "Middlemarch");
        assert_eq!(
            books.books[0].words[0].note.as_deref(),
            Some("at breakfast")
        );
        let file = data.path().join("Middlemarch.tsv");
        state
            .export_word_book(first, ExportFormat::Tsv, &file)
            .unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.starts_with("Headword\tNote\tDictionary\tAdded\nbanter\tat breakfast\tSample\t")
        );
    }

    #[test]
    fn a_damaged_word_books_file_is_reported_without_stopping_lookups() {
        let data = tempfile::tempdir().unwrap();
        std::fs::write(data.path().join("word-books.json"), "{ broken").unwrap();
        let folder = library_folder();

        let state = AppState::load(
            &data.path().join("library.json"),
            Some(folder.path().to_owned()),
            data.path(),
        );

        assert!(state.word_books().is_err());
        assert!(
            state
                .toggle_word("serendipity", "a", "A", "Word Book")
                .is_err()
        );
        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
    }

    #[test]
    fn the_interface_sends_export_formats_as_words() {
        assert_eq!(
            serde_json::from_value::<ExportFormat>(json!("csv")).unwrap(),
            ExportFormat::Csv
        );
        assert_eq!(
            serde_json::from_value::<ExportFormat>(json!("tsv")).unwrap(),
            ExportFormat::Tsv
        );
    }

    #[test]
    fn a_library_that_cannot_be_loaded_is_reported_not_fatal() {
        let data = tempfile::tempdir().unwrap();
        let file = data.path().join("library.json");
        std::fs::write(&file, "{ folders: ").unwrap();

        let state = AppState::load(&file, None, data.path());

        assert!(state.library().problem.is_some());
        assert!(state.rescan().is_err());
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }

    #[test]
    fn each_entry_says_how_the_headword_is_said_where_it_shows_that() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        MdxBuilder::new()
            .title("A Sample Learner's")
            .entry(
                "serif",
                r#"<span class="phon">/ˈserɪf/</span> a small line (sample)"#,
            )
            .write_to(folder.path().join("a-learner.mdx"))
            .unwrap();
        MdxBuilder::new()
            .title("B Sample Thesaurus")
            .entry("serif", "<p>see also: sans serif (sample)</p>")
            .write_to(folder.path().join("b-thesaurus.mdx"))
            .unwrap();
        let state = AppState::load(
            &data.path().join("library.json"),
            Some(folder.path().to_owned()),
            data.path(),
        );

        let said: Vec<Option<String>> = state
            .look_up("serif")
            .unwrap()
            .into_iter()
            .map(|entry| entry.pronunciation)
            .collect();

        assert_eq!(said, [Some("/ˈserɪf/".to_owned()), None]);
    }
}
