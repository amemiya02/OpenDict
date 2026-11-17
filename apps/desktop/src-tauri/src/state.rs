use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use opendict_core::{
    Error as CoreError, History, Library, Move, OpenError, Preferences, Registration, Suggestion,
};
use serde::{Deserialize, Serialize};

use crate::entry_protocol;

/// What the shell holds between commands: the Library, or why it could not be loaded, and
/// the reader's preferences and History, or why they could not be read.
pub struct AppState {
    library: Mutex<Result<Library, String>>,
    preferences: Mutex<Result<Preferences, String>>,
    history: Mutex<Result<History, String>>,
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
    /// and History live in `data_dir`.
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
        let history = History::load(&data_dir.join("history.json"));
        AppState {
            library: Mutex::new(library.map_err(|error| error.to_string())),
            preferences: Mutex::new(preferences.map_err(|error| error.to_string())),
            history: Mutex::new(history.map_err(|error| error.to_string())),
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

    /// The headwords of committed Lookups, newest first.
    pub fn history(&self) -> Result<Vec<String>, String> {
        let history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        match history.as_ref() {
            Ok(h) => Ok(h.headwords().to_vec()),
            Err(reason) => Err(reason.clone()),
        }
    }

    /// Records a committed Lookup of `headword`; returns History as it is afterwards.
    pub fn record_lookup(&self, headword: &str) -> Result<Vec<String>, String> {
        let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        let history = history.as_mut().map_err(|reason| reason.clone())?;
        history.record(headword).map_err(|e| e.to_string())?;
        Ok(history.headwords().to_vec())
    }

    pub fn clear_history(&self) -> Result<(), String> {
        let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        let history = history.as_mut().map_err(|reason| reason.clone())?;
        history.clear().map_err(|e| e.to_string())
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
                "allowNetwork": false
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
    fn committed_lookups_are_kept_in_history_in_the_data_directory() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());

        state.record_lookup("apple").unwrap();
        let history = state.record_lookup("serendipity").unwrap();

        assert_eq!(history, ["serendipity", "apple"]);
        let restarted = AppState::load(&data.path().join("library.json"), None, data.path());
        assert_eq!(restarted.history().unwrap(), ["serendipity", "apple"]);
        assert!(data.path().join("history.json").exists());
    }

    #[test]
    fn history_can_be_cleared() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(&data.path().join("library.json"), None, data.path());
        state.record_lookup("apple").unwrap();

        state.clear_history().unwrap();

        assert!(state.history().unwrap().is_empty());
    }

    #[test]
    fn a_damaged_history_is_reported_without_stopping_lookups() {
        let data = tempfile::tempdir().unwrap();
        std::fs::write(data.path().join("history.json"), "[ broken").unwrap();
        let folder = library_folder();

        let state = AppState::load(
            &data.path().join("library.json"),
            Some(folder.path().to_owned()),
            data.path(),
        );

        assert!(state.history().is_err());
        assert!(state.record_lookup("serendipity").is_err());
        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
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
}
