use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use opendict_core::{Library, Move, OpenError};
use serde::{Deserialize, Serialize};

/// What the shell holds between commands: the Library, or why it could not be loaded.
pub struct AppState {
    library: Mutex<Result<Library, String>>,
}

/// An Entry as the interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub dictionary_id: String,
    pub dictionary_title: String,
    /// Untrusted HTML; the interface renders it only inside a sandboxed frame.
    pub html: String,
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
    Unsupported(String),
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
    /// kept only in memory, so the reader's own Library is never touched.
    pub fn load(library_file: &Path, override_folder: Option<PathBuf>) -> AppState {
        let library = match override_folder {
            Some(folder) => Library::open(&folder),
            None => Library::load(library_file),
        };
        AppState {
            library: Mutex::new(library.map_err(|error| error.to_string())),
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
                dictionary_id: entry.dictionary_id,
                dictionary_title: entry.dictionary_title,
                html: entry.html,
            })
            .collect())
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

fn reason(error: &OpenError) -> FailureReason {
    match error {
        OpenError::Unreadable(detail) => FailureReason::Unreadable(detail.clone()),
        OpenError::NotADictionary => FailureReason::NotADictionary,
        OpenError::UnsupportedVersion(version) => {
            FailureReason::UnsupportedVersion(version.clone())
        }
        OpenError::Damaged => FailureReason::Damaged,
        OpenError::NeedsRegistrationCode => FailureReason::NeedsRegistrationCode,
        OpenError::Unsupported(feature) => FailureReason::Unsupported(feature.clone()),
        other => FailureReason::Unsupported(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use opendict_mdict_fixture::MdxBuilder;
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

    #[test]
    fn a_new_library_is_empty() {
        let data = tempfile::tempdir().unwrap();

        let state = AppState::load(&data.path().join("library.json"), None);

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
        let state = AppState::load(&data.path().join("library.json"), None);

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
                "html": "<p>a happy accident (sample)</p>"
            }])
        );
    }

    #[test]
    fn a_disabled_dictionary_no_longer_answers() {
        let data = tempfile::tempdir().unwrap();
        let folder = library_folder();
        let state = AppState::load(&data.path().join("library.json"), None);
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

        let state = AppState::load(&file, Some(folder.path().to_owned()));
        state.rescan().unwrap();

        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
        assert!(!file.exists(), "the reader's Library file is not touched");
    }

    #[test]
    fn a_library_that_cannot_be_loaded_is_reported_not_fatal() {
        let data = tempfile::tempdir().unwrap();
        let file = data.path().join("library.json");
        std::fs::write(&file, "{ folders: ").unwrap();

        let state = AppState::load(&file, None);

        assert!(state.library().problem.is_some());
        assert!(state.rescan().is_err());
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }
}
