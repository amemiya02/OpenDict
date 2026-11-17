//! The dictionary-format interface the core programs against.
//!
//! Lookup never names a file format. Each format (MDict now; StarDict or DSL later)
//! implements [`DictionaryFormat`] to recognise and open its files, and the opened file
//! answers through [`DictionaryReader`].

use std::path::Path;

/// A dictionary file format the Library can read.
pub trait DictionaryFormat: Send + Sync {
    /// Whether `path` is the main file of a Dictionary in this format.
    fn recognises(&self, path: &Path) -> bool;

    /// Opens the Dictionary whose main file is `path`.
    fn open(&self, path: &Path) -> Result<Box<dyn DictionaryReader>, OpenError>;
}

/// An open Dictionary, as a format presents it to the core.
pub trait DictionaryReader: Send + Sync {
    /// The Dictionary's title as its file states it; may be empty.
    fn title(&self) -> &str;

    /// How many Entries (headwords, redirects included) the Dictionary files.
    fn entry_count(&self) -> usize;

    /// Every headword the Dictionary files, in the file's order. A headword filed more
    /// than once may appear more than once.
    fn headwords(&self) -> Box<dyn Iterator<Item = &str> + '_>;

    /// The records filed under exactly `headword`, in the file's order.
    fn records(&self, headword: &str) -> Result<Vec<RawRecord>, FormatError>;
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
        let mdx = opendict_mdict::Mdx::open(path).map_err(open_error)?;
        Ok(Box::new(MdictReader { mdx }))
    }
}

struct MdictReader {
    mdx: opendict_mdict::Mdx,
}

impl DictionaryReader for MdictReader {
    fn title(&self) -> &str {
        self.mdx.header().title()
    }

    fn entry_count(&self) -> usize {
        self.mdx.headwords().count()
    }

    fn headwords(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(self.mdx.headwords())
    }

    fn records(&self, headword: &str) -> Result<Vec<RawRecord>, FormatError> {
        let records = self.mdx.lookup(headword).map_err(mdict_error)?;
        Ok(records
            .into_iter()
            .map(|record| match record {
                opendict_mdict::Record::Text(text) => RawRecord::Text(text),
                opendict_mdict::Record::Link(target) => RawRecord::Link(target),
            })
            .collect())
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
