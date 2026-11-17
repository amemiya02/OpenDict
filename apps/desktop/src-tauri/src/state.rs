use std::path::PathBuf;
use std::sync::Mutex;

use opendict_core::{Config, Library, Preferences};
use serde::Serialize;

use crate::entry_protocol;

/// What the shell holds between commands: the Library, or why there is none, and the
/// reader's preferences.
pub struct AppState {
    config_file: PathBuf,
    library: Result<Library, LibraryProblem>,
    preferences: Mutex<Result<Preferences, String>>,
}

enum LibraryProblem {
    Unconfigured,
    Unavailable {
        folder: Option<PathBuf>,
        reason: String,
    },
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

/// Whether there is a Library to look words up in, as the interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LibraryStatus {
    /// No Library Folder is configured; `config_file` is where to set one.
    Unconfigured { config_file: PathBuf },
    /// The configuration or the Library Folder could not be read.
    Unavailable {
        folder: Option<PathBuf>,
        reason: String,
    },
    Ready {
        folder: PathBuf,
        dictionary_count: usize,
    },
}

impl AppState {
    /// Reads the configuration and opens the Library Folder it names. `override_folder`
    /// (from the environment) takes precedence over the configuration file. Preferences
    /// live in `data_dir`.
    pub fn load(
        config_file: PathBuf,
        override_folder: Option<PathBuf>,
        data_dir: PathBuf,
    ) -> AppState {
        let folder = match override_folder {
            Some(folder) => Ok(Some(folder)),
            None => Config::load(&config_file).map(|config| config.library_folder),
        };
        let library = match folder {
            Ok(Some(folder)) => {
                Library::open(&folder).map_err(|error| LibraryProblem::Unavailable {
                    folder: Some(folder),
                    reason: error.to_string(),
                })
            }
            Ok(None) => Err(LibraryProblem::Unconfigured),
            Err(error) => Err(LibraryProblem::Unavailable {
                folder: None,
                reason: error.to_string(),
            }),
        };
        let preferences =
            Preferences::load(&data_dir.join("preferences.json")).map_err(|e| e.to_string());
        AppState {
            config_file,
            library,
            preferences: Mutex::new(preferences),
        }
    }

    /// The Entries for exactly `headword`. Without a Library there are none.
    pub fn look_up(&self, headword: &str) -> Result<Vec<EntryView>, String> {
        let Ok(library) = &self.library else {
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

    /// The HTML of one Dictionary's Entry for exactly `headword`.
    pub fn entry_html(
        &self,
        dictionary_id: &str,
        headword: &str,
    ) -> Result<Option<String>, String> {
        let Ok(library) = &self.library else {
            return Ok(None);
        };
        library
            .entry(dictionary_id, headword)
            .map(|entry| entry.map(|e| e.html))
            .map_err(|error| error.to_string())
    }

    /// A file one Dictionary's Entries refer to.
    pub fn resource(&self, dictionary_id: &str, path: &str) -> Result<Option<Vec<u8>>, String> {
        let Ok(library) = &self.library else {
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

    pub fn library_status(&self) -> LibraryStatus {
        match &self.library {
            Ok(library) => LibraryStatus::Ready {
                folder: library.folder().to_owned(),
                dictionary_count: library.dictionaries().count(),
            },
            Err(LibraryProblem::Unconfigured) => LibraryStatus::Unconfigured {
                config_file: self.config_file.clone(),
            },
            Err(LibraryProblem::Unavailable { folder, reason }) => LibraryStatus::Unavailable {
                folder: folder.clone(),
                reason: reason.clone(),
            },
        }
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
        folder
    }

    #[test]
    fn looks_up_in_the_configured_library_folder() {
        let folder = library_folder();
        let config = tempfile::tempdir().unwrap();
        let config_file = config.path().join("config.json");
        std::fs::write(
            &config_file,
            json!({ "libraryFolder": folder.path() }).to_string(),
        )
        .unwrap();

        let state = AppState::load(config_file, None, config.path().join("data"));

        assert_eq!(
            serde_json::to_value(state.library_status()).unwrap(),
            json!({ "kind": "ready", "folder": folder.path(), "dictionaryCount": 1 })
        );
        assert_eq!(
            serde_json::to_value(state.look_up("serendipity").unwrap()).unwrap(),
            json!([{
                "dictionaryId": "sample.mdx",
                "dictionaryTitle": "Sample Dictionary",
                "src": entry_protocol::entry_url("sample.mdx", "serendipity"),
                "allowNetwork": false
            }])
        );
    }

    #[test]
    fn the_environment_override_wins_over_the_configuration_file() {
        let folder = library_folder();
        let config = tempfile::tempdir().unwrap();

        let state = AppState::load(
            config.path().join("config.json"),
            Some(folder.path().to_owned()),
            config.path().join("data"),
        );

        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
    }

    #[test]
    fn without_a_library_folder_it_says_where_to_configure_one() {
        let config = tempfile::tempdir().unwrap();
        let config_file = config.path().join("config.json");

        let state = AppState::load(config_file.clone(), None, config.path().join("data"));

        assert_eq!(
            state.library_status(),
            LibraryStatus::Unconfigured { config_file }
        );
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }

    #[test]
    fn a_missing_library_folder_is_reported_not_fatal() {
        let config = tempfile::tempdir().unwrap();
        let missing = config.path().join("no-such-folder");

        let state = AppState::load(
            config.path().join("config.json"),
            Some(missing.clone()),
            config.path().join("data"),
        );

        assert!(
            matches!(state.library_status(), LibraryStatus::Unavailable { folder: Some(f), .. } if f == missing)
        );
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }
}
