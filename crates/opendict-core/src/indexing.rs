//! Building a Dictionary's index away from the Library: [`Library::index_jobs`] hands out
//! one [`IndexJob`] per Dictionary that needs an index, the caller runs each wherever it
//! likes (a background thread, so Lookups go on meanwhile), and gives the outcome back
//! with [`Library::finish_index`].
//!
//! [`Library::index_jobs`]: crate::Library::index_jobs
//! [`Library::finish_index`]: crate::Library::finish_index

use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;

use crate::format::DictionaryReader;
use crate::index::StoredIndex;
use crate::scan::Stamp;

/// Where a Dictionary's index stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexState {
    /// The index is built; the Dictionary answers Lookups.
    Ready,
    /// The index is not built yet (or indexing was cancelled). The Dictionary does not
    /// answer Lookups until it is.
    Pending,
    /// The index could not be built, for this reason (meant for the reader). The other
    /// Dictionaries are not affected. A rescan tries again.
    Failed(String),
}

/// The work of building one Dictionary's index. It holds what it needs, so it runs
/// without the Library.
pub struct IndexJob {
    pub(crate) dictionary_id: String,
    pub(crate) title: String,
    pub(crate) reader: Arc<dyn DictionaryReader>,
    pub(crate) stamp: Stamp,
    /// The folder stored indexes go in; `None` keeps the index in memory.
    pub(crate) folder: Option<PathBuf>,
}

/// How an [`IndexJob`] ended, to hand back to [`crate::Library::finish_index`].
pub struct IndexOutcome {
    pub dictionary_id: String,
    pub result: IndexResult,
}

/// How an [`IndexJob`] ended.
pub enum IndexResult {
    /// The index is built.
    Built(BuiltIndex),
    /// Indexing was cancelled. The Dictionary still needs its index.
    Cancelled,
    /// The index could not be built, for this reason (meant for the reader).
    Failed(String),
}

/// A built index, not yet in place.
pub struct BuiltIndex {
    pub(crate) stamp: Stamp,
    pub(crate) place: Built,
}

pub(crate) enum Built {
    /// Written beside its final name, waiting to be renamed into place.
    File {
        partial: PathBuf,
        path: PathBuf,
    },
    Memory(Vec<u8>),
}

impl IndexJob {
    /// The key of the Dictionary being indexed.
    pub fn dictionary_id(&self) -> &str {
        &self.dictionary_id
    }

    /// The Dictionary's title, as the Library shows it.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// How many headwords there are to read: the end of the progress count.
    pub fn entry_count(&self) -> usize {
        self.reader.entry_count()
    }

    /// Builds the index. `progress` is told how many headwords have been read so far
    /// (ending with [`IndexJob::entry_count`]); returning [`ControlFlow::Break`] from it
    /// cancels the job, leaving nothing behind.
    pub fn run(self, progress: &mut dyn FnMut(usize) -> ControlFlow<()>) -> IndexOutcome {
        let result = match self.reader.headwords(progress) {
            Ok(None) => IndexResult::Cancelled,
            Err(error) => IndexResult::Failed(error.to_string()),
            Ok(Some(headwords)) => {
                let bytes = StoredIndex::encode(headwords, &self.stamp);
                match self.place(bytes) {
                    Ok(place) => IndexResult::Built(BuiltIndex {
                        stamp: self.stamp,
                        place,
                    }),
                    Err(error) => IndexResult::Failed(error.to_string()),
                }
            }
        };
        IndexOutcome {
            dictionary_id: self.dictionary_id,
            result,
        }
    }

    fn place(&self, bytes: Vec<u8>) -> std::io::Result<Built> {
        let Some(folder) = &self.folder else {
            return Ok(Built::Memory(bytes));
        };
        std::fs::create_dir_all(folder)?;
        let path = index_file(folder, &self.dictionary_id);
        let partial = path.with_extension("odx.partial");
        std::fs::write(&partial, bytes)?;
        Ok(Built::File { partial, path })
    }
}

/// Where the index of the Dictionary with key `id` is stored.
pub(crate) fn index_file(folder: &std::path::Path, id: &str) -> PathBuf {
    folder.join(format!("{id}.odx"))
}
