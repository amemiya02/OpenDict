use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub use opendict_core::Theme;
use opendict_core::{
    DataDirectory, Error as CoreError, ExportFormat, History, IndexState, Library, Move, OpenError,
    Preferences, Registration, SavedFrom, Suggestion, WordBookId, WordBooks,
};
use serde::{Deserialize, Serialize};

use crate::entry_protocol;

/// What the shell holds between commands: the data directory, the Library (or why it
/// could not be loaded), the reader's preferences, History and Word Books (or why they
/// could not be read), and the indexing that runs beside them. Cloning shares the same
/// state.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    /// The app's home folder, which says where the data directory is.
    home: PathBuf,
    /// The one Library Folder used instead of the reader's Library, if any.
    override_folder: Option<PathBuf>,
    /// The data directory, or where it should be and why it cannot be used. Locked
    /// before anything kept in it whenever both are needed.
    data: Mutex<Result<DataDirectory, DataDirectoryView>>,
    library: Mutex<Result<Library, String>>,
    /// What is being indexed right now, for the Library view.
    indexing: Mutex<Option<IndexingView>>,
    /// Whether an indexing thread is running. Changed only while `library` is locked, so
    /// a thread that finds no work cannot miss a Dictionary added meanwhile.
    indexing_running: AtomicBool,
    cancel: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
    preferences: Mutex<Result<Preferences, String>>,
    history: Mutex<Result<History, String>>,
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
    /// The first audio the Entry offers, as the path of its `sound://` link, for the
    /// Pronounce action. The interface plays it from the Dictionary's prefix at `src`.
    pub sound: Option<String>,
    /// How the headword is said, where the Entry shows that plainly. Plain text.
    pub pronunciation: Option<String>,
    /// In the dark theme, whether the Entry keeps its Dictionary's own white page rather
    /// than being recoloured.
    pub as_authored: bool,
}

/// How the page reads: the reader's theme, text size and zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceView {
    pub theme: Theme,
    /// In the dark theme, every Dictionary shows as authored unless the reader chose
    /// otherwise for it.
    pub all_as_authored: bool,
    /// The base text size of Entries, in CSS pixels.
    pub text_size: u32,
    /// Page zoom, in percent.
    pub zoom: u32,
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
    /// The Dictionary being indexed right now, if any.
    pub indexing: Option<IndexingView>,
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
    pub index: IndexView,
}

/// Whether a Dictionary's index is built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "camelCase")]
pub enum IndexView {
    Ready,
    Pending,
    Failed(String),
}

/// Indexing in progress: one Dictionary at a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingView {
    pub dictionary_id: String,
    pub title: String,
    /// Headwords read so far, out of `total`.
    pub done: usize,
    pub total: usize,
    /// Dictionaries waiting after this one.
    pub waiting: usize,
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

/// Where the reader's data is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirectoryView {
    pub path: PathBuf,
    /// Whether it is where a new reader's data starts.
    pub is_default: bool,
    /// Why it cannot be used, if it cannot (it may be on a drive that is not there).
    pub problem: Option<String>,
}

/// Why the data directory did not move, as a kind the interface words in the reader's
/// language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum DataDirectoryError {
    /// The folder already holds OpenDict data, which the reader may use instead.
    HoldsData(PathBuf),
    /// Anything else; the data stayed where it was.
    Failed(String),
}

/// A step in Library order, as the interface sends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Up,
    Down,
}

/// How often indexing progress is passed on while a Dictionary is being indexed.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

impl AppState {
    /// Loads the reader's data from the data directory that `home` (the app's folder
    /// for its data) says, which starts as `home` itself. With `override_folder` (from
    /// the environment, for tests and development) the Library is that one folder
    /// instead, kept only in memory, so the reader's own Library is never touched.
    pub fn load(home: &Path, override_folder: Option<PathBuf>) -> AppState {
        let data = located(home);
        let loaded = Loaded::from(&data, override_folder.as_deref());
        AppState {
            inner: Arc::new(Inner {
                home: home.to_owned(),
                override_folder,
                data: Mutex::new(data),
                library: Mutex::new(loaded.library),
                preferences: Mutex::new(loaded.preferences),
                history: Mutex::new(loaded.history),
                word_books: Mutex::new(loaded.word_books),
                indexing: Mutex::new(None),
                indexing_running: AtomicBool::new(false),
                cancel: AtomicBool::new(false),
                worker: Mutex::new(None),
            }),
        }
    }

    /// Where the reader's data is kept.
    pub fn data_directory(&self) -> DataDirectoryView {
        data_view(&self.inner.data.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Moves the reader's data into the folder `to` and uses it from then on. Indexing
    /// stops for the move; start it again afterwards. On failure the data stays, and
    /// is used, where it was.
    pub fn move_data_directory(&self, to: &Path) -> Result<DataDirectoryView, DataDirectoryError> {
        self.reload_data(|data| {
            let data = data.as_mut().map_err(|view| {
                DataDirectoryError::Failed(view.problem.clone().unwrap_or_default())
            })?;
            data.relocate(to, &mut |_| ControlFlow::Continue(()))
                .map(|_| ())
                .map_err(|error| match error {
                    CoreError::DataDirectoryHoldsData(path) => DataDirectoryError::HoldsData(path),
                    other => DataDirectoryError::Failed(other.to_string()),
                })
        })
    }

    /// Uses the data already in `folder` (starting afresh if there is none) from then
    /// on; the data used until now stays where it is. Indexing stops for the switch;
    /// start it again afterwards.
    pub fn use_data_directory(
        &self,
        folder: &Path,
    ) -> Result<DataDirectoryView, DataDirectoryError> {
        let home = self.inner.home.clone();
        self.reload_data(|data| {
            *data = Ok(DataDirectory::use_folder(&home, folder)
                .map_err(|error| DataDirectoryError::Failed(error.to_string()))?);
            Ok(())
        })
    }

    /// Lets go of every file in the data directory, applies `change` to it, and loads
    /// the reader's data again from wherever it is afterwards.
    fn reload_data(
        &self,
        change: impl FnOnce(
            &mut Result<DataDirectory, DataDirectoryView>,
        ) -> Result<(), DataDirectoryError>,
    ) -> Result<DataDirectoryView, DataDirectoryError> {
        self.cancel_indexing();
        self.wait_for_indexing();
        let inner = &*self.inner;
        let mut data = inner.data.lock().unwrap_or_else(|e| e.into_inner());
        let mut library = self.lock();
        let mut preferences = inner.preferences.lock().unwrap_or_else(|e| e.into_inner());
        let mut history = inner.history.lock().unwrap_or_else(|e| e.into_inner());
        let mut word_books = inner.word_books.lock().unwrap_or_else(|e| e.into_inner());
        // Every file is let go first: the Library maps its index files, which could
        // then be neither copied safely nor removed.
        let busy = "the data directory is moving".to_owned();
        *library = Err(busy.clone());
        *preferences = Err(busy.clone());
        *history = Err(busy.clone());
        *word_books = Err(busy);

        let changed = change(&mut data);

        let loaded = Loaded::from(&data, inner.override_folder.as_deref());
        *library = loaded.library;
        *preferences = loaded.preferences;
        *history = loaded.history;
        *word_books = loaded.word_books;
        changed.map(|()| data_view(&data))
    }

    fn lock(&self) -> MutexGuard<'_, Result<Library, String>> {
        // A panic while holding the lock leaves the Library as it was; keep serving it.
        self.inner
            .library
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn progress(&self) -> Option<IndexingView> {
        self.inner
            .indexing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn set_progress(&self, progress: Option<IndexingView>) {
        *self
            .inner
            .indexing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = progress;
    }

    /// Applies `change` to the Library and returns the Library as it is afterwards.
    fn change(
        &self,
        change: impl FnOnce(&mut Library) -> opendict_core::Result<()>,
    ) -> Result<LibraryView, String> {
        let mut guard = self.lock();
        let library = guard.as_mut().map_err(|problem| problem.clone())?;
        change(library).map_err(|error| error.to_string())?;
        Ok(view(&guard, self.progress()))
    }

    /// The Entries for exactly `headword`, from enabled, indexed Dictionaries in Library
    /// order. Without a Library there are none.
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
                as_authored: self.as_authored(&entry.dictionary_id),
                sound: entry.sounds().into_iter().next(),
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

    /// Up to `limit` more suggestions for `query` after `headword`: the headword column,
    /// continued.
    pub fn headwords_after(
        &self,
        query: &str,
        headword: &str,
        limit: usize,
    ) -> Result<Vec<SuggestionView>, String> {
        let guard = self.lock();
        let Ok(library) = &*guard else {
            return Ok(Vec::new());
        };
        let items = library
            .suggest_after(query, headword, limit)
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
        let preferences = self
            .inner
            .preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        preferences
            .as_ref()
            .is_ok_and(|p| p.allows_network(dictionary_id))
    }

    pub fn set_allow_network(&self, dictionary_id: &str, allow: bool) -> Result<(), String> {
        self.change_preferences(|p| p.set_allows_network(dictionary_id, allow))
    }

    /// The reading preferences. If the preferences cannot be read, the defaults.
    pub fn appearance(&self) -> AppearanceView {
        let preferences = self
            .inner
            .preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match preferences.as_ref() {
            Ok(p) => AppearanceView {
                theme: p.theme(),
                all_as_authored: p.all_as_authored(),
                text_size: p.text_size(),
                zoom: p.zoom(),
            },
            Err(_) => AppearanceView {
                theme: Theme::System,
                all_as_authored: false,
                text_size: opendict_core::DEFAULT_TEXT_SIZE,
                zoom: opendict_core::DEFAULT_ZOOM,
            },
        }
    }

    /// Whether, in the dark theme, the Dictionary shows as authored. If the preferences
    /// cannot be read, every Dictionary is recoloured.
    pub fn as_authored(&self, dictionary_id: &str) -> bool {
        let preferences = self
            .inner
            .preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        preferences
            .as_ref()
            .is_ok_and(|p| p.as_authored(dictionary_id))
    }

    pub fn set_as_authored(&self, dictionary_id: &str, as_authored: bool) -> Result<(), String> {
        self.change_preferences(|p| p.set_as_authored(dictionary_id, as_authored))
    }

    pub fn set_all_as_authored(&self, as_authored: bool) -> Result<(), String> {
        self.change_preferences(|p| p.set_all_as_authored(as_authored))
    }

    pub fn set_theme(&self, theme: Theme) -> Result<(), String> {
        self.change_preferences(|p| p.set_theme(theme))
    }

    /// Returns the size as set, within readable limits.
    pub fn set_text_size(&self, pixels: u32) -> Result<u32, String> {
        self.change_preferences(|p| p.set_text_size(pixels))
    }

    /// Returns the zoom as set, within limits.
    pub fn set_zoom(&self, percent: u32) -> Result<u32, String> {
        self.change_preferences(|p| p.set_zoom(percent))
    }

    fn change_preferences<T>(
        &self,
        change: impl FnOnce(&mut Preferences) -> opendict_core::Result<T>,
    ) -> Result<T, String> {
        let mut preferences = self
            .inner
            .preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match preferences.as_mut() {
            Ok(p) => change(p).map_err(|e| e.to_string()),
            Err(reason) => Err(reason.clone()),
        }
    }

    /// The headwords of committed Lookups, newest first.
    pub fn history(&self) -> Result<Vec<String>, String> {
        let history = self.inner.history.lock().unwrap_or_else(|e| e.into_inner());
        match history.as_ref() {
            Ok(h) => Ok(h.headwords().to_vec()),
            Err(reason) => Err(reason.clone()),
        }
    }

    /// Records a committed Lookup of `headword`; returns History as it is afterwards.
    pub fn record_lookup(&self, headword: &str) -> Result<Vec<String>, String> {
        let mut history = self.inner.history.lock().unwrap_or_else(|e| e.into_inner());
        let history = history.as_mut().map_err(|reason| reason.clone())?;
        history.record(headword).map_err(|e| e.to_string())?;
        Ok(history.headwords().to_vec())
    }

    pub fn clear_history(&self) -> Result<(), String> {
        let mut history = self.inner.history.lock().unwrap_or_else(|e| e.into_inner());
        let history = history.as_mut().map_err(|reason| reason.clone())?;
        history.clear().map_err(|e| e.to_string())
    }

    /// Applies `change` to the Word Books and returns them as they are afterwards. A
    /// Word Books file that could not be read is reported, never overwritten.
    fn change_word_books(
        &self,
        change: impl FnOnce(&mut WordBooks) -> opendict_core::Result<()>,
    ) -> Result<WordBooksView, String> {
        let mut guard = self
            .inner
            .word_books
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let guard = self
            .inner
            .word_books
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let books = guard.as_ref().map_err(|problem| problem.clone())?;
        books
            .export(WordBookId(id), format, file)
            .map_err(|error| error.to_string())
    }

    /// The Word Book's name, to suggest a name for the file it is exported to.
    pub fn word_book_name(&self, id: u64) -> Option<String> {
        let guard = self
            .inner
            .word_books
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let books = guard.as_ref().ok()?;
        books.book(WordBookId(id)).map(|book| book.name.clone())
    }

    pub fn library(&self) -> LibraryView {
        view(&self.lock(), self.progress())
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
        Ok(view(&guard, self.progress()))
    }

    pub fn move_dictionary(&self, id: &str, direction: Direction) -> Result<LibraryView, String> {
        let step = match direction {
            Direction::Up => Move::Up,
            Direction::Down => Move::Down,
        };
        self.change(|library| library.move_dictionary(id, step))
    }

    /// Indexes every Dictionary that needs it, one at a time on a background thread, so
    /// Lookups go on meanwhile. `notify` receives the Library view whenever indexing
    /// progresses or a Dictionary's index is done. Does nothing new if indexing is
    /// already running: that run picks up Dictionaries added since.
    pub fn start_indexing(&self, notify: impl Fn(LibraryView) + Send + 'static) {
        {
            let _library = self.lock();
            if self.inner.indexing_running.swap(true, Ordering::SeqCst) {
                return;
            }
            self.inner.cancel.store(false, Ordering::SeqCst);
        }
        let state = self.clone();
        let worker = std::thread::spawn(move || state.index(&notify));
        let previous = self
            .inner
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .replace(worker);
        // A previous run has already ended (it cleared `indexing_running`); reap it.
        if let Some(previous) = previous {
            let _ = previous.join();
        }
    }

    /// Stops indexing after the Dictionary being read. Its index and those still
    /// waiting stay pending until indexing is started again.
    pub fn cancel_indexing(&self) {
        self.inner.cancel.store(true, Ordering::SeqCst);
    }

    /// Waits until the indexing run started last has ended.
    pub fn wait_for_indexing(&self) {
        let worker = self
            .inner
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
    }

    /// The indexing thread: takes the first pending Dictionary, indexes it without
    /// holding the Library, puts the index in place, and repeats until none is left or
    /// the reader cancels.
    fn index(&self, notify: &dyn Fn(LibraryView)) {
        loop {
            let (job, waiting) = {
                let mut guard = self.lock();
                let jobs = match guard.as_mut() {
                    Ok(library) if !self.inner.cancel.load(Ordering::SeqCst) => {
                        library.index_jobs()
                    }
                    _ => Vec::new(),
                };
                let waiting = jobs.len().saturating_sub(1);
                match jobs.into_iter().next() {
                    Some(job) => (job, waiting),
                    None => {
                        self.inner.indexing_running.store(false, Ordering::SeqCst);
                        drop(guard);
                        self.set_progress(None);
                        notify(self.library());
                        return;
                    }
                }
            };

            let mut progress = IndexingView {
                dictionary_id: job.dictionary_id().to_owned(),
                title: job.title().to_owned(),
                done: 0,
                total: job.entry_count(),
                waiting,
            };
            self.set_progress(Some(progress.clone()));
            notify(self.library());
            let mut reported = Instant::now();
            let outcome = job.run(&mut |done| {
                if self.inner.cancel.load(Ordering::SeqCst) {
                    return ControlFlow::Break(());
                }
                progress.done = done;
                if reported.elapsed() >= PROGRESS_INTERVAL {
                    reported = Instant::now();
                    self.set_progress(Some(progress.clone()));
                    notify(self.library());
                }
                ControlFlow::Continue(())
            });

            if let Ok(library) = self.lock().as_mut() {
                library.finish_index(outcome);
            }
        }
    }
}

/// The data directory `home` says, or where it should be and why it cannot be used.
fn located(home: &Path) -> Result<DataDirectory, DataDirectoryView> {
    DataDirectory::locate(home).map_err(|error| DataDirectoryView {
        path: match &error {
            CoreError::DataDirectoryMissing(path) => path.clone(),
            _ => home.to_owned(),
        },
        is_default: false,
        problem: Some(error.to_string()),
    })
}

fn data_view(data: &Result<DataDirectory, DataDirectoryView>) -> DataDirectoryView {
    match data {
        Ok(data) => DataDirectoryView {
            path: data.path().to_owned(),
            is_default: data.is_default(),
            problem: None,
        },
        Err(view) => view.clone(),
    }
}

/// The reader's data as read from the data directory. Without a data directory, each
/// part says why it is missing, and nothing is written anywhere else.
struct Loaded {
    library: Result<Library, String>,
    preferences: Result<Preferences, String>,
    history: Result<History, String>,
    word_books: Result<WordBooks, String>,
}

impl Loaded {
    fn from(
        data: &Result<DataDirectory, DataDirectoryView>,
        override_folder: Option<&Path>,
    ) -> Loaded {
        let data = match data {
            Ok(data) => data,
            Err(view) => {
                let problem = view.problem.clone().unwrap_or_default();
                return Loaded {
                    library: Err(problem.clone()),
                    preferences: Err(problem.clone()),
                    history: Err(problem.clone()),
                    word_books: Err(problem),
                };
            }
        };
        let library = match override_folder {
            Some(folder) => Library::open(folder),
            None => Library::load(&data.library_file()),
        };
        Loaded {
            library: library.map_err(|error| error.to_string()),
            preferences: Preferences::load(&data.preferences_file()).map_err(|e| e.to_string()),
            history: History::load(&data.history_file()).map_err(|e| e.to_string()),
            word_books: WordBooks::load(&data.word_books_file()).map_err(|e| e.to_string()),
        }
    }
}

fn view(library: &Result<Library, String>, indexing: Option<IndexingView>) -> LibraryView {
    let library = match library {
        Ok(library) => library,
        Err(problem) => {
            return LibraryView {
                folders: Vec::new(),
                dictionaries: Vec::new(),
                failures: Vec::new(),
                indexing: None,
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
                index: match &dictionary.index {
                    IndexState::Ready => IndexView::Ready,
                    IndexState::Pending => IndexView::Pending,
                    IndexState::Failed(reason) => IndexView::Failed(reason.clone()),
                },
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
        indexing,
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
    use opendict_mdict_fixture::{Damage, Identity, MdxBuilder};
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
        let state = AppState::load(data.path(), None);
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
        index(&state);

        assert_eq!(view.dictionaries[0].title, "Bought");
        assert!(view.failures.is_empty());
        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
        let kept = AppState::load(data.path(), None);
        assert_eq!(kept.library().dictionaries.len(), 1, "kept for next time");
    }

    #[test]
    fn a_kept_code_that_no_longer_works_is_its_own_reason() {
        assert_eq!(
            serde_json::to_value(reason(&OpenError::WrongRegistrationCode)).unwrap(),
            json!({ "kind": "wrongRegistrationCode" })
        );
    }

    /// Indexes everything pending and waits until it is done.
    fn index(state: &AppState) {
        state.start_indexing(|_| {});
        state.wait_for_indexing();
    }

    #[test]
    fn a_new_library_is_empty() {
        let data = tempfile::tempdir().unwrap();

        let state = AppState::load(data.path(), None);

        assert_eq!(
            serde_json::to_value(state.library()).unwrap(),
            json!({
                "folders": [], "dictionaries": [], "failures": [], "indexing": null, "problem": null
            })
        );
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }

    #[test]
    fn an_added_folder_is_shown_and_answers_lookups_once_indexed() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(data.path(), None);

        let view = serde_json::to_value(state.add_folder(folder.path()).unwrap()).unwrap();

        assert_eq!(
            view["folders"],
            json!([{ "path": folder.path(), "dictionaryCount": 2, "problem": null }])
        );
        let dictionary = &view["dictionaries"][0];
        assert_eq!(dictionary["title"], "Sample Dictionary");
        assert_eq!(dictionary["entryCount"], 1);
        assert_eq!(dictionary["enabled"], true);
        assert_eq!(dictionary["index"], json!({ "state": "pending" }));
        assert!(dictionary["size"].as_u64().unwrap() > 0);
        assert_eq!(
            view["failures"],
            json!([{ "path": folder.path().join("broken.mdx"), "reason": { "kind": "notADictionary" } }])
        );

        index(&state);

        let view = serde_json::to_value(state.library()).unwrap();
        assert_eq!(
            view["dictionaries"][0]["index"],
            json!({ "state": "ready" })
        );
        assert_eq!(view["indexing"], json!(null));
        let id = view["dictionaries"][0]["id"].as_str().unwrap();
        assert_eq!(
            serde_json::to_value(state.look_up("serendipity").unwrap()).unwrap(),
            json!([{
                "dictionaryId": id,
                "dictionaryTitle": "Sample Dictionary",
                "src": entry_protocol::entry_url(id, "serendipity"),
                "allowNetwork": false,
                "sound": null,
                "pronunciation": null,
                "asAuthored": false
            }])
        );
    }

    #[test]
    fn indexing_reports_each_dictionary_and_ends_with_nothing_in_progress() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(data.path(), None);
        state.add_folder(folder.path()).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));

        let record = Arc::clone(&seen);
        state.start_indexing(move |view| record.lock().unwrap().push(view.indexing));
        state.wait_for_indexing();

        let seen = seen.lock().unwrap();
        let first = seen[0]
            .as_ref()
            .expect("the first report names the Dictionary");
        assert_eq!(first.title, "Sample Dictionary");
        assert_eq!((first.done, first.total, first.waiting), (0, 1, 0));
        assert_eq!(seen.last(), Some(&None));
    }

    #[test]
    fn cancelled_indexing_leaves_dictionaries_pending_until_started_again() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(data.path(), None);
        state.add_folder(folder.path()).unwrap();

        // The reader cancels as soon as the first progress report arrives.
        let canceller = state.clone();
        state.start_indexing(move |view| {
            if view.indexing.is_some() {
                canceller.cancel_indexing();
            }
        });
        state.wait_for_indexing();

        assert_eq!(state.library().dictionaries[0].index, IndexView::Pending);
        assert_eq!(state.library().indexing, None);
        assert_eq!(state.look_up("serendipity").unwrap(), []);

        index(&state);

        assert_eq!(state.library().dictionaries[0].index, IndexView::Ready);
        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
    }

    #[test]
    fn a_patterns_column_continues_with_its_matches_only() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(data.path(), None);
        state.add_folder(folder.path()).unwrap();
        index(&state);

        assert_eq!(state.headwords_after("a*", "apple", 50).unwrap(), []);
        assert_eq!(
            serde_json::to_value(state.headwords_after("s*", "s", 50).unwrap()).unwrap(),
            json!([{ "headword": "serendipity", "count": 1 }])
        );
    }

    #[test]
    fn a_dictionary_that_fails_to_index_is_shown_with_its_reason() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        std::fs::write(
            folder.path().join("damaged.mdx"),
            MdxBuilder::new()
                .title("Damaged")
                .entry("apple", "<p>a fruit (sample)</p>")
                .build_damaged(Damage::KeyBlock),
        )
        .unwrap();
        let state = AppState::load(data.path(), None);
        state.add_folder(folder.path()).unwrap();

        index(&state);

        let view = serde_json::to_value(state.library()).unwrap();
        let states: Vec<_> = view["dictionaries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| (d["title"].clone(), d["index"]["state"].clone()))
            .collect();
        assert_eq!(
            states,
            [
                (json!("Damaged"), json!("failed")),
                (json!("Sample Dictionary"), json!("ready")),
            ]
        );
        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
    }

    #[test]
    fn a_disabled_dictionary_no_longer_answers() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(data.path(), None);
        let id = state.add_folder(folder.path()).unwrap().dictionaries[0]
            .id
            .clone();
        index(&state);

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

        let state = AppState::load(data.path(), Some(folder.path().to_owned()));
        state.rescan().unwrap();

        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
        assert!(
            !data.path().join("library.json").exists(),
            "the reader's Library file is not touched"
        );
        assert!(
            !data.path().join("indexes").exists(),
            "nor is anything stored beside it"
        );
    }

    #[test]
    fn committed_lookups_are_kept_in_history_in_the_data_directory() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(data.path(), None);

        state.record_lookup("apple").unwrap();
        let history = state.record_lookup("serendipity").unwrap();

        assert_eq!(history, ["serendipity", "apple"]);
        let restarted = AppState::load(data.path(), None);
        assert_eq!(restarted.history().unwrap(), ["serendipity", "apple"]);
        assert!(data.path().join("history.json").exists());
    }

    #[test]
    fn history_can_be_cleared() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(data.path(), None);
        state.record_lookup("apple").unwrap();

        state.clear_history().unwrap();

        assert!(state.history().unwrap().is_empty());
    }

    #[test]
    fn a_damaged_history_is_reported_without_stopping_lookups() {
        let data = tempfile::tempdir().unwrap();
        std::fs::write(data.path().join("history.json"), "[ broken").unwrap();
        let folder = library_folder();

        let state = AppState::load(data.path(), Some(folder.path().to_owned()));

        assert!(state.history().is_err());
        assert!(state.record_lookup("serendipity").is_err());
        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
    }

    #[test]
    fn a_saved_word_is_kept_in_the_data_directory_with_where_it_came_from() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(data.path(), None);

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
        let restarted = AppState::load(data.path(), None);
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
        let state = AppState::load(data.path(), None);
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

        let state = AppState::load(data.path(), Some(folder.path().to_owned()));

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

        let state = AppState::load(data.path(), None);
        index(&state);

        assert!(state.library().problem.is_some());
        assert!(state.rescan().is_err());
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }

    #[test]
    fn suggestions_reach_the_interface_in_the_lookup_result_shape() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(data.path(), None);
        state.add_folder(folder.path()).unwrap();
        index(&state);

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
            serde_json::to_value(state.suggest("serendipities", 50).unwrap()).unwrap()["lemma"],
            json!({ "from": "serendipities", "to": "serendipity" })
        );
        assert_eq!(
            serde_json::to_value(state.headwords_after("", "apple", 50).unwrap()).unwrap(),
            json!([{ "headword": "serendipity", "count": 1 }])
        );
    }

    #[test]
    fn without_a_library_there_is_nothing_to_suggest() {
        let data = tempfile::tempdir().unwrap();

        let state = AppState::load(data.path(), None);

        let suggestions = state.suggest("", 50).unwrap();
        assert!(suggestions.browse && suggestions.items.is_empty());
        assert_eq!(state.headwords_after("", "a", 50).unwrap(), []);
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
        let state = AppState::load(data.path(), Some(folder.path().to_owned()));

        let said: Vec<Option<String>> = state
            .look_up("serif")
            .unwrap()
            .into_iter()
            .map(|entry| entry.pronunciation)
            .collect();

        assert_eq!(said, [Some("/ˈserɪf/".to_owned()), None]);
    }

    #[test]
    fn the_reading_preferences_are_kept_in_the_data_directory() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(data.path(), None);
        assert_eq!(
            state.appearance(),
            AppearanceView {
                theme: Theme::System,
                all_as_authored: false,
                text_size: 16,
                zoom: 100
            }
        );

        state.set_theme(Theme::Dark).unwrap();
        state.set_all_as_authored(true).unwrap();
        assert_eq!(state.set_text_size(99).unwrap(), 32);
        assert_eq!(state.set_zoom(150).unwrap(), 150);

        let again = AppState::load(data.path(), None);
        assert_eq!(
            again.appearance(),
            AppearanceView {
                theme: Theme::Dark,
                all_as_authored: true,
                text_size: 32,
                zoom: 150
            }
        );
    }

    #[test]
    fn the_interface_sends_and_receives_themes_as_words() {
        assert_eq!(
            serde_json::to_value(Theme::System).unwrap(),
            json!("system")
        );
        assert_eq!(
            serde_json::from_value::<Theme>(json!("dark")).unwrap(),
            Theme::Dark
        );
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(data.path(), None);
        assert_eq!(
            serde_json::to_value(state.appearance()).unwrap(),
            json!({"theme": "system", "allAsAuthored": false, "textSize": 16, "zoom": 100})
        );
    }

    /// A reader's data: an indexed Library, a Lookup in History, a saved word and a theme.
    fn filled(home: &Path, folder: &Path) -> AppState {
        let state = AppState::load(home, None);
        state.add_folder(folder).unwrap();
        index(&state);
        state.record_lookup("serendipity").unwrap();
        let id = state.library().dictionaries[0].id.clone();
        state
            .toggle_word("serendipity", &id, "Sample Dictionary", "Word Book")
            .unwrap();
        state.set_theme(Theme::Dark).unwrap();
        state
    }

    /// What the reader sees of their data through `state`.
    fn seen(state: &AppState) -> (usize, Vec<String>, usize, Theme) {
        (
            state.look_up("serendipity").unwrap().len(),
            state.history().unwrap(),
            state.word_books().unwrap().books[0].words.len(),
            state.appearance().theme,
        )
    }

    fn reader_data() -> (usize, Vec<String>, usize, Theme) {
        (1, vec!["serendipity".to_owned()], 1, Theme::Dark)
    }

    #[test]
    fn the_data_directory_moves_with_everything_in_it_and_is_used_from_then_on() {
        let home = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let synced = tempfile::tempdir().unwrap();
        let to = synced.path().join("OpenDict");
        let state = filled(home.path(), folder.path());
        assert_eq!(
            serde_json::to_value(state.data_directory()).unwrap(),
            json!({ "path": home.path(), "isDefault": true, "problem": null })
        );

        let view = state.move_data_directory(&to).unwrap();

        assert_eq!(
            serde_json::to_value(view).unwrap(),
            json!({ "path": to, "isDefault": false, "problem": null })
        );
        assert_eq!(seen(&state), reader_data());
        assert!(!home.path().join("library.json").exists());
        assert!(!home.path().join("history.json").exists());
        let restarted = AppState::load(home.path(), None);
        assert_eq!(restarted.data_directory().path, to);
        assert_eq!(seen(&restarted), reader_data());
        assert_eq!(
            restarted.library().dictionaries[0].index,
            IndexView::Ready,
            "the indexes moved too"
        );
    }

    #[test]
    fn the_data_moves_while_a_dictionary_is_being_indexed() {
        let home = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let synced = tempfile::tempdir().unwrap();
        let state = AppState::load(home.path(), None);
        state.add_folder(folder.path()).unwrap();
        state.start_indexing(|_| {});

        state.move_data_directory(synced.path()).unwrap();
        index(&state);

        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
        let restarted = AppState::load(home.path(), None);
        assert_eq!(restarted.data_directory().path, synced.path());
        assert_eq!(restarted.look_up("serendipity").unwrap().len(), 1);
    }

    #[test]
    fn a_folder_holding_other_data_is_offered_for_use_instead() {
        let folder = library_folder();
        let synced = tempfile::tempdir().unwrap();
        let other_home = tempfile::tempdir().unwrap();
        let other = filled(other_home.path(), folder.path());
        other.move_data_directory(synced.path()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let state = AppState::load(home.path(), None);
        state.record_lookup("own").unwrap();

        let refused = state.move_data_directory(synced.path()).unwrap_err();

        assert_eq!(
            serde_json::to_value(refused).unwrap(),
            json!({ "kind": "holdsData", "detail": synced.path() })
        );
        assert_eq!(state.history().unwrap(), ["own"]);
        let view = state.use_data_directory(synced.path()).unwrap();
        assert_eq!(view.path, synced.path());
        assert_eq!(seen(&state), reader_data());
        assert_eq!(seen(&AppState::load(home.path(), None)), reader_data());
    }

    #[test]
    fn a_data_directory_that_has_gone_is_reported_and_another_can_be_chosen() {
        let home = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let synced = tempfile::tempdir().unwrap();
        let to = synced.path().join("OpenDict");
        filled(home.path(), folder.path())
            .move_data_directory(&to)
            .unwrap();
        std::fs::rename(&to, synced.path().join("Unplugged")).unwrap();

        let state = AppState::load(home.path(), None);

        let view = state.data_directory();
        assert_eq!(view.path, to);
        assert!(view.problem.is_some());
        assert!(state.library().problem.is_some());
        assert!(state.history().is_err());
        assert!(
            state.record_lookup("lost").is_err(),
            "nothing is written elsewhere"
        );
        let view = state
            .use_data_directory(&synced.path().join("Unplugged"))
            .unwrap();
        assert_eq!(view.problem, None);
        assert_eq!(seen(&state), reader_data());
    }

    #[test]
    fn a_move_that_fails_keeps_the_data_in_use_where_it_was() {
        let home = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = filled(home.path(), folder.path());

        let error = state
            .move_data_directory(&home.path().join("indexes").join("inside"))
            .unwrap_err();

        assert!(matches!(error, DataDirectoryError::Failed(_)), "{error:?}");
        assert_eq!(state.data_directory().path, home.path());
        assert_eq!(seen(&state), reader_data());
    }
}
