use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::format::{
    DictionaryFormat, DictionaryReader, MdictFormat, OpenError, RawRecord, Registration,
};
use crate::registrations::Registrations;
use crate::scan::{self, Unit};

/// Every Dictionary the reader has made available: the Dictionaries found in their
/// Library Folders, in Library order, each enabled or not.
///
/// A Library loaded from a file ([`Library::load`]) records its Library Folders, order
/// and enabled states there after every change, and keeps the registration codes the
/// reader enters in `registrations.json` beside it. Dictionary files are only ever read.
pub struct Library {
    file: Option<PathBuf>,
    formats: Vec<&'static dyn DictionaryFormat>,
    record: Record,
    registrations: Registrations,
    folders: Vec<LibraryFolder>,
    dictionaries: Vec<Dictionary>,
    failures: Vec<OpenFailure>,
}

/// What the Library keeps between starts.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Record {
    folders: Vec<PathBuf>,
    /// Dictionary keys in Library order. Keys of Dictionaries not found right now stay,
    /// so a Dictionary on a drive that is briefly missing keeps its place.
    order: Vec<String>,
    disabled: Vec<String>,
}

struct Dictionary {
    info: DictionaryInfo,
    reader: Box<dyn DictionaryReader>,
}

/// A Library Folder and what was found in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFolder {
    pub path: PathBuf,
    /// How many Dictionaries were found in it, including any that failed to open.
    pub dictionary_count: usize,
    /// Why the folder could not be read, if it could not.
    pub problem: Option<String>,
}

/// What the Library knows about one of its Dictionaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryInfo {
    /// The Dictionary's key. It stays the same across rescans and restarts.
    pub id: String,
    /// The title from the Dictionary's header, or its file name when the header has none.
    pub title: String,
    /// The Dictionary's main file (the MDX).
    pub path: PathBuf,
    /// Its resource files (`name.mdd`, `name.1.mdd`, ...), in order.
    pub resources: Vec<PathBuf>,
    /// The stylesheets and scripts beside the main file.
    pub companions: Vec<PathBuf>,
    /// How many Entries it files.
    pub entry_count: usize,
    /// The size of all its files together, in bytes.
    pub size: u64,
    /// Whether it answers Lookups.
    pub enabled: bool,
}

/// A Dictionary file that was found but could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFailure {
    pub path: PathBuf,
    pub reason: OpenError,
}

/// What one Dictionary says about one Headword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub dictionary_id: String,
    pub dictionary_title: String,
    /// The Entry's content: untrusted HTML that must be rendered in isolation.
    pub html: String,
}

/// A step in Library order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// Towards the front: answer earlier.
    Up,
    /// Towards the back: answer later.
    Down,
}

/// The longest `@@@LINK=` chain the Library follows before giving up.
const MAX_REDIRECTS: usize = 8;

impl Library {
    /// The Library recorded in `file`, with its Library Folders scanned. A missing file is
    /// an empty Library; the file is written on the first change.
    pub fn load(file: &Path) -> Result<Library> {
        Self::load_with(file, &[&MdictFormat])
    }

    /// Like [`Library::load`], reading only the given formats.
    pub fn load_with(file: &Path, formats: &[&'static dyn DictionaryFormat]) -> Result<Library> {
        let record = match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| Error::LibraryFile {
                file: file.to_owned(),
                reason: error.to_string(),
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Record::default(),
            Err(error) => {
                return Err(Error::LibraryFile {
                    file: file.to_owned(),
                    reason: error.to_string(),
                });
            }
        };
        let mut library = Library::new(Some(file.to_owned()), formats, record);
        library.registrations = Registrations::load(file.with_file_name("registrations.json"))?;
        library.rescan()?;
        Ok(library)
    }

    /// A Library of the one Library Folder `folder`, kept only in memory.
    pub fn open(folder: &Path) -> Result<Library> {
        Self::open_with(folder, &[&MdictFormat])
    }

    /// Like [`Library::open`], reading only the given formats.
    pub fn open_with(folder: &Path, formats: &[&'static dyn DictionaryFormat]) -> Result<Library> {
        readable(folder)?;
        let record = Record {
            folders: vec![folder.to_owned()],
            ..Record::default()
        };
        let mut library = Library::new(None, formats, record);
        library.rescan()?;
        Ok(library)
    }

    fn new(
        file: Option<PathBuf>,
        formats: &[&'static dyn DictionaryFormat],
        record: Record,
    ) -> Library {
        Library {
            file,
            formats: formats.to_vec(),
            record,
            registrations: Registrations::default(),
            folders: Vec::new(),
            dictionaries: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// The Library Folders, in the order they were added.
    pub fn folders(&self) -> &[LibraryFolder] {
        &self.folders
    }

    /// Adds a Library Folder: every Dictionary inside it, including in subfolders, joins
    /// the Library. Adding a folder that is already there changes nothing.
    pub fn add_folder(&mut self, folder: &Path) -> Result<()> {
        readable(folder)?;
        if self.record.folders.iter().any(|known| known == folder) {
            return Ok(());
        }
        self.record.folders.push(folder.to_owned());
        self.rescan()?;
        self.save()
    }

    /// Removes a Library Folder: its Dictionaries leave the Library. Nothing is deleted
    /// from disk.
    pub fn remove_folder(&mut self, folder: &Path) -> Result<()> {
        self.record.folders.retain(|known| known != folder);
        self.rescan()?;
        self.save()
    }

    /// Scans every Library Folder again, so Dictionaries added or removed on disk since the
    /// last scan join or leave the Library. Dictionaries already known keep their key,
    /// place and enabled state.
    pub fn rescan(&mut self) -> Result<()> {
        let mut folders = Vec::new();
        let mut dictionaries: Vec<Dictionary> = Vec::new();
        let mut failures: Vec<OpenFailure> = Vec::new();

        for folder in &self.record.folders {
            let files = match scan::files_under(folder) {
                Ok(files) => files,
                Err(error) => {
                    folders.push(LibraryFolder {
                        path: folder.clone(),
                        dictionary_count: 0,
                        problem: Some(error.to_string()),
                    });
                    continue;
                }
            };
            let mut count = 0;
            for path in &files {
                let Some(format) = self.formats.iter().find(|format| format.recognises(path))
                else {
                    continue;
                };
                count += 1;
                let unit = Unit::gather(path, &files);
                let id = match scan::identity(path) {
                    Ok(id) => id,
                    Err(error) => {
                        failures.push(OpenFailure {
                            path: path.clone(),
                            reason: OpenError::Unreadable(error.to_string()),
                        });
                        continue;
                    }
                };
                // Library Folders may overlap; a Dictionary joins the Library once.
                if dictionaries.iter().any(|known| known.info.id == id)
                    || failures.iter().any(|known| same_file(&known.path, path))
                {
                    continue;
                }
                let opened = match self.registrations.get(path) {
                    Some(registration) => format.open_registered(path, registration),
                    None => format.open(path),
                };
                match opened {
                    Ok(reader) => {
                        let info = describe(id, &unit, reader.as_ref(), &self.record);
                        dictionaries.push(Dictionary { info, reader });
                    }
                    Err(reason) => failures.push(OpenFailure {
                        path: path.clone(),
                        reason,
                    }),
                }
            }
            folders.push(LibraryFolder {
                path: folder.clone(),
                dictionary_count: count,
                problem: None,
            });
        }

        // Library order: known Dictionaries where the record puts them, newcomers after
        // them in the order they were found.
        let position = |id: &str| self.record.order.iter().position(|known| known == id);
        dictionaries.sort_by_key(|dictionary| position(&dictionary.info.id).unwrap_or(usize::MAX));

        self.folders = folders;
        self.dictionaries = dictionaries;
        self.failures = failures;

        if self.record_order() {
            self.save()?;
        }
        Ok(())
    }

    /// Every Dictionary in the Library, enabled or not, in Library order.
    pub fn dictionaries(&self) -> impl Iterator<Item = &DictionaryInfo> {
        self.dictionaries.iter().map(|dictionary| &dictionary.info)
    }

    /// The Dictionary files that were found but could not be opened.
    pub fn failures(&self) -> &[OpenFailure] {
        &self.failures
    }

    /// Unlocks the Dictionary at `path`, which failed to open for want of its registration
    /// code, with the code and e-mail (or device ID) the reader entered. When they unlock
    /// it, they are kept, so it opens on every later scan, and it joins the Library. When
    /// they do not, nothing is kept and the error says why.
    pub fn register(&mut self, path: &Path, registration: Registration) -> Result<()> {
        let waiting = self.failures.iter().any(|failure| {
            failure.path == path
                && matches!(
                    failure.reason,
                    OpenError::NeedsRegistrationCode | OpenError::WrongRegistrationCode
                )
        });
        let format = self
            .formats
            .iter()
            .find(|format| format.recognises(path))
            .filter(|_| waiting)
            .ok_or_else(|| Error::UnknownDictionary(path.display().to_string()))?;
        match format.open_registered(path, &registration) {
            Ok(reader) => drop(reader),
            Err(OpenError::WrongRegistrationCode) => {
                return Err(Error::WrongRegistration(path.to_owned()));
            }
            Err(OpenError::NeedsRegistrationCode) => {
                return Err(Error::RegistrationCodeNeeded(path.to_owned()));
            }
            Err(reason) => {
                return Err(Error::Dictionary {
                    dictionary: path.display().to_string(),
                    reason: reason.to_string(),
                });
            }
        }
        self.registrations.insert(path, registration)?;
        self.rescan()
    }

    /// Enables or disables a Dictionary. A disabled Dictionary stays in the Library but
    /// does not answer Lookups.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        let dictionary = self.find_mut(id)?;
        dictionary.info.enabled = enabled;
        self.record.disabled.retain(|known| known != id);
        if !enabled {
            self.record.disabled.push(id.to_owned());
        }
        self.save()
    }

    /// Moves a Dictionary one place up or down in Library order. Moving the first one up
    /// or the last one down changes nothing.
    pub fn move_dictionary(&mut self, id: &str, step: Move) -> Result<()> {
        let from = self
            .dictionaries
            .iter()
            .position(|dictionary| dictionary.info.id == id)
            .ok_or_else(|| Error::UnknownDictionary(id.to_owned()))?;
        let to = match step {
            Move::Up => from.checked_sub(1),
            Move::Down => Some(from + 1).filter(|&to| to < self.dictionaries.len()),
        };
        let Some(to) = to else {
            return Ok(());
        };
        self.dictionaries.swap(from, to);
        self.record_order();
        self.save()
    }

    /// The Entries for exactly `headword`, one per enabled Dictionary that has it, in
    /// Library order. Redirects (`@@@LINK=`) are followed; several records under the
    /// headword in one Dictionary are kept together in one Entry.
    pub fn look_up(&self, headword: &str) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for dictionary in self.dictionaries.iter().filter(|d| d.info.enabled) {
            let records = resolve(dictionary.reader.as_ref(), headword).map_err(|error| {
                Error::Dictionary {
                    dictionary: dictionary.info.title.clone(),
                    reason: error.to_string(),
                }
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

    fn find_mut(&mut self, id: &str) -> Result<&mut Dictionary> {
        self.dictionaries
            .iter_mut()
            .find(|dictionary| dictionary.info.id == id)
            .ok_or_else(|| Error::UnknownDictionary(id.to_owned()))
    }

    /// Writes the current Library order into the record: the Dictionaries found now, then
    /// the keys of those not found now. Returns whether the record changed.
    fn record_order(&mut self) -> bool {
        let mut order: Vec<String> = self
            .dictionaries
            .iter()
            .map(|dictionary| dictionary.info.id.clone())
            .collect();
        let absent: Vec<String> = self
            .record
            .order
            .iter()
            .filter(|id| !order.contains(id))
            .cloned()
            .collect();
        order.extend(absent);
        let changed = order != self.record.order;
        self.record.order = order;
        changed
    }

    fn save(&self) -> Result<()> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        let failed = |reason: String| Error::LibraryFile {
            file: file.clone(),
            reason,
        };
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|error| failed(error.to_string()))?;
        }
        let text = serde_json::to_string_pretty(&self.record)
            .map_err(|error| failed(error.to_string()))?;
        // Write beside the file, then replace it, so a crash never leaves half a record.
        let partial = file.with_extension("json.partial");
        std::fs::write(&partial, text).map_err(|error| failed(error.to_string()))?;
        std::fs::rename(&partial, file).map_err(|error| failed(error.to_string()))
    }
}

fn readable(folder: &Path) -> Result<()> {
    std::fs::read_dir(folder)
        .map(drop)
        .map_err(|source| Error::LibraryFolder {
            folder: folder.to_owned(),
            source,
        })
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// The text records for `headword`, following redirects with cycle protection.
fn resolve(
    reader: &dyn DictionaryReader,
    headword: &str,
) -> std::result::Result<Vec<String>, crate::FormatError> {
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

fn describe(
    id: String,
    unit: &Unit,
    reader: &dyn DictionaryReader,
    record: &Record,
) -> DictionaryInfo {
    let title = match reader.title().trim() {
        "" => unit
            .main
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default(),
        title => title.to_owned(),
    };
    let size = unit
        .files()
        .filter_map(|file| std::fs::metadata(file).ok())
        .map(|metadata| metadata.len())
        .sum();
    DictionaryInfo {
        enabled: !record.disabled.contains(&id),
        id,
        title,
        path: unit.main.clone(),
        resources: unit.resources.clone(),
        companions: unit.companions.clone(),
        entry_count: reader.entry_count(),
        size,
    }
}
