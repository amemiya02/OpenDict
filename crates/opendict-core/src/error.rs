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

    /// The registration code or e-mail entered does not unlock the Dictionary at this path.
    #[error("the registration code or e-mail does not unlock {0}")]
    WrongRegistration(PathBuf),

    /// Only an e-mail was entered, and the Dictionary at this path does not carry its
    /// registration code.
    #[error("{0} needs its registration code as well as the e-mail")]
    RegistrationCodeNeeded(PathBuf),

    /// A Dictionary failed while answering a Lookup.
    #[error("{dictionary}: {reason}")]
    Dictionary { dictionary: String, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;
