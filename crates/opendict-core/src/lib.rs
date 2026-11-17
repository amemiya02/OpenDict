//! OpenDict's core: the Library and Lookup, with no user interface and no network access.
//!
//! The core reads Dictionaries through the [`DictionaryFormat`] interface, so Lookup
//! never depends on a particular file format.

mod error;
mod format;
mod library;
mod preferences;
mod registrations;
mod scan;

pub use error::{Error, Result};
pub use format::{
    DictionaryFormat, DictionaryReader, FormatError, MdictFormat, OpenError, RawRecord,
    Registration,
};
pub use library::{DictionaryInfo, Entry, Library, LibraryFolder, Move, OpenFailure};
pub use preferences::Preferences;
