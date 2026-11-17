use std::path::PathBuf;

/// Everything that can go wrong in the core.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The configuration file could not be read.
    #[error("cannot read the configuration file {file}: {reason}")]
    Config { file: PathBuf, reason: String },

    /// The Library Folder could not be read.
    #[error("cannot read the Library Folder {folder}: {source}")]
    LibraryFolder {
        folder: PathBuf,
        source: std::io::Error,
    },

    /// A Dictionary failed while answering a Lookup.
    #[error("{dictionary}: {reason}")]
    Dictionary { dictionary: String, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;
