use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The headwords the reader committed Lookups for (Enter, a click, a followed link),
/// newest first, each listed once. Kept in one JSON file; every change is written
/// straight away.
#[derive(Debug, Clone)]
pub struct History {
    file: PathBuf,
    stored: Stored,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    /// Newest first.
    #[serde(default)]
    headwords: Vec<String>,
}

impl History {
    /// How many Lookups History keeps; older ones fall off the end.
    pub const LIMIT: usize = 1000;

    /// Reads History from `file`. A missing file means an empty History.
    pub fn load(file: &Path) -> Result<History> {
        let stored = match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| problem(file, error))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Stored::default(),
            Err(error) => return Err(problem(file, error)),
        };
        Ok(History {
            file: file.to_owned(),
            stored,
        })
    }

    /// The recorded headwords, newest first.
    pub fn headwords(&self) -> &[String] {
        &self.stored.headwords
    }

    /// Records a Lookup of `headword` as the newest, and saves. A headword already in
    /// History moves to the top. Blank text is not a Lookup and is ignored.
    pub fn record(&mut self, headword: &str) -> Result<()> {
        let headword = headword.trim();
        if headword.is_empty() {
            return Ok(());
        }
        let headwords = &mut self.stored.headwords;
        if headwords.first().is_some_and(|newest| newest == headword) {
            return Ok(());
        }
        headwords.retain(|known| known != headword);
        headwords.insert(0, headword.to_owned());
        headwords.truncate(Self::LIMIT);
        self.save()
    }

    /// Forgets every Lookup, and saves.
    pub fn clear(&mut self) -> Result<()> {
        self.stored.headwords.clear();
        self.save()
    }

    /// Writes to a temporary file first so a crash never leaves a half-written file.
    fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(&self.stored).expect("History serialises");
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(|error| problem(&self.file, error))?;
        }
        let temporary = self.file.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|error| problem(&self.file, error))?;
        std::fs::rename(&temporary, &self.file).map_err(|error| problem(&self.file, error))
    }
}

fn problem(file: &Path, error: impl ToString) -> Error {
    Error::History {
        file: file.to_owned(),
        reason: error.to_string(),
    }
}
