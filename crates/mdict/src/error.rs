use std::io;

/// Everything that can go wrong reading an MDict file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The file could not be opened or read.
    #[error("cannot read the file: {0}")]
    Io(#[from] io::Error),

    /// The file does not start with an MDict header.
    #[error("not an MDict file: {0}")]
    NotMdict(&'static str),

    /// The file uses a format version this parser does not read.
    #[error("unsupported MDict format version {0:?}")]
    UnsupportedVersion(String),

    /// The file uses a feature (encoding, compression, encryption) this parser does not read.
    #[error("unsupported MDict feature: {0}")]
    Unsupported(String),

    /// The file is encrypted with a registration code (`Encrypted="1"`), which this
    /// parser cannot read without the code. (An encrypted key index alone, `Encrypted="2"`,
    /// needs no code and is read.)
    #[error("the dictionary is encrypted and needs a registration code")]
    Encrypted,

    /// A part of the file is damaged: a checksum, size or offset does not add up.
    #[error("damaged MDict file: {0}")]
    Corrupt(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
