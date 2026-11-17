//! The registration codes and e-mails the reader has entered, kept in their own file
//! beside the Library record so the record says nothing about the reader. The file is
//! only ever written whole, and on Unix only its owner may read it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::format::Registration;

/// Registrations by the path of the Dictionary's main file.
#[derive(Default, Clone)]
pub(crate) struct Registrations {
    /// Where they are kept; `None` keeps them in memory only.
    file: Option<PathBuf>,
    by_path: BTreeMap<PathBuf, Registration>,
}

impl Registrations {
    /// The registrations kept in `file`; none if it does not exist yet.
    pub(crate) fn load(file: PathBuf) -> Result<Registrations> {
        let by_path = match std::fs::read_to_string(&file) {
            // The parser's message could quote the file, so it is not passed on.
            Ok(text) => serde_json::from_str(&text)
                .map_err(|_| failed(&file, "it is not a valid registrations file"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(failed(&file, error)),
        };
        Ok(Registrations {
            file: Some(file),
            by_path,
        })
    }

    pub(crate) fn get(&self, path: &Path) -> Option<&Registration> {
        self.by_path.get(path)
    }

    /// Keeps `registration` for the Dictionary at `path` and writes the file. If the file
    /// cannot be written, nothing is kept.
    pub(crate) fn insert(&mut self, path: &Path, registration: Registration) -> Result<()> {
        let mut by_path = self.by_path.clone();
        by_path.insert(path.to_owned(), registration);
        if let Some(file) = &self.file {
            write(file, &by_path)?;
        }
        self.by_path = by_path;
        Ok(())
    }
}

fn write(file: &Path, by_path: &BTreeMap<PathBuf, Registration>) -> Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| failed(file, error))?;
    }
    let text = serde_json::to_string_pretty(by_path).map_err(|error| failed(file, error))?;
    // Write beside the file, then replace it, so a crash never leaves half of it. A
    // leftover partial file is removed first: it might have been created with looser
    // permissions, which opening it again would keep.
    let partial = file.with_extension("json.partial");
    match std::fs::remove_file(&partial) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed(file, error)),
    }
    write_private(&partial, text.as_bytes()).map_err(|error| failed(file, error))?;
    std::fs::rename(&partial, file).map_err(|error| failed(file, error))
}

/// Writes a file only its owner can read. (On Windows the application data folder is
/// already private to the user.)
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

/// An error naming the file but never its contents.
fn failed(file: &Path, error: impl std::fmt::Display) -> Error {
    Error::LibraryFile {
        file: file.to_owned(),
        reason: error.to_string(),
    }
}
