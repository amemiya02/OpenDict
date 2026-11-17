//! OpenDict's core: the Library and Lookup, with no user interface and no network access.
//!
//! The core reads Dictionaries through the [`DictionaryFormat`] interface, so Lookup
//! never depends on a particular file format. Each Dictionary is indexed once (see
//! [`Library::index_jobs`]); Lookup and the headword column read the index.

mod data_directory;
mod error;
mod format;
mod history;
mod index;
mod indexing;
mod lemma;
mod library;
mod links;
mod near;
mod normalization;
mod pattern;
mod preferences;
mod pronunciation;
mod registrations;
mod scan;
mod word_books;

pub use data_directory::{DataDirectory, MoveProgress, Relocation};
pub use error::{Error, Result};
pub use format::{
    DictionaryFormat, DictionaryReader, FormatError, Location, MdictFormat, OpenError, RawRecord,
    Registration,
};
pub use history::History;
pub use index::{HeadwordIndex, IndexError, IndexedHeadword, Start, index_key};
pub use indexing::{BuiltIndex, IndexJob, IndexOutcome, IndexResult, IndexState};
pub use lemma::{BaseForm, base_forms};
pub use library::{
    DictionaryInfo, Entry, Lemma, Library, LibraryFolder, Move, OpenFailure, Suggestion,
    Suggestions,
};
pub use preferences::Preferences;
pub use pronunciation::pronunciation;
pub use word_books::{ExportFormat, SavedFrom, SavedWord, WordBook, WordBookId, WordBooks};
