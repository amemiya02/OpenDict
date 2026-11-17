use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::format::{DictionaryFormat, DictionaryReader, MdictFormat, OpenError, RawRecord};
use crate::index::{HeadwordIndex, IndexError, IndexedHeadword, MemoryIndex, Start, index_key};
use crate::near;
use crate::pattern::Pattern;
use crate::scan::{self, Unit};

/// Every Dictionary the reader has made available: the Dictionaries found in their
/// Library Folders, in Library order, each enabled or not.
///
/// A Library loaded from a file ([`Library::load`]) records its Library Folders, order
/// and enabled states there after every change. Dictionary files are only ever read.
pub struct Library {
    file: Option<PathBuf>,
    formats: Vec<&'static dyn DictionaryFormat>,
    record: Record,
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
    /// Built on the first query that needs it, so opening the Library stays quick.
    index: OnceLock<Box<dyn HeadwordIndex>>,
}

impl Dictionary {
    fn new(info: DictionaryInfo, reader: Box<dyn DictionaryReader>) -> Dictionary {
        Dictionary {
            info,
            reader,
            index: OnceLock::new(),
        }
    }

    fn index(&self) -> &dyn HeadwordIndex {
        self.index
            .get_or_init(|| Box::new(MemoryIndex::build(self.reader.as_ref())))
            .as_ref()
    }

    /// Up to `limit` headwords from `start` in index order that `scan` takes, reading the
    /// index a page at a time until `scan` says stop or the index ends.
    fn scan(
        &self,
        start: Start<'_>,
        limit: usize,
        scan: &dyn Fn(&IndexedHeadword) -> Scan,
    ) -> std::result::Result<Vec<IndexedHeadword>, IndexError> {
        let index = self.index();
        let page_size = limit.clamp(SCAN_PAGE, 16 * SCAN_PAGE);
        let mut taken = Vec::new();
        let mut page = index.headwords(start, page_size)?;
        while taken.len() < limit {
            let full = page.len() == page_size;
            for headword in &page {
                match scan(headword) {
                    Scan::Take => taken.push(headword.clone()),
                    Scan::Skip => {}
                    Scan::Stop => return Ok(taken),
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
enum Scan {
    /// Keep it and read on.
    Take,
    /// Pass it over and read on.
    Skip,
    /// Read no further.
    Stop,
}

/// How many headwords a scan reads from an index at least at a time.
const SCAN_PAGE: usize = 256;

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
                match format.open(path) {
                    Ok(reader) => {
                        let info = describe(id, &unit, reader.as_ref(), &self.record);
                        dictionaries.push(Dictionary::new(info, reader));
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
    /// order, like a page of a printed index. A query with wildcards (`*` for any run of
    /// characters, `?` for one) offers only its matches. An empty query browses the index
    /// from its start.
    pub fn suggest(&self, query: &str, limit: usize) -> Result<Suggestions> {
        let key = index_key(query.trim());
        if let Some(pattern) = Pattern::parse(&key) {
            return self.suggest_pattern(&pattern, limit);
        }
        let items = self.merged(Start::AtKey(&key), limit)?;
        let browse = key.is_empty();
        let matched = browse
            || items
                .first()
                .is_some_and(|(first, _)| first.key.starts_with(&key));
        // No headword starts with the query: offer close spellings, and failing those,
        // the index from where the query would stand.
        let close = if matched {
            Vec::new()
        } else {
            self.near(&key, limit)?
        };
        let near = !close.is_empty();
        let items = if near { close } else { items };
        Ok(Suggestions {
            items: items.into_iter().map(suggestion).collect(),
            lemma: None,
            near,
            miss: !matched && !near,
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
            Some(_) => Scan::Take,
            None => Scan::Skip,
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

    /// Up to `limit` more suggestions for `query`, after `headword`: the column continued as
    /// the reader scrolls. A pattern's column continues with its matches; any other column
    /// continues through the index.
    pub fn suggest_after(
        &self,
        query: &str,
        headword: &str,
        limit: usize,
    ) -> Result<Vec<Suggestion>> {
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
        self.merged_where(start, limit, |_| Scan::Take)
    }

    /// Like [`Library::merged`], keeping only the headwords `scan` takes and reading each
    /// index only until `scan` says stop.
    fn merged_where(
        &self,
        start: Start<'_>,
        limit: usize,
        scan: impl Fn(&IndexedHeadword) -> Scan,
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
        scan: &dyn Fn(&IndexedHeadword) -> Scan,
    ) -> Result<BTreeMap<IndexedHeadword, usize>> {
        let mut counts: BTreeMap<IndexedHeadword, usize> = BTreeMap::new();
        for dictionary in self.dictionaries.iter().filter(|d| d.info.enabled) {
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

/// A scan for `pattern`'s matches. They all start with its prefix, so the scan stops at
/// the first headword that does not.
fn matching(pattern: &Pattern) -> impl Fn(&IndexedHeadword) -> Scan {
    move |headword| {
        if !headword.key.starts_with(pattern.prefix()) {
            Scan::Stop
        } else if pattern.matches(&headword.key) {
            Scan::Take
        } else {
            Scan::Skip
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
    fn entry(&self, headword: &str) -> Result<Option<Entry>> {
        let records = resolve(self.reader.as_ref(), headword).map_err(|e| self.error(e))?;
        Ok((!records.is_empty()).then(|| Entry {
            dictionary_id: self.info.id.clone(),
            dictionary_title: self.info.title.clone(),
            html: records.join("\n"),
        }))
    }

    fn error(&self, error: crate::FormatError) -> Error {
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
