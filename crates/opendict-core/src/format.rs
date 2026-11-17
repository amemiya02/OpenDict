//! The dictionary-format interface the core programs against.
//!
//! Lookup never names a file format. Each format (MDict now; StarDict or DSL later)
//! implements [`DictionaryFormat`] to recognise and open its files, and the opened file
//! answers through [`DictionaryReader`].

use std::ops::ControlFlow;
use std::path::Path;

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
        Ok(Box::new(MdictReader { mdx }))
    }
}

struct MdictReader {
    mdx: opendict_mdict::LazyMdx,
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
