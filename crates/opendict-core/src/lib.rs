//! OpenDict's core: the Library and Lookup, with no user interface and no network access.
//!
//! The core reads Dictionaries through the [`DictionaryFormat`] interface, so Lookup
//! never depends on a particular file format.

mod error;
mod format;
mod library;
mod scan;

pub use error::{Error, Result};
pub use format::{
    DictionaryFormat, DictionaryReader, FormatError, MdictFormat, OpenError, RawRecord,
};
pub use library::{DictionaryInfo, Entry, Library, LibraryFolder, Move, OpenFailure};
