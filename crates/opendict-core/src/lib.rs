//! OpenDict's core: the Library and Lookup, with no user interface and no network access.
//!
//! The core reads Dictionaries through the [`DictionaryFormat`] interface, so Lookup
//! never depends on a particular file format.

mod config;
mod error;
mod format;
mod library;
mod preferences;

pub use config::Config;
pub use error::{Error, Result};
pub use format::{DictionaryFormat, DictionaryReader, FormatError, MdictFormat, RawRecord};
pub use library::{DictionaryInfo, Entry, Library, OpenFailure};
pub use preferences::Preferences;
