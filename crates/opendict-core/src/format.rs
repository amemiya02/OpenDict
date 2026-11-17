//! The dictionary-format interface the core programs against.
//!
//! Lookup never names a file format. Each format (MDict now; StarDict or DSL later)
//! implements [`DictionaryFormat`] to recognise and open its files, and the opened file
//! answers through [`DictionaryReader`].

use std::ops::ControlFlow;
use std::path::{Component, Path, PathBuf};

/// A dictionary file format the Library can read.
pub trait DictionaryFormat: Send + Sync {
    /// Whether `path` is the main file of a Dictionary in this format.
    fn recognises(&self, path: &Path) -> bool;

    /// Opens the Dictionary whose main file is `path`.
    fn open(&self, path: &Path) -> Result<Box<dyn DictionaryReader>, OpenError>;
}

/// An open Dictionary, as a format presents it to the core.
///
/// Opening should be cheap: the core keeps its own index of every Dictionary's headwords
/// (built once with [`DictionaryReader::headwords`]) and reads records by where they lie.
pub trait DictionaryReader: Send + Sync {
    /// The Dictionary's title as its file states it; may be empty.
    fn title(&self) -> &str;

    /// How many Entries (headwords, redirects included) the Dictionary files.
    fn entry_count(&self) -> usize;

    /// Reads every headword the Dictionary files, in the file's order, with where its
    /// record lies; a headword filed more than once appears more than once.
    ///
    /// Reports how many headwords have been read so far to `progress` as it goes (at
    /// least once before the end, and with the total at the end). Stops and returns
    /// `Ok(None)` as soon as `progress` returns [`ControlFlow::Break`].
    fn headwords(
        &self,
        progress: &mut dyn FnMut(usize) -> ControlFlow<()>,
    ) -> Result<Option<Vec<(String, Location)>>, FormatError>;

    /// The record at `location`, as [`DictionaryReader::headwords`] reported it.
    fn record(&self, location: Location) -> Result<RawRecord, FormatError>;

    /// The bytes of a file the Dictionary's Entries refer to (an image, a stylesheet, a
    /// sound), by the relative path an Entry uses, or `None` if the Dictionary has no such
    /// file. Must never return anything from outside the Dictionary.
    fn resource(&self, path: &str) -> Result<Option<Vec<u8>>, FormatError> {
        let _ = path;
        Ok(None)
    }
}

/// Where a record lies in a Dictionary's file: two numbers whose meaning is the format's
/// own. The core stores them in its index and hands them back unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub start: u64,
    pub end: u64,
}

/// One record as a format returns it, before the core resolves redirects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawRecord {
    /// Entry content (HTML).
    Text(String),
    /// A redirect to another headword in the same Dictionary.
    Link(String),
}

/// A format could not read a Dictionary. The message is meant for the reader.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct FormatError(pub String);

/// Why a Dictionary could not be opened, in words meant for the reader.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OpenError {
    /// The file exists but could not be read.
    #[error("the file cannot be read: {0}")]
    Unreadable(String),
    /// The file is not a dictionary in the format its name suggests.
    #[error("this is not a dictionary file")]
    NotADictionary,
    /// The file uses a format version OpenDict does not read.
    #[error("format version {0} is not supported")]
    UnsupportedVersion(String),
    /// Part of the file is damaged.
    #[error("the file is damaged")]
    Damaged,
    /// The Dictionary is encrypted (`Encrypted=1`) and opens only with its registration
    /// code or e-mail.
    #[error("it needs a registration code")]
    NeedsRegistrationCode,
    /// The file uses a feature OpenDict does not read yet.
    #[error("it uses {0}, which OpenDict cannot read yet")]
    Unsupported(String),
}

/// MDict (`.mdx`), read with `opendict-mdict`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MdictFormat;

impl DictionaryFormat for MdictFormat {
    fn recognises(&self, path: &Path) -> bool {
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"))
    }

    fn open(&self, path: &Path) -> Result<Box<dyn DictionaryReader>, OpenError> {
        let mdx = opendict_mdict::LazyMdx::open(path).map_err(open_error)?;
        Ok(Box::new(MdictReader {
            mdx,
            resource_files: open_resource_files(path),
            folder: path.parent().map(Path::to_owned).unwrap_or_default(),
        }))
    }
}

struct MdictReader {
    mdx: opendict_mdict::LazyMdx,
    /// `name.mdd`, `name.1.mdd`, `name.2.mdd`, … in that order.
    resource_files: Vec<opendict_mdict::Mdd>,
    /// The folder the MDX is in; stylesheets and scripts beside it belong to the Dictionary.
    folder: PathBuf,
}

/// How many headwords are read between two progress reports.
const PROGRESS_STEP: usize = 4096;

impl DictionaryReader for MdictReader {
    fn title(&self) -> &str {
        self.mdx.header().title()
    }

    fn entry_count(&self) -> usize {
        self.mdx.key_count()
    }

    fn headwords(
        &self,
        progress: &mut dyn FnMut(usize) -> ControlFlow<()>,
    ) -> Result<Option<Vec<(String, Location)>>, FormatError> {
        let mut keys = Vec::with_capacity(self.mdx.key_count().min(1 << 22));
        if progress(0).is_break() {
            return Ok(None);
        }
        for key in self.mdx.keys() {
            keys.push(key.map_err(mdict_error)?);
            if keys.len() % PROGRESS_STEP == 0 && progress(keys.len()).is_break() {
                return Ok(None);
            }
        }
        // A record ends where the next larger start begins, or at the end of the stream.
        let mut starts: Vec<u64> = keys.iter().map(|(_, start)| *start).collect();
        starts.sort_unstable();
        starts.dedup();
        let stream_end = self.mdx.record_stream_len();
        let headwords = keys
            .into_iter()
            .map(|(headword, start)| {
                let next = starts.partition_point(|&s| s <= start);
                let end = starts.get(next).copied().unwrap_or(stream_end);
                (headword, Location { start, end })
            })
            .collect::<Vec<_>>();
        if progress(headwords.len()).is_break() {
            return Ok(None);
        }
        Ok(Some(headwords))
    }

    fn record(&self, location: Location) -> Result<RawRecord, FormatError> {
        let record = self
            .mdx
            .record(location.start..location.end)
            .map_err(mdict_error)?;
        Ok(match record {
            opendict_mdict::Record::Text(text) => RawRecord::Text(text),
            opendict_mdict::Record::Link(target) => RawRecord::Link(target),
        })
    }

    /// A file beside the MDX first (so a loose stylesheet can stand in for the one in the
    /// MDD), then each MDD in order.
    fn resource(&self, path: &str) -> Result<Option<Vec<u8>>, FormatError> {
        let Some(relative) = contained_path(path) else {
            return Ok(None);
        };
        if is_loose_resource(&relative) {
            match std::fs::read(self.folder.join(&relative)) {
                Ok(bytes) => return Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(FormatError(error.to_string())),
            }
        }
        let key = relative.to_string_lossy();
        for mdd in &self.resource_files {
            if let Some(bytes) = mdd.resource(&key).map_err(mdict_error)? {
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }
}

/// The MDD files that go with the MDX at `path`. One that cannot be read is skipped: the
/// Entries still open, without its resources.
fn open_resource_files(path: &Path) -> Vec<opendict_mdict::Mdd> {
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let names =
        std::iter::once(format!("{stem}.mdd")).chain((1..).map(|n| format!("{stem}.{n}.mdd")));
    names
        .map(|name| path.with_file_name(name))
        .take_while(|candidate| candidate.is_file())
        .filter_map(|candidate| opendict_mdict::Mdd::open(candidate).ok())
        .collect()
}

/// `path` as a relative path made only of plain names, or `None` if it could reach
/// outside the Dictionary's folder (`..`, a root, a drive prefix).
fn contained_path(path: &str) -> Option<PathBuf> {
    let mut relative = PathBuf::new();
    for part in path.split(['/', '\\']) {
        if part.is_empty() && relative.as_os_str().is_empty() {
            continue; // a leading separator: MDD keys start with one
        }
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(name)), None) if !part.contains(':') => relative.push(name),
            _ => return None,
        }
    }
    (!relative.as_os_str().is_empty()).then_some(relative)
}

/// File types served from beside the MDX. Never the dictionary files themselves, nor
/// documents that could stand in for an Entry.
fn is_loose_resource(path: &Path) -> bool {
    const TYPES: &[&str] = &[
        "css", "js", "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "ttf", "otf", "woff",
        "woff2", "mp3", "ogg", "oga", "spx", "wav", "m4a", "aac",
    ];
    path.extension()
        .is_some_and(|ext| TYPES.iter().any(|t| ext.eq_ignore_ascii_case(t)))
}

fn open_error(error: opendict_mdict::Error) -> OpenError {
    use opendict_mdict::Error;
    match error {
        Error::Io(error) => OpenError::Unreadable(error.to_string()),
        Error::NotMdict(_) => OpenError::NotADictionary,
        Error::UnsupportedVersion(version) => OpenError::UnsupportedVersion(version),
        Error::Corrupt(_) => OpenError::Damaged,
        Error::Encrypted => OpenError::NeedsRegistrationCode,
        Error::Unsupported(feature) => OpenError::Unsupported(feature),
        other => OpenError::Unsupported(other.to_string()),
    }
}

fn mdict_error(error: opendict_mdict::Error) -> FormatError {
    FormatError(error.to_string())
}
