use std::path::PathBuf;

use opendict_core::{Config, Library};
use serde::Serialize;

/// What the shell holds between commands: the Library, or why there is none.
pub struct AppState {
    config_file: PathBuf,
    library: Result<Library, LibraryProblem>,
}

enum LibraryProblem {
    Unconfigured,
    Unavailable {
        folder: Option<PathBuf>,
        reason: String,
    },
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
    /// (from the environment) takes precedence over the configuration file.
    pub fn load(config_file: PathBuf, override_folder: Option<PathBuf>) -> AppState {
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
        AppState {
            config_file,
            library,
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
                dictionary_id: entry.dictionary_id,
                dictionary_title: entry.dictionary_title,
                html: entry.html,
            })
            .collect())
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

        let state = AppState::load(config_file, None);

        assert_eq!(
            serde_json::to_value(state.library_status()).unwrap(),
            json!({ "kind": "ready", "folder": folder.path(), "dictionaryCount": 1 })
        );
        assert_eq!(
            serde_json::to_value(state.look_up("serendipity").unwrap()).unwrap(),
            json!([{
                "dictionaryId": "sample.mdx",
                "dictionaryTitle": "Sample Dictionary",
                "html": "<p>a happy accident (sample)</p>"
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
        );

        assert_eq!(state.look_up("serendipity").unwrap().len(), 1);
    }

    #[test]
    fn without_a_library_folder_it_says_where_to_configure_one() {
        let config = tempfile::tempdir().unwrap();
        let config_file = config.path().join("config.json");

        let state = AppState::load(config_file.clone(), None);

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

        let state = AppState::load(config.path().join("config.json"), Some(missing.clone()));

        assert!(
            matches!(state.library_status(), LibraryStatus::Unavailable { folder: Some(f), .. } if f == missing)
        );
        assert_eq!(state.look_up("serendipity").unwrap(), []);
    }
}
