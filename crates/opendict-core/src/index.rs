//! Ordered headword queries: the interface Lookup reads a Dictionary's headwords through.
//!
//! Every Dictionary's headwords are kept in index order: by [`index_key`], then by the
//! headword itself. Lookup asks a [`HeadwordIndex`] for the headwords from a point in
//! that order, so the headword column can read like a printed index. [`MemoryIndex`]
//! builds the order in memory from the Dictionary's file; a persistent index can stand
//! behind the same interface.

use crate::format::DictionaryReader;
use crate::normalization::normalize;

/// The key a headword is filed under in index order: its [`normalize`]d form. The same
/// function orders the index and places a query in it.
pub fn index_key(headword: &str) -> String {
    normalize(headword)
}

/// One headword in index order, with the key it is filed under.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedHeadword {
    /// [`index_key`] of the headword. Index order compares this first.
    pub key: String,
    /// The headword exactly as the Dictionary files it.
    pub headword: String,
}

impl IndexedHeadword {
    pub fn new(headword: &str) -> IndexedHeadword {
        IndexedHeadword {
            key: index_key(headword),
            headword: headword.to_owned(),
        }
    }
}

/// Where in index order a query starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start<'a> {
    /// At the first headword whose key is not before this key.
    AtKey(&'a str),
    /// At the first headword after this one.
    After(&'a IndexedHeadword),
}

/// A Dictionary's headwords in index order, each headword once.
pub trait HeadwordIndex: Send + Sync {
    /// Up to `limit` headwords in index order, from `start` on.
    fn headwords(&self, start: Start<'_>, limit: usize)
    -> Result<Vec<IndexedHeadword>, IndexError>;
}

/// An index could not be read. The message is meant for the reader.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct IndexError(pub String);

/// A [`HeadwordIndex`] held in memory, built from the Dictionary's file.
pub struct MemoryIndex {
    sorted: Vec<IndexedHeadword>,
}

impl MemoryIndex {
    pub fn build(reader: &dyn DictionaryReader) -> MemoryIndex {
        let mut sorted: Vec<IndexedHeadword> =
            reader.headwords().map(IndexedHeadword::new).collect();
        sorted.sort_unstable();
        sorted.dedup();
        MemoryIndex { sorted }
    }
}

impl HeadwordIndex for MemoryIndex {
    fn headwords(
        &self,
        start: Start<'_>,
        limit: usize,
    ) -> Result<Vec<IndexedHeadword>, IndexError> {
        let first = match start {
            Start::AtKey(key) => self.sorted.partition_point(|item| item.key.as_str() < key),
            Start::After(after) => self.sorted.partition_point(|item| item <= after),
        };
        Ok(self.sorted[first..].iter().take(limit).cloned().collect())
    }
}
