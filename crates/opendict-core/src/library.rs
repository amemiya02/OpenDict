use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::format::{
    DictionaryFormat, DictionaryReader, MdictFormat, OpenError, RawRecord, Registration,
};
use crate::index::{HeadwordIndex, IndexError, IndexedHeadword, Start, StoredIndex, index_key};
use crate::indexing::{Built, IndexJob, IndexOutcome, IndexResult, IndexState, index_file};
use crate::lemma;
use crate::near;
use crate::pattern::Pattern;
use crate::registrations::Registrations;
use crate::scan::{self, Stamp, Unit};

/// Every Dictionary the reader has made available: the Dictionaries found in their
/// Library Folders, in Library order, each enabled or not.
///
/// A Library loaded from a file ([`Library::load`]) records its Library Folders, order,
/// enabled states, Groups and Current Group there after every change, and keeps the
/// registration codes the reader enters in `registrations.json` beside it. Dictionary
/// files are only ever read.
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
    /// This Library's own number, and how many scans and registrations have changed it:
    /// a scan made before a change is not put in place after it.
    instance: u64,
    generation: u64,
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
    /// The reader's Groups, in the order they were created.
    groups: Vec<GroupRecord>,
    /// The Group Lookups run against; none means the whole Library.
    current_group: Option<String>,
}

/// A Group as the Library file keeps it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupRecord {
    id: String,
    name: String,
    /// Dictionary keys in Group order. Keys of Dictionaries not found right now stay, so
    /// a Dictionary on a drive that is briefly missing keeps its place in the Group.
    dictionaries: Vec<String>,
}

struct Dictionary {
    info: DictionaryInfo,
    reader: Arc<dyn DictionaryReader>,
    stamp: Stamp,
    /// Present when `info.index` is [`IndexState::Ready`].
    index: Option<StoredIndex>,
}

impl Dictionary {
    /// Up to `limit` headwords from `start` in index order that `scan` takes, reading the
    /// index a page at a time until `scan` says stop or the index ends. A Dictionary whose
    /// index is not built yet has none.
    fn scan(
        &self,
        start: Start<'_>,
        limit: usize,
        scan: &dyn Fn(&IndexedHeadword) -> Pass,
    ) -> std::result::Result<Vec<IndexedHeadword>, IndexError> {
        let Some(index) = self.index.as_ref() else {
            return Ok(Vec::new());
        };
        let page_size = limit.clamp(SCAN_PAGE, 16 * SCAN_PAGE);
        let mut taken = Vec::new();
        let mut page = index.headwords(start, page_size)?;
        while taken.len() < limit {
            let full = page.len() == page_size;
            for headword in &page {
                match scan(headword) {
                    Pass::Take => taken.push(headword.clone()),
                    Pass::Skip => {}
                    Pass::Stop => return Ok(taken),
                }
                if taken.len() == limit {
                    return Ok(taken);
                }
            }
            match page.last() {
                Some(last) if full => page = index.headwords(Start::After(last), page_size)?,
                _ => break,
            }
        }
        Ok(taken)
    }
}

/// What a scan of the index does with one headword.
enum Pass {
    /// Keep it and read on.
    Take,
    /// Pass it over and read on.
    Skip,
    /// Read no further.
    Stop,
}

/// How many headwords a scan reads from an index at least at a time.
const SCAN_PAGE: usize = 256;

/// A scan of Library Folders, made by [`Library::scan`] and its siblings. It holds what
/// it needs, so it runs (reading the folders and opening every Dictionary in them, the
/// slow part) without the Library; [`Library::finish_scan`] puts the result in place.
pub struct Scan {
    /// Which Library made it, and how many changes that Library had seen then.
    library: u64,
    generation: u64,
    folders: Vec<PathBuf>,
    formats: Vec<&'static dyn DictionaryFormat>,
    registrations: Registrations,
}

/// What a [`Scan`] found, for [`Library::finish_scan`].
pub struct Scanned {
    library: u64,
    generation: u64,
    /// The Library Folders scanned.
    folders: Vec<PathBuf>,
    found_folders: Vec<LibraryFolder>,
    opened: Vec<Opened>,
    failures: Vec<OpenFailure>,
}

/// A Dictionary a scan opened.
struct Opened {
    info: DictionaryInfo,
    reader: Arc<dyn DictionaryReader>,
    stamp: Stamp,
}

/// Each Library gets its own number, so a scan is never finished on another Library.
static LIBRARIES: AtomicU64 = AtomicU64::new(0);

impl Scan {
    /// Reads every Library Folder and opens every Dictionary found.
    pub fn run(self) -> Scanned {
        let mut found_folders = Vec::new();
        let mut opened: Vec<Opened> = Vec::new();
        let mut failures: Vec<OpenFailure> = Vec::new();

        for folder in &self.folders {
            let files = match scan::files_under(folder) {
                Ok(files) => files,
                Err(error) => {
                    found_folders.push(LibraryFolder {
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
                let unit = Unit::gather(path, &files, |file| {
                    self.formats.iter().any(|format| format.recognises(file))
                });
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
                if opened.iter().any(|known| known.info.id == id)
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
                let reader = match self.registrations.get(path) {
                    Some(registration) => format.open_registered(path, registration),
                    None => format.open(path),
                };
                match reader {
                    Ok(reader) => opened.push(Opened {
                        info: describe(id, &unit, reader.as_ref()),
                        reader: Arc::from(reader),
                        stamp,
                    }),
                    Err(reason) => failures.push(OpenFailure {
                        path: path.clone(),
                        reason,
                    }),
                }
            }
            found_folders.push(LibraryFolder {
                path: folder.clone(),
                dictionary_count: count,
                problem: None,
            });
        }
        Scanned {
            library: self.library,
            generation: self.generation,
            folders: self.folders,
            found_folders,
            opened,
            failures,
        }
    }
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

impl Entry {
    /// The audio the Entry offers: the targets of its `sound://` links, in the order they
    /// appear, as paths inside the Dictionary written the way the Entry writes them
    /// (`uk/apple.mp3`, `us\apple.mp3`).
    pub fn sounds(&self) -> Vec<String> {
        crate::links::href_values(&self.html)
            .filter_map(|href| strip_prefix_ignore_case(&href, "sound://").map(str::to_owned))
            .filter(|path| !path.is_empty())
            .collect()
    }
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
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

/// A named, ordered subset of the Library. A Dictionary may belong to several Groups,
/// with its own place in each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The Group's key. It stays the same when the Group is renamed.
    pub id: String,
    pub name: String,
    /// The keys of its Dictionaries found in the Library now, in Group order.
    pub dictionaries: Vec<String>,
}

/// Another set of Dictionaries that has the word the Current Group lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elsewhere {
    /// The Group's key, or `None` for the whole Library.
    pub group: Option<String>,
    /// The headword that set offers for the query.
    pub headword: String,
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
        let mut record: Record = match std::fs::read_to_string(file) {
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
        // A Current Group the file names but does not hold would leave nothing to answer.
        if let Some(current) = &record.current_group
            && !record.groups.iter().any(|group| &group.id == current)
        {
            record.current_group = None;
        }
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
            instance: LIBRARIES.fetch_add(1, AtomicOrdering::Relaxed),
            generation: 0,
        }
    }

    /// The Library Folders, in the order they were added.
    pub fn folders(&self) -> &[LibraryFolder] {
        &self.folders
    }

    /// Adds a Library Folder: every Dictionary inside it, including in subfolders, joins
    /// the Library. Adding a folder that is already there changes nothing.
    pub fn add_folder(&mut self, folder: &Path) -> Result<()> {
        if let Some(scan) = self.scan_with_folder(folder)? {
            self.finish_scan(scan.run())?;
        }
        Ok(())
    }

    /// Removes a Library Folder: its Dictionaries leave the Library. Nothing is deleted
    /// from disk.
    pub fn remove_folder(&mut self, folder: &Path) -> Result<()> {
        let scan = self.scan_without_folder(folder);
        self.finish_scan(scan.run())?;
        Ok(())
    }

    /// Scans every Library Folder again, so Dictionaries added or removed on disk since the
    /// last scan join or leave the Library. Dictionaries already known keep their key,
    /// place and enabled state.
    ///
    /// To scan without holding the Library, use [`Library::scan`], run it, and hand the
    /// result to [`Library::finish_scan`].
    pub fn rescan(&mut self) -> Result<()> {
        loop {
            let scanned = self.scan().run();
            if self.finish_scan(scanned)? {
                return Ok(());
            }
        }
    }

    /// A scan of the Library Folders as they are now, to run with [`Scan::run`] while the
    /// Library goes on answering, then hand to [`Library::finish_scan`].
    pub fn scan(&self) -> Scan {
        self.scan_of(self.record.folders.clone())
    }

    /// A scan of the Library Folders with `folder` added, or `None` if it is one already.
    /// Fails if `folder` cannot be read.
    pub fn scan_with_folder(&self, folder: &Path) -> Result<Option<Scan>> {
        readable(folder)?;
        if self.record.folders.iter().any(|known| known == folder) {
            return Ok(None);
        }
        let mut folders = self.record.folders.clone();
        folders.push(folder.to_owned());
        Ok(Some(self.scan_of(folders)))
    }

    /// A scan of the Library Folders with `folder` taken out.
    pub fn scan_without_folder(&self, folder: &Path) -> Scan {
        let mut folders = self.record.folders.clone();
        folders.retain(|known| known != folder);
        self.scan_of(folders)
    }

    fn scan_of(&self, folders: Vec<PathBuf>) -> Scan {
        Scan {
            library: self.instance,
            generation: self.generation,
            folders,
            formats: self.formats.clone(),
            registrations: self.registrations.clone(),
        }
    }

    /// Puts what a scan found in place: its Library Folders become the Library's, and
    /// its Dictionaries replace those found before, keeping their place, enabled state
    /// and index. Returns `false`, changing nothing, if the Library changed while the
    /// scan ran (another scan finished first, a code was entered); scan again then.
    pub fn finish_scan(&mut self, scanned: Scanned) -> Result<bool> {
        if scanned.library != self.instance || scanned.generation != self.generation {
            return Ok(false);
        }
        self.generation += 1;
        let mut known = std::mem::take(&mut self.dictionaries);
        let mut dictionaries = Vec::with_capacity(scanned.opened.len());
        for opened in scanned.opened {
            let mut info = opened.info;
            info.enabled = !self.record.disabled.contains(&info.id);
            let index = self.known_index(&mut known, &info.id, &opened.stamp);
            if index.is_some() {
                info.index = IndexState::Ready;
            }
            dictionaries.push(Dictionary {
                info,
                reader: opened.reader,
                stamp: opened.stamp,
                index,
            });
        }

        // Library order: known Dictionaries where the record puts them, newcomers after
        // them in the order they were found.
        let position = |id: &str| self.record.order.iter().position(|known| known == id);
        dictionaries.sort_by_key(|dictionary| position(&dictionary.info.id).unwrap_or(usize::MAX));

        let folders_changed = self.record.folders != scanned.folders;
        self.record.folders = scanned.folders;
        self.folders = scanned.found_folders;
        self.dictionaries = dictionaries;
        self.failures = scanned.failures;

        if self.record_order() || folders_changed {
            self.save()?;
        }
        if self.index_folder.is_none() {
            self.build_indexes();
        }
        Ok(true)
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
        self.generation += 1;
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

    /// The reader's Groups, in the order they were created.
    pub fn groups(&self) -> Vec<Group> {
        self.record
            .groups
            .iter()
            .map(|group| Group {
                id: group.id.clone(),
                name: group.name.clone(),
                dictionaries: group
                    .dictionaries
                    .iter()
                    .filter(|id| self.dictionaries.iter().any(|d| d.info.id == **id))
                    .cloned()
                    .collect(),
            })
            .collect()
    }

    /// Creates an empty Group named `name` and returns its key. The name must not be
    /// blank or the name of another Group.
    pub fn create_group(&mut self, name: &str) -> Result<String> {
        let name = self.group_name(name, None)?;
        let next = self
            .record
            .groups
            .iter()
            .filter_map(|group| group.id.strip_prefix('g')?.parse::<u64>().ok())
            .max()
            .map_or(1, |last| last + 1);
        let id = format!("g{next}");
        self.record.groups.push(GroupRecord {
            id: id.clone(),
            name,
            dictionaries: Vec::new(),
        });
        self.save()?;
        Ok(id)
    }

    /// Renames a Group. The name must not be blank or the name of another Group.
    pub fn rename_group(&mut self, id: &str, name: &str) -> Result<()> {
        let name = self.group_name(name, Some(id))?;
        self.group_mut(id)?.name = name;
        self.save()
    }

    /// Deletes a Group. Its Dictionaries stay in the Library. When it was the Current
    /// Group, Lookups run against the whole Library again.
    pub fn delete_group(&mut self, id: &str) -> Result<()> {
        self.group_mut(id)?;
        self.record.groups.retain(|group| group.id != id);
        if self.record.current_group.as_deref() == Some(id) {
            self.record.current_group = None;
        }
        self.save()
    }

    /// Puts a Dictionary into a Group (at the end of its order) or takes it out. Putting
    /// in a member, or taking out a non-member, changes nothing.
    pub fn set_in_group(&mut self, group: &str, dictionary: &str, member: bool) -> Result<()> {
        if !self.dictionaries.iter().any(|d| d.info.id == dictionary) {
            return Err(Error::UnknownDictionary(dictionary.to_owned()));
        }
        let members = &mut self.group_mut(group)?.dictionaries;
        let present = members.iter().any(|id| id == dictionary);
        if member && !present {
            members.push(dictionary.to_owned());
        } else if !member && present {
            members.retain(|id| id != dictionary);
        }
        self.save()
    }

    /// Moves a Dictionary one place up or down in a Group's order. Moving the first one up
    /// or the last one down changes nothing; nor does it change Library order.
    pub fn move_in_group(&mut self, group: &str, dictionary: &str, step: Move) -> Result<()> {
        let found: Vec<String> = self
            .dictionaries
            .iter()
            .map(|d| d.info.id.clone())
            .collect();
        let members = &mut self.group_mut(group)?.dictionaries;
        // Places in the record of the members found now: absent ones are stepped over.
        let places: Vec<usize> = (0..members.len())
            .filter(|&at| found.contains(&members[at]))
            .collect();
        let from = places
            .iter()
            .position(|&at| members[at] == dictionary)
            .ok_or_else(|| Error::UnknownDictionary(dictionary.to_owned()))?;
        let to = match step {
            Move::Up => from.checked_sub(1),
            Move::Down => Some(from + 1).filter(|&to| to < places.len()),
        };
        let Some(to) = to else {
            return Ok(());
        };
        members.swap(places[from], places[to]);
        self.save()
    }

    /// The key of the Current Group, or `None` when Lookups run against the whole Library.
    pub fn current_group(&self) -> Option<&str> {
        self.record.current_group.as_deref()
    }

    /// Makes `group` the Current Group (`None`: the whole Library). Every Lookup after
    /// this runs against it, in its order.
    pub fn set_current_group(&mut self, group: Option<&str>) -> Result<()> {
        if let Some(id) = group {
            self.group_mut(id)?;
        }
        self.record.current_group = group.map(str::to_owned);
        self.save()
    }

    /// Another Group that has what the reader typed, for when the Current Group does not:
    /// the first of the other Groups, in the order they were created, whose Dictionaries
    /// file the query, its base form or a headword starting with it; failing those, the
    /// whole Library. `None` when none has it, and for a pattern or an empty query.
    pub fn elsewhere(&self, query: &str) -> Result<Option<Elsewhere>> {
        let key = index_key(query.trim());
        if key.is_empty() || Pattern::parse(&key).is_some() {
            return Ok(None);
        }
        let current = self.current_group();
        let candidates = self
            .record
            .groups
            .iter()
            .map(|group| Some(group.id.as_str()))
            .chain(std::iter::once(None))
            .filter(|&group| group != current);
        for group in candidates {
            let answering = self.answering_in(group);
            if answering.dictionaries.is_empty() {
                continue;
            }
            if let Some(headword) = answering.offer(&key)? {
                return Ok(Some(Elsewhere {
                    group: group.map(str::to_owned),
                    headword,
                }));
            }
        }
        Ok(None)
    }

    /// The Dictionaries Lookups run against now: the Current Group's.
    fn answering(&self) -> Answering<'_> {
        self.answering_in(self.current_group())
    }

    /// The enabled Dictionaries of `group` in its order, or of the whole Library in
    /// Library order. A Group that no longer exists has none.
    fn answering_in(&self, group: Option<&str>) -> Answering<'_> {
        let enabled = |id: &str| {
            self.dictionaries
                .iter()
                .find(|d| d.info.id == id && d.info.enabled)
        };
        let dictionaries = match group {
            None => self
                .dictionaries
                .iter()
                .filter(|d| d.info.enabled)
                .collect(),
            Some(group) => self
                .record
                .groups
                .iter()
                .find(|known| known.id == group)
                .map(|known| {
                    known
                        .dictionaries
                        .iter()
                        .filter_map(|id| enabled(id))
                        .collect()
                })
                .unwrap_or_default(),
        };
        Answering { dictionaries }
    }

    fn group_mut(&mut self, id: &str) -> Result<&mut GroupRecord> {
        self.record
            .groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or_else(|| Error::UnknownGroup(id.to_owned()))
    }

    /// `name` trimmed, if it is not blank and no Group but `renaming` has it.
    fn group_name(&self, name: &str, renaming: Option<&str>) -> Result<String> {
        let name = name.trim();
        let taken = self
            .record
            .groups
            .iter()
            .any(|group| group.name == name && Some(group.id.as_str()) != renaming);
        if name.is_empty() || taken {
            return Err(Error::GroupName(name.to_owned()));
        }
        Ok(name.to_owned())
    }

    /// The Entries for exactly `headword`, one per enabled Dictionary in the Current Group
    /// that has it, in Current Group order. Redirects (`@@@LINK=`) are followed; several records under the
    /// headword in one Dictionary are kept together in one Entry.
    pub fn look_up(&self, headword: &str) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for dictionary in self.answering().dictionaries {
            entries.extend(dictionary.entry(headword)?);
        }
        Ok(entries)
    }

    /// Up to `limit` suggestions for `query`, from the Current Group: the headwords whose
    /// key starts with the query's, then the headwords that follow them in index order,
    /// like a page of a printed index. A query with wildcards (`*` for any run of
    /// characters, `?` for one) offers only its matches. An empty query browses the index
    /// from its start. Each suggestion counts the Dictionaries in the Current Group that
    /// file it, and a base form is offered only when the Current Group files it.
    pub fn suggest(&self, query: &str, limit: usize) -> Result<Suggestions> {
        self.answering().suggest(query, limit)
    }

    /// Up to `limit` headwords that follow `headword` in index order, from the Current
    /// Group: the column continued as the reader scrolls.
    pub fn headwords_after(&self, headword: &str, limit: usize) -> Result<Vec<Suggestion>> {
        self.answering().headwords_after(headword, limit)
    }

    /// Up to `limit` more suggestions for `query`, after `headword`: the column continued as
    /// the reader scrolls. A pattern's column continues with its matches; any other column
    /// continues through the index.
    pub fn suggest_after(
        &self,
        query: &str,
        headword: &str,
        limit: usize,
    ) -> Result<Vec<Suggestion>> {
        self.answering().suggest_after(query, headword, limit)
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

/// The Dictionaries a Lookup runs against: the enabled Dictionaries of one Group in that
/// Group's order, or of the whole Library in Library order.
struct Answering<'a> {
    dictionaries: Vec<&'a Dictionary>,
}

impl Answering<'_> {
    /// The headword these Dictionaries offer first for the query `key`, as the column
    /// would: the query itself when it is a headword, else its base form, else the first
    /// headword starting with it. Close spellings are not looked for.
    fn offer(&self, key: &str) -> Result<Option<String>> {
        let first = self.merged(Start::AtKey(key), 1)?.into_iter().next();
        if let Some((headword, _)) = first.as_ref().filter(|(h, _)| h.key == key) {
            return Ok(Some(headword.headword.clone()));
        }
        if let Some((base, _)) = self.base_headword(key, false)? {
            return Ok(Some(base.headword));
        }
        Ok(first
            .filter(|(h, _)| h.key.starts_with(key))
            .map(|(h, _)| h.headword))
    }

    /// Up to `limit` suggestions for `query`, from enabled Dictionaries: the headwords
    /// whose key starts with the query's, then the headwords that follow them in index
    /// order, like a page of a printed index. A query with wildcards (`*` for any run of
    /// characters, `?` for one) offers only its matches. An empty query browses the index
    /// from its start.
    fn suggest(&self, query: &str, limit: usize) -> Result<Suggestions> {
        let typed = query.trim();
        let key = index_key(typed);
        // A pattern offers only its matches; Lemmatization does not apply to it.
        if let Some(pattern) = Pattern::parse(&key) {
            return self.suggest_pattern(&pattern, limit);
        }
        let mut items = self.merged(Start::AtKey(&key), limit)?;
        let browse = key.is_empty();
        let matched = items
            .first()
            .is_some_and(|(first, _)| first.key.starts_with(&key));
        let mut lemma = None;
        if !browse && limit > 0 {
            // A query that is itself a headword stays first; its base form follows it.
            let is_headword = items.first().is_some_and(|(first, _)| first.key == key);
            if let Some(base) = self.base_headword(&key, is_headword)? {
                lemma = Some(Lemma {
                    from: typed.to_owned(),
                    to: base.0.headword.clone(),
                });
                if matched {
                    items.retain(|(headword, _)| *headword != base.0);
                    items.insert(usize::from(is_headword), base);
                    items.truncate(limit);
                } else {
                    // Nothing starts with the query: the column reads on from the base form.
                    let after = self.merged(Start::After(&base.0), limit - 1)?;
                    items = std::iter::once(base).chain(after).collect();
                }
            }
        }
        // No headword starts with the query and it has no base form filed: offer close
        // spellings, and failing those, the index from where the query would stand.
        let unanswered = !browse && !matched && lemma.is_none();
        let mut near = false;
        if unanswered {
            let close = self.near(&key, limit)?;
            if !close.is_empty() {
                items = close;
                near = true;
            }
        }
        Ok(Suggestions {
            items: items.into_iter().map(suggestion).collect(),
            lemma,
            near,
            miss: unanswered && !near,
            browse,
        })
    }

    /// Up to `limit` headwords spelt nearly like `key`, nearest first, then in index order,
    /// each with how many enabled Dictionaries file it.
    fn near(&self, key: &str, limit: usize) -> Result<Vec<(IndexedHeadword, usize)>> {
        let query: Vec<char> = key.chars().collect();
        let tolerance = near::tolerance(&query);
        let distance = |headword: &IndexedHeadword| {
            let candidate: Vec<char> = headword.key.chars().collect();
            near::distance(&query, &candidate, tolerance)
        };
        let close = |headword: &IndexedHeadword| match distance(headword) {
            Some(_) => Pass::Take,
            None => Pass::Skip,
        };
        let counts = self.counted(Start::AtKey(""), usize::MAX, &close)?;
        let mut found: Vec<(usize, IndexedHeadword, usize)> = counts
            .into_iter()
            .filter_map(|(headword, count)| Some((distance(&headword)?, headword, count)))
            .collect();
        found.sort();
        Ok(found
            .into_iter()
            .take(limit)
            .map(|(_, headword, count)| (headword, count))
            .collect())
    }

    fn suggest_pattern(&self, pattern: &Pattern, limit: usize) -> Result<Suggestions> {
        let prefix = pattern.prefix();
        let items = self.merged_where(Start::AtKey(prefix), limit, matching(pattern))?;
        let miss = items.is_empty();
        // Nothing matches: the index from where the pattern's letters would stand.
        let items = if miss {
            self.merged(Start::AtKey(prefix), limit)?
        } else {
            items
        };
        Ok(Suggestions {
            items: items.into_iter().map(suggestion).collect(),
            lemma: None,
            near: false,
            miss,
            browse: false,
        })
    }

    /// The headword of the most likely base form of the query `key` that an enabled
    /// Dictionary files, with how many of them file it: Lemmatization.
    fn base_headword(
        &self,
        key: &str,
        query_is_headword: bool,
    ) -> Result<Option<(IndexedHeadword, usize)>> {
        for base in lemma::base_forms(key) {
            if base.doubtful && query_is_headword {
                continue;
            }
            let base_key = index_key(&base.form);
            if base_key.is_empty() || base_key == key {
                continue;
            }
            let first = self.merged(Start::AtKey(&base_key), 1)?.into_iter().next();
            if let Some(found) = first.filter(|(headword, _)| headword.key == base_key) {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Up to `limit` headwords that follow `headword` in index order, from enabled
    /// Dictionaries: the column continued as the reader scrolls.
    fn headwords_after(&self, headword: &str, limit: usize) -> Result<Vec<Suggestion>> {
        let after = IndexedHeadword::new(headword);
        Ok(self
            .merged(Start::After(&after), limit)?
            .into_iter()
            .map(suggestion)
            .collect())
    }

    /// Up to `limit` more suggestions for `query`, after `headword`: the column continued as
    /// the reader scrolls. A pattern's column continues with its matches; any other column
    /// continues through the index.
    fn suggest_after(&self, query: &str, headword: &str, limit: usize) -> Result<Vec<Suggestion>> {
        let after = IndexedHeadword::new(headword);
        let items = match Pattern::parse(&index_key(query.trim())) {
            Some(pattern) if pattern.matches(&after.key) => {
                self.merged_where(Start::After(&after), limit, matching(&pattern))?
            }
            _ => self.merged(Start::After(&after), limit)?,
        };
        Ok(items.into_iter().map(suggestion).collect())
    }

    /// The first `limit` headwords from `start` across enabled Dictionaries, each once,
    /// with how many of them file it.
    fn merged(&self, start: Start<'_>, limit: usize) -> Result<Vec<(IndexedHeadword, usize)>> {
        self.merged_where(start, limit, |_| Pass::Take)
    }

    /// Like [`Library::merged`], keeping only the headwords `scan` takes and reading each
    /// index only until `scan` says stop.
    fn merged_where(
        &self,
        start: Start<'_>,
        limit: usize,
        scan: impl Fn(&IndexedHeadword) -> Pass,
    ) -> Result<Vec<(IndexedHeadword, usize)>> {
        // Each headword among the first `limit` overall is among the first `limit` of
        // every Dictionary that files it, so `limit` from each is enough to count it.
        let counts = self.counted(start, limit, &scan)?;
        Ok(counts.into_iter().take(limit).collect())
    }

    /// Up to `limit` headwords `scan` takes from each enabled Dictionary, from `start`,
    /// with how many of them file each.
    fn counted(
        &self,
        start: Start<'_>,
        limit: usize,
        scan: &dyn Fn(&IndexedHeadword) -> Pass,
    ) -> Result<BTreeMap<IndexedHeadword, usize>> {
        let mut counts: BTreeMap<IndexedHeadword, usize> = BTreeMap::new();
        for dictionary in &self.dictionaries {
            let headwords =
                dictionary
                    .scan(start, limit, scan)
                    .map_err(|error| Error::Dictionary {
                        dictionary: dictionary.info.title.clone(),
                        reason: error.to_string(),
                    })?;
            for headword in headwords {
                *counts.entry(headword).or_default() += 1;
            }
        }
        Ok(counts)
    }
}

/// A scan for `pattern`'s matches. They all start with its prefix, so the scan stops at
/// the first headword that does not.
fn matching(pattern: &Pattern) -> impl Fn(&IndexedHeadword) -> Pass {
    move |headword| {
        if !headword.key.starts_with(pattern.prefix()) {
            Pass::Stop
        } else if pattern.matches(&headword.key) {
            Pass::Take
        } else {
            Pass::Skip
        }
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

/// What the Library shows of a Dictionary just opened; enabled until the Library's
/// record says otherwise.
fn describe(id: String, unit: &Unit, reader: &dyn DictionaryReader) -> DictionaryInfo {
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
        enabled: true,
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
