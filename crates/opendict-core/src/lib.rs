//! OpenDict's core: the Library and Lookup, with no user interface and no network access.
//!
//! The core reads Dictionaries through the [`DictionaryFormat`] interface, so Lookup
//! never depends on a particular file format. Each Dictionary is indexed once (see
//! [`Library::index_jobs`]); Lookup and the headword column read the index.

mod error;
mod format;
mod index;
mod indexing;
mod library;
mod preferences;
mod scan;

pub use error::{Error, Result};
pub use format::{
    DictionaryFormat, DictionaryReader, FormatError, Location, MdictFormat, OpenError, RawRecord,
};
pub use index::{HeadwordIndex, IndexError, IndexedHeadword, Start, index_key};
pub use indexing::{BuiltIndex, IndexJob, IndexOutcome, IndexResult, IndexState};
pub use library::{DictionaryInfo, Entry, Library, LibraryFolder, Move, OpenFailure};
pub use preferences::Preferences;
