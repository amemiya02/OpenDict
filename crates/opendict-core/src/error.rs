use std::path::PathBuf;

/// Everything that can go wrong in the core.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The file the Library is recorded in could not be read or written.
    #[error("cannot read or write the Library file {file}: {reason}")]
    LibraryFile { file: PathBuf, reason: String },

    /// The Library Folder could not be read.
    #[error("cannot read the Library Folder {folder}: {source}")]
    LibraryFolder {
        folder: PathBuf,
        source: std::io::Error,
    },

    /// No Dictionary in the Library has this key.
    #[error("no Dictionary in the Library has the key {0}")]
    UnknownDictionary(String),

    /// A Dictionary failed while answering a Lookup.
    #[error("{dictionary}: {reason}")]
    Dictionary { dictionary: String, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;
