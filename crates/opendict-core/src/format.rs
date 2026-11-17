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
    fn open(&self, path: &Path) -> Result<Box<dyn DictionaryReader>, FormatError>;
}

/// An open Dictionary, as a format presents it to the core.
pub trait DictionaryReader: Send + Sync {
    /// The Dictionary's title as its file states it; may be empty.
    fn title(&self) -> &str;

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

/// MDict (`.mdx`), read with `opendict-mdict`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MdictFormat;

impl DictionaryFormat for MdictFormat {
    fn recognises(&self, path: &Path) -> bool {
        path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"))
    }

    fn open(&self, path: &Path) -> Result<Box<dyn DictionaryReader>, FormatError> {
        let mdx = opendict_mdict::Mdx::open(path).map_err(mdict_error)?;
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

fn mdict_error(error: opendict_mdict::Error) -> FormatError {
    FormatError(error.to_string())
}
