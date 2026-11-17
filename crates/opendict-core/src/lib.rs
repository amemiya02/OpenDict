//! OpenDict's core: the Library and Lookup, with no user interface and no network access.
//!
//! The core reads Dictionaries through the [`DictionaryFormat`] interface, so Lookup
//! never depends on a particular file format.

mod error;
mod format;
mod index;
mod library;
mod preferences;
mod scan;

pub use error::{Error, Result};
pub use format::{
    DictionaryFormat, DictionaryReader, FormatError, MdictFormat, OpenError, RawRecord,
};
pub use index::{HeadwordIndex, IndexError, IndexedHeadword, MemoryIndex, Start, index_key};
pub use library::{
    DictionaryInfo, Entry, Lemma, Library, LibraryFolder, Move, OpenFailure, Suggestion,
    Suggestions,
};
pub use preferences::Preferences;
