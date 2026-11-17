use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::format::{
    DictionaryFormat, DictionaryReader, MdictFormat, OpenError, RawRecord, Registration,
};
use crate::index::{HeadwordIndex, IndexedHeadword, Start, StoredIndex, index_key};
use crate::indexing::{Built, IndexJob, IndexOutcome, IndexResult, IndexState, index_file};
use crate::registrations::Registrations;
use crate::scan::{self, Stamp, Unit};

/// Every Dictionary the reader has made available: the Dictionaries found in their
/// Library Folders, in Library order, each enabled or not.
///
/// A Library loaded from a file ([`Library::load`]) records its Library Folders, order
/// and enabled states there after every change, and keeps the registration codes the
/// reader enters in `registrations.json` beside it. Dictionary files are only ever read.
pub struct Library {
    file: Option<PathBuf>,
    /// Where indexes are stored: `indexes` beside the Library file. `None` for a Library
    /// kept only in memory, whose indexes are built in memory as it opens.
    index_folder: Option<PathBuf>,
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
    reader: Arc<dyn DictionaryReader>,
    stamp: Stamp,
    /// Present when `info.index` is [`IndexState::Ready`].
    index: Option<StoredIndex>,
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
    /// Whether its index is built. Only a Dictionary whose index is ready answers.
    pub index: IndexState,
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

/// What the headword column offers for what the reader typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestions {
    /// Headwords in index order: the matches first, then the headwords that follow them.
    pub items: Vec<Suggestion>,
    /// The query was an inflected form of a headword.
    pub lemma: Option<Lemma>,
    /// Nothing matched exactly or by prefix; the items are close spellings.
    pub near: bool,
    /// Nothing matched; the items are the headwords after where the query would stand.
    pub miss: bool,
    /// The query was empty; the items are the headword index from its start.
    pub browse: bool,
}

/// One headword in the headword column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub headword: String,
    /// How many enabled Dictionaries file this headword.
    pub count: usize,
}

/// An inflected form and the headword of its base form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lemma {
    pub from: String,
    pub to: String,
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
    ///
    /// Indexes are stored in the folder `indexes` beside `file`. A Dictionary whose
    /// stored index is missing or out of date is [`IndexState::Pending`] until its index
    /// is built: see [`Library::index_jobs`].
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

    /// A Library of the one Library Folder `folder`, kept only in memory. Every
    /// Dictionary is indexed (in memory) before it is returned, and again on a rescan.
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
            index_folder: file.as_deref().map(|file| file.with_file_name("indexes")),
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
        let mut known = std::mem::take(&mut self.dictionaries);
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
                let stamp = match scan::stamp(path) {
                    Ok(stamp) => stamp,
                    Err(error) => {
                        failures.push(OpenFailure {
                            path: path.clone(),
                            reason: OpenError::Unreadable(error.to_string()),
                        });
                        continue;
                    }
                };
                let opened = match self.registrations.get(path) {
                    Some(registration) => format.open_registered(path, registration),
                    None => format.open(path),
                };
                match opened {
                    Ok(reader) => {
                        let mut info = describe(id, &unit, reader.as_ref(), &self.record);
                        let index = self.known_index(&mut known, &info.id, &stamp);
                        if index.is_some() {
                            info.index = IndexState::Ready;
                        }
                        dictionaries.push(Dictionary {
                            info,
                            reader: Arc::from(reader),
                            stamp,
                            index,
                        });
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
        if self.index_folder.is_none() {
            self.build_indexes();
        }
        Ok(())
    }

    /// The index a Dictionary already has: the one in memory from before a rescan, or the
    /// stored one, if it was built from the file as it is now.
    fn known_index(
        &self,
        known: &mut Vec<Dictionary>,
        id: &str,
        stamp: &Stamp,
    ) -> Option<StoredIndex> {
        if let Some(at) = known
            .iter()
            .position(|d| d.info.id == id && d.stamp == *stamp && d.index.is_some())
        {
            return known.swap_remove(at).index;
        }
        let folder = self.index_folder.as_ref()?;
        StoredIndex::open(&index_file(folder, id), stamp)
    }

    /// One job for each Dictionary whose index is [`IndexState::Pending`], in Library
    /// order. Run them anywhere (each holds what it needs) and hand each outcome back
    /// with [`Library::finish_index`].
    pub fn index_jobs(&self) -> Vec<IndexJob> {
        self.dictionaries
            .iter()
            .filter(|d| d.info.index == IndexState::Pending)
            .map(|d| IndexJob {
                dictionary_id: d.info.id.clone(),
                title: d.info.title.clone(),
                reader: Arc::clone(&d.reader),
                stamp: d.stamp,
                folder: self.index_folder.clone(),
            })
            .collect()
    }

    /// Puts a built index in place, or records why it could not be built. An outcome
    /// for a Dictionary that has left the Library, or whose file changed meanwhile, is
    /// dropped.
    pub fn finish_index(&mut self, outcome: IndexOutcome) {
        let dictionary = self
            .dictionaries
            .iter_mut()
            .find(|d| d.info.id == outcome.dictionary_id);
        let built = match outcome.result {
            IndexResult::Built(built) => built,
            IndexResult::Cancelled => return,
            IndexResult::Failed(reason) => {
                if let Some(dictionary) = dictionary {
                    dictionary.info.index = IndexState::Failed(reason);
                }
                return;
            }
        };
        let Some(dictionary) = dictionary.filter(|d| d.stamp == built.stamp) else {
            if let Built::File { partial, .. } = &built.place {
                let _ = std::fs::remove_file(partial);
            }
            return;
        };
        let index = match built.place {
            Built::Memory(bytes) => StoredIndex::from_bytes(bytes, &built.stamp)
                .ok_or_else(|| "the index could not be read back".to_owned()),
            Built::File { partial, path } => {
                // Unmap the old index first: a mapped file cannot be replaced on Windows.
                dictionary.index = None;
                std::fs::rename(&partial, &path)
                    .map_err(|error| error.to_string())
                    .and_then(|()| {
                        StoredIndex::open(&path, &built.stamp)
                            .ok_or_else(|| "the stored index could not be read back".to_owned())
                    })
            }
        };
        match index {
            Ok(index) => {
                dictionary.index = Some(index);
                dictionary.info.index = IndexState::Ready;
            }
            Err(reason) => dictionary.info.index = IndexState::Failed(reason),
        }
    }

    /// Builds every pending index here and now, without progress reports. The desktop
    /// shell runs [`Library::index_jobs`] on a thread instead.
    pub fn build_indexes(&mut self) {
        for job in self.index_jobs() {
            let outcome = job.run(&mut |_| ControlFlow::Continue(()));
            self.finish_index(outcome);
        }
    }

    /// The headword index of the Dictionary with key `id`, if it is built.
    pub fn index_of(&self, id: &str) -> Option<&dyn HeadwordIndex> {
        self.dictionaries
            .iter()
            .find(|d| d.info.id == id)
            .and_then(|d| d.index.as_ref())
            .map(|index| index as &dyn HeadwordIndex)
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
            entries.extend(dictionary.entry(headword)?);
        }
        Ok(entries)
    }

    /// Up to `limit` suggestions for `query`, from enabled Dictionaries: the headwords
    /// whose key starts with the query's, then the headwords that follow them in index
    /// order, like a page of a printed index. An empty query browses the index from its
    /// start.
    pub fn suggest(&self, query: &str, limit: usize) -> Result<Suggestions> {
        let key = index_key(query.trim());
        let items = self.merged(Start::AtKey(&key), limit)?;
        let browse = key.is_empty();
        let miss = !browse
            && !items
                .first()
                .is_some_and(|(first, _)| first.key.starts_with(&key));
        Ok(Suggestions {
            items: items.into_iter().map(suggestion).collect(),
            lemma: None,
            near: false,
            miss,
            browse,
        })
    }

    /// Up to `limit` headwords that follow `headword` in index order, from enabled
    /// Dictionaries: the column continued as the reader scrolls.
    pub fn headwords_after(&self, headword: &str, limit: usize) -> Result<Vec<Suggestion>> {
        let after = IndexedHeadword::new(headword);
        Ok(self
            .merged(Start::After(&after), limit)?
            .into_iter()
            .map(suggestion)
            .collect())
    }

    /// The first `limit` headwords from `start` across enabled Dictionaries, each once,
    /// with how many of them file it.
    fn merged(&self, start: Start<'_>, limit: usize) -> Result<Vec<(IndexedHeadword, usize)>> {
        // Each headword among the first `limit` overall is among the first `limit` of
        // every Dictionary that files it, so `limit` from each is enough to count it.
        let mut counts: BTreeMap<IndexedHeadword, usize> = BTreeMap::new();
        for dictionary in self.dictionaries.iter().filter(|d| d.info.enabled) {
            // A Dictionary answers once its index is built.
            let Some(index) = &dictionary.index else {
                continue;
            };
            let headwords = index
                .headwords(start, limit)
                .map_err(|error| Error::Dictionary {
                    dictionary: dictionary.info.title.clone(),
                    reason: error.to_string(),
                })?;
            for headword in headwords {
                *counts.entry(headword).or_default() += 1;
            }
        }
        Ok(counts.into_iter().take(limit).collect())
    }

    /// The Entry for exactly `headword` in the Dictionary with `dictionary_id`, if that
    /// Dictionary is in the Library and has the headword.
    pub fn entry(&self, dictionary_id: &str, headword: &str) -> Result<Option<Entry>> {
        match self.dictionary(dictionary_id) {
            Some(dictionary) => dictionary.entry(headword),
            None => Ok(None),
        }
    }

    /// The bytes of a file the Dictionary's Entries refer to, by the relative path an Entry
    /// uses (`img/cat.png`): from its MDD files or from beside its main file. `None` when
    /// the Dictionary has no such file; never a file from outside the Dictionary.
    pub fn resource(&self, dictionary_id: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let Some(dictionary) = self.dictionary(dictionary_id) else {
            return Ok(None);
        };
        dictionary
            .reader
            .resource(path)
            .map_err(|error| dictionary.error(error))
    }

    /// An enabled Dictionary: a disabled one answers nothing, not even by its key.
    fn dictionary(&self, id: &str) -> Option<&Dictionary> {
        self.dictionaries
            .iter()
            .find(|d| d.info.id == id && d.info.enabled)
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

fn suggestion((headword, count): (IndexedHeadword, usize)) -> Suggestion {
    Suggestion {
        headword: headword.headword,
        count,
    }
}

impl Dictionary {
    /// The Entry for exactly `headword`. A Dictionary answers once its index is built.
    fn entry(&self, headword: &str) -> Result<Option<Entry>> {
        let Some(index) = &self.index else {
            return Ok(None);
        };
        let records = resolve(self.reader.as_ref(), index, headword).map_err(|e| self.error(e))?;
        Ok((!records.is_empty()).then(|| Entry {
            dictionary_id: self.info.id.clone(),
            dictionary_title: self.info.title.clone(),
            html: records.join("\n"),
        }))
    }

    fn error(&self, error: impl std::fmt::Display) -> Error {
        Error::Dictionary {
            dictionary: self.info.title.clone(),
            reason: error.to_string(),
        }
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
    index: &StoredIndex,
    headword: &str,
) -> std::result::Result<Vec<String>, String> {
    let records = |headword: &str| -> std::result::Result<Vec<RawRecord>, String> {
        index
            .locations(headword)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|location| reader.record(location).map_err(|error| error.to_string()))
            .collect()
    };
    let mut texts = Vec::new();
    for record in records(headword)? {
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
                    let Some(next) = records(&target)?.into_iter().next() else {
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
        index: IndexState::Pending,
    }
}
