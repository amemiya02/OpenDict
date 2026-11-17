//! The data directory: the one folder all of a reader's data lives in, which the reader
//! can move as a whole (for example into a synced folder).
//!
//! The app's home folder (the platform's folder for the app's data) is where the data
//! directory starts. Once it has been moved, a small file in the home folder says where
//! it went; that file is the only thing the app keeps outside the data directory.
//!
//! A move copies everything first and switches over only once the copy is complete and
//! on disk; the old copy is removed last. A move that fails, is cancelled or is cut short
//! leaves the reader's data where it was.

use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The names, inside the data directory, that the reader's data is kept under. Anything
/// else in the folder is not OpenDict's and is never moved or removed.
const DATA: [&str; 6] = [
    "library.json",
    "registrations.json",
    "indexes",
    "preferences.json",
    "history.json",
    "word-books.json",
];

/// The file in the home folder that says where the data directory is, once it has moved.
const POINTER: &str = "data-directory.json";

/// The file that says the OpenDict data beside it is not anyone's data: a move into the
/// folder has not finished, or a move out of it has and left something behind.
const MOVING: &str = ".opendict-moving";

/// The folder the reader's Library record, registrations, indexes, preferences, History
/// and Word Books are kept in.
#[derive(Debug, Clone)]
pub struct DataDirectory {
    home: PathBuf,
    path: PathBuf,
}

/// How far a move has got, in bytes copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveProgress {
    pub done: u64,
    pub total: u64,
}

/// A finished move.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relocation {
    /// What could not be removed from the old place after the data was safely in the new
    /// one. The data in the new place is complete either way.
    pub left_behind: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pointer {
    path: PathBuf,
}

impl DataDirectory {
    /// The data directory the app uses, given its home: the platform's folder for the
    /// app's data.
    pub fn locate(home: &Path) -> Result<DataDirectory> {
        let pointer = home.join(POINTER);
        let path = match std::fs::read_to_string(&pointer) {
            Ok(text) => {
                serde_json::from_str::<Pointer>(&text)
                    .map_err(|error| failed(&pointer, error))?
                    .path
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => home.to_owned(),
            Err(error) => return Err(failed(&pointer, error)),
        };
        // The home folder is made when something is first saved; a folder the data was
        // moved to must still be there.
        if path != home && !path.is_dir() {
            return Err(Error::DataDirectoryMissing(path));
        }
        // A move into this folder finished, but stopped before it could say so.
        let _ = std::fs::remove_file(path.join(MOVING));
        Ok(DataDirectory {
            home: home.to_owned(),
            path,
        })
    }

    /// Where the data directory is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the data directory is in the home folder, where a new reader's data starts.
    pub fn is_default(&self) -> bool {
        self.path == self.home
    }

    /// The file the Library is recorded in. Registrations and indexes are kept beside it.
    pub fn library_file(&self) -> PathBuf {
        self.path.join("library.json")
    }

    /// The file per-Dictionary preferences are kept in.
    pub fn preferences_file(&self) -> PathBuf {
        self.path.join("preferences.json")
    }

    /// The file History is kept in.
    pub fn history_file(&self) -> PathBuf {
        self.path.join("history.json")
    }

    /// The file Word Books are kept in.
    pub fn word_books_file(&self) -> PathBuf {
        self.path.join("word-books.json")
    }

    /// Moves the reader's data into the folder `to` (made if it does not exist) and uses
    /// it from then on, here and at every later start.
    ///
    /// Nothing may have the data's files open meanwhile: drop the Library, History,
    /// Word Books and preferences first, and load them again from the new place.
    ///
    /// `progress` hears how far the copy has got and may cancel it; `None` means it was
    /// cancelled and the data stayed where it was. On an error the data stays where it was
    /// too.
    pub fn relocate(
        &mut self,
        to: &Path,
        progress: &mut dyn FnMut(MoveProgress) -> ControlFlow<()>,
    ) -> Result<Option<Relocation>> {
        let here = resolve(&self.path);
        let there = resolve(to);
        if there == here {
            return Ok(Some(Relocation::default()));
        }
        if there.starts_with(&here) {
            return Err(failed(to, "it is inside the data directory"));
        }
        let marker = to.join(MOVING);
        if marker.exists() {
            // An earlier move into this folder was cut short; what it copied is not
            // anyone's data yet.
            clear_unfinished(to).map_err(|error| failed(to, error))?;
        }
        if holds_data(to) {
            return Err(Error::DataDirectoryHoldsData(to.to_owned()));
        }
        let present: Vec<&str> = DATA
            .into_iter()
            .filter(|name| self.path.join(name).exists())
            .collect();
        let made = !to.exists();
        std::fs::create_dir_all(to).map_err(|error| failed(to, error))?;
        // Marks everything copied as unfinished until the move is done, so a move cut
        // short is recognised (and cleared) when the folder is chosen again.
        if let Err(error) = write_durably(&marker, b"") {
            undo(to, made);
            return Err(failed(to, error));
        }

        let mut copy = Copy {
            done: 0,
            total: present
                .iter()
                .map(|name| size(&self.path.join(name)))
                .sum(),
            progress,
        };
        let mut outcome = Ok(ControlFlow::Continue(()));
        for name in &present {
            outcome = copy.entry(&self.path.join(name), &to.join(name));
            if !matches!(outcome, Ok(ControlFlow::Continue(()))) {
                break;
            }
        }
        let switched = match outcome {
            Ok(ControlFlow::Continue(())) => point(&self.home, to),
            Ok(ControlFlow::Break(())) => {
                undo(to, made);
                return Ok(None);
            }
            Err(error) => Err(error),
        };
        if let Err(error) = switched {
            undo(to, made);
            return Err(error);
        }
        // The data is in its new place from here on; the marker only says so twice.
        let _ = std::fs::remove_file(&marker);

        // The old copy goes last. It is marked first, so whatever of it cannot be removed
        // now is known for a leftover and never stops the data moving back.
        let old = std::mem::replace(&mut self.path, to.to_owned());
        let _ = write_durably(&old.join(MOVING), b"");
        let left_behind: Vec<PathBuf> = present
            .iter()
            .map(|name| old.join(name))
            .filter(|path| remove(path).is_err())
            .collect();
        if left_behind.is_empty() {
            let _ = std::fs::remove_file(old.join(MOVING));
        }
        Ok(Some(Relocation { left_behind }))
    }

    /// Uses the data already in `folder` (made if it does not exist, to start afresh
    /// there) from now on, given the app's home. The data in use until now is left as it
    /// was, wherever it is. This is how a second computer takes up data from a synced
    /// folder, and how a reader whose data directory has gone chooses another.
    pub fn use_folder(home: &Path, folder: &Path) -> Result<DataDirectory> {
        std::fs::create_dir_all(folder).map_err(|error| failed(folder, error))?;
        if folder.join(MOVING).exists() {
            // Only a copy that never finished, or a leftover, is here: not data to use.
            clear_unfinished(folder).map_err(|error| failed(folder, error))?;
        }
        point(home, folder)?;
        DataDirectory::locate(home)
    }
}

/// Records in the app's `home` that the data directory is `to`, in one step.
fn point(home: &Path, to: &Path) -> Result<()> {
    let pointer = home.join(POINTER);
    if resolve(to) == resolve(home) {
        return match std::fs::remove_file(&pointer) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(failed(&pointer, error)),
        };
    }
    let text = serde_json::to_string_pretty(&Pointer {
        path: to.to_owned(),
    })
    .expect("the pointer serialises");
    let partial = pointer.with_extension("json.partial");
    write_durably(&partial, text.as_bytes()).map_err(|error| failed(&pointer, error))?;
    std::fs::rename(&partial, &pointer).map_err(|error| failed(&pointer, error))
}

/// A copy in progress, reporting as it goes.
struct Copy<'a> {
    done: u64,
    total: u64,
    progress: &'a mut dyn FnMut(MoveProgress) -> ControlFlow<()>,
}

impl Copy<'_> {
    /// Copies the file or folder `from` to `to`, each file flushed to disk.
    fn entry(&mut self, from: &Path, to: &Path) -> Result<ControlFlow<()>> {
        let metadata = std::fs::metadata(from).map_err(|error| failed(from, error))?;
        if metadata.is_dir() {
            std::fs::create_dir(to).map_err(|error| failed(to, error))?;
            let mut children: Vec<_> = std::fs::read_dir(from)
                .map_err(|error| failed(from, error))?
                .map(|child| child.map(|child| child.file_name()))
                .collect::<std::io::Result<_>>()
                .map_err(|error| failed(from, error))?;
            children.sort();
            for child in children {
                if self.entry(&from.join(&child), &to.join(&child))?.is_break() {
                    return Ok(ControlFlow::Break(()));
                }
            }
            return Ok(ControlFlow::Continue(()));
        }
        std::fs::copy(from, to).map_err(|error| failed(from, error))?;
        // Flushing needs write access on Windows.
        std::fs::OpenOptions::new()
            .write(true)
            .open(to)
            .and_then(|file| file.sync_all())
            .map_err(|error| failed(to, error))?;
        self.done += metadata.len();
        Ok((self.progress)(MoveProgress {
            done: self.done,
            total: self.total,
        }))
    }
}

/// Whether `folder` holds any of a reader's data.
fn holds_data(folder: &Path) -> bool {
    DATA.iter().any(|name| folder.join(name).exists())
}

/// `path` with links and `..` resolved, as far as it exists, so two spellings of one
/// folder compare equal.
fn resolve(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut existing = path;
    loop {
        if let Ok(resolved) = existing.canonicalize() {
            return rest.iter().rev().fold(resolved, |path, name| path.join(name));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return path.to_owned(),
        }
    }
}

/// The bytes in the file or folder at `path`.
fn size(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => std::fs::read_dir(path)
            .map(|children| {
                children
                    .filter_map(|child| child.ok())
                    .map(|child| size(&child.path()))
                    .sum()
            })
            .unwrap_or(0),
        Ok(metadata) => metadata.len(),
        Err(_) => 0,
    }
}

fn remove(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// Removes what an unfinished move copied into `folder`, and the marker last.
fn clear_unfinished(folder: &Path) -> std::io::Result<()> {
    for name in DATA {
        let path = folder.join(name);
        if path.exists() {
            remove(&path)?;
        }
    }
    std::fs::remove_file(folder.join(MOVING))
}

/// Takes back a move into `folder` that did not finish, as far as it can: what was
/// copied, and the folder itself if the move `made` it.
fn undo(folder: &Path, made: bool) {
    if clear_unfinished(folder).is_ok() && made {
        let _ = std::fs::remove_dir(folder);
    }
}

fn write_durably(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn failed(path: &Path, error: impl std::fmt::Display) -> Error {
    Error::DataDirectory {
        path: path.to_owned(),
        reason: error.to_string(),
    }
}
