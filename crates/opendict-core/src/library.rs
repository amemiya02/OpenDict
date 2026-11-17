use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::format::{DictionaryFormat, DictionaryReader, MdictFormat, RawRecord};

/// The Dictionaries found in one Library Folder.
pub struct Library {
    folder: PathBuf,
    dictionaries: Vec<Dictionary>,
    failures: Vec<OpenFailure>,
}

struct Dictionary {
    info: DictionaryInfo,
    reader: Box<dyn DictionaryReader>,
}

/// What the Library knows about one of its Dictionaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryInfo {
    /// Identifies the Dictionary within its Library Folder (its path relative to the folder).
    pub id: String,
    /// The title from the Dictionary's header, or its file name when the header has none.
    pub title: String,
}

/// A Dictionary file that was found but could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFailure {
    pub path: PathBuf,
    pub reason: String,
}

/// What one Dictionary says about one Headword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub dictionary_id: String,
    pub dictionary_title: String,
    /// The Entry's content: untrusted HTML that must be rendered in isolation.
    pub html: String,
}

/// The longest `@@@LINK=` chain the Library follows before giving up.
const MAX_REDIRECTS: usize = 8;

impl Library {
    /// Scans `folder` and its subfolders for Dictionaries in the formats OpenDict reads.
    /// A Dictionary that fails to open is recorded in [`Library::failures`] and does not
    /// stop the others.
    pub fn open(folder: &Path) -> Result<Library> {
        Self::open_with(folder, &[&MdictFormat])
    }

    /// Like [`Library::open`], reading only the given formats.
    pub fn open_with(folder: &Path, formats: &[&dyn DictionaryFormat]) -> Result<Library> {
        let mut files = Vec::new();
        collect_files(folder, &mut files).map_err(|source| Error::LibraryFolder {
            folder: folder.to_owned(),
            source,
        })?;
        files.sort();

        let mut dictionaries = Vec::new();
        let mut failures = Vec::new();
        for path in files {
            let Some(format) = formats.iter().find(|format| format.recognises(&path)) else {
                continue;
            };
            match format.open(&path) {
                Ok(reader) => {
                    let info = describe(folder, &path, reader.as_ref());
                    dictionaries.push(Dictionary { info, reader });
                }
                Err(error) => failures.push(OpenFailure {
                    path,
                    reason: error.to_string(),
                }),
            }
        }
        Ok(Library {
            folder: folder.to_owned(),
            dictionaries,
            failures,
        })
    }

    /// The Library Folder this Library was read from.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The Dictionaries that opened, in Library order (by path for now).
    pub fn dictionaries(&self) -> impl Iterator<Item = &DictionaryInfo> {
        self.dictionaries.iter().map(|dictionary| &dictionary.info)
    }

    /// The Dictionary files that were found but could not be opened.
    pub fn failures(&self) -> &[OpenFailure] {
        &self.failures
    }

    /// The Entries for exactly `headword`, one per Dictionary that has it, in Library
    /// order. Redirects (`@@@LINK=`) are followed; several records under the headword in
    /// one Dictionary are kept together in one Entry.
    pub fn look_up(&self, headword: &str) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for dictionary in &self.dictionaries {
            let records = resolve(dictionary.reader.as_ref(), headword).map_err(|error| Error::Dictionary {
                dictionary: dictionary.info.title.clone(),
                reason: error.to_string(),
            })?;
            if !records.is_empty() {
                entries.push(Entry {
                    dictionary_id: dictionary.info.id.clone(),
                    dictionary_title: dictionary.info.title.clone(),
                    html: records.join("\n"),
                });
            }
        }
        Ok(entries)
    }
}

/// The text records for `headword`, following redirects with cycle protection.
fn resolve(reader: &dyn DictionaryReader, headword: &str) -> std::result::Result<Vec<String>, crate::FormatError> {
    let mut texts = Vec::new();
    for record in reader.records(headword)? {
        let mut record = record;
        let mut visited = vec![headword.to_owned()];
        loop {
            match record {
                RawRecord::Text(text) => {
                    texts.push(text);
                    break;
                }
                RawRecord::Link(target) => {
                    if visited.contains(&target) || visited.len() > MAX_REDIRECTS {
                        break;
                    }
                    let Some(next) = reader.records(&target)?.into_iter().next() else {
                        break;
                    };
                    visited.push(target);
                    record = next;
                }
            }
        }
    }
    Ok(texts)
}

fn describe(folder: &Path, path: &Path, reader: &dyn DictionaryReader) -> DictionaryInfo {
    let id = path
        .strip_prefix(folder)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let title = match reader.title().trim() {
        "" => path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| id.clone()),
        title => title.to_owned(),
    };
    DictionaryInfo { id, title }
}

fn collect_files(folder: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(&entry.path(), out)?;
        } else if file_type.is_file() {
            out.push(entry.path());
        }
    }
    Ok(())
}
