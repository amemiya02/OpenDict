use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The reader's per-Dictionary preferences, kept in one JSON file. Every change is
/// written straight away.
#[derive(Debug, Clone)]
pub struct Preferences {
    file: PathBuf,
    stored: Stored,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    /// Keyed by Dictionary id.
    #[serde(default)]
    dictionaries: BTreeMap<String, DictionaryPreferences>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryPreferences {
    /// Whether the Dictionary's Entries may load from or talk to the network. Off unless
    /// the reader turns it on.
    #[serde(default)]
    allow_network: bool,
}

impl Preferences {
    /// Reads the preferences from `file`. A missing file means every default.
    pub fn load(file: &Path) -> Result<Preferences> {
        let stored = match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| problem(file, error))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Stored::default(),
            Err(error) => return Err(problem(file, error)),
        };
        Ok(Preferences {
            file: file.to_owned(),
            stored,
        })
    }

    /// Whether the Dictionary with `dictionary_id` may use the network.
    pub fn allows_network(&self, dictionary_id: &str) -> bool {
        self.stored
            .dictionaries
            .get(dictionary_id)
            .is_some_and(|d| d.allow_network)
    }

    /// Allows or forbids network access for one Dictionary, and saves.
    pub fn set_allows_network(&mut self, dictionary_id: &str, allow: bool) -> Result<()> {
        let entry = self
            .stored
            .dictionaries
            .entry(dictionary_id.to_owned())
            .or_default();
        entry.allow_network = allow;
        if *entry == DictionaryPreferences::default() {
            self.stored.dictionaries.remove(dictionary_id);
        }
        self.save()
    }

    /// Writes to a temporary file first so a crash never leaves a half-written file.
    fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(&self.stored).expect("preferences serialise");
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(|error| problem(&self.file, error))?;
        }
        let temporary = self.file.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|error| problem(&self.file, error))?;
        std::fs::rename(&temporary, &self.file).map_err(|error| problem(&self.file, error))
    }
}

fn problem(file: &Path, error: impl ToString) -> Error {
    Error::Preferences {
        file: file.to_owned(),
        reason: error.to_string(),
    }
}
