use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// OpenDict's configuration. For now it names the one Library Folder; Library Folders
/// managed from the interface replace it later.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub library_folder: Option<PathBuf>,
}

impl Config {
    /// Reads the configuration from a JSON file. A missing file is an empty configuration.
    pub fn load(file: &Path) -> Result<Config> {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(error) => return Err(Error::Config { file: file.to_owned(), reason: error.to_string() }),
        };
        serde_json::from_str(&text).map_err(|error| Error::Config { file: file.to_owned(), reason: error.to_string() })
    }
}
