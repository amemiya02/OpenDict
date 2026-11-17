//! The dictionary-format interface the core programs against.
//!
//! Lookup never names a file format. Each format (MDict now; StarDict or DSL later)
//! implements [`DictionaryFormat`] to recognise and open its files, and the opened file
//! answers through [`DictionaryReader`].

use std::path::{Component, Path, PathBuf};

/// A dictionary file format the Library can read.
pub trait DictionaryFormat: Send + Sync {
    /// Whether `path` is the main file of a Dictionary in this format.
    fn recognises(&self, path: &Path) -> bool;

    /// Opens the Dictionary whose main file is `path`.
    fn open(&self, path: &Path) -> Result<Box<dyn DictionaryReader>, OpenError>;
}

/// An open Dictionary, as a format presents it to the core.
pub trait DictionaryReader: Send + Sync {
    /// The Dictionary's title as its file states it; may be empty.
    fn title(&self) -> &str;

    /// How many Entries (headwords, redirects included) the Dictionary files.
    fn entry_count(&self) -> usize;

    /// The records filed under exactly `headword`, in the file's order.
    fn records(&self, headword: &str) -> Result<Vec<RawRecord>, FormatError>;

    /// The bytes of a file the Dictionary's Entries refer to (an image, a stylesheet, a
    /// sound), by the relative path an Entry uses, or `None` if the Dictionary has no such
    /// file. Must never return anything from outside the Dictionary.
    fn resource(&self, path: &str) -> Result<Option<Vec<u8>>, FormatError> {
        let _ = path;
        Ok(None)
    }
}

/// One record as a format returns it, before the core resolves redirects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawRecord {
    /// Entry content (HTML).
    Text(String),
    /// A redirect to another headword in the same Dictionary.
    Link(String),
}

/// A format could not read a Dictionary. The message is meant for the reader.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct FormatError(pub String);

/// Why a Dictionary could not be opened, in words meant for the reader.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OpenError {
    /// The file exists but could not be read.
    #[error("the file cannot be read: {0}")]
    Unreadable(String),
    /// The file is not a dictionary in the format its name suggests.
    #[error("this is not a dictionary file")]
    NotADictionary,
    /// The file uses a format version OpenDict does not read.
    #[error("format version {0} is not supported")]
    UnsupportedVersion(String),
    /// Part of the file is damaged.
    #[error("the file is damaged")]
    Damaged,
    /// The Dictionary is encrypted (`Encrypted=1`) and opens only with its registration
    /// code or e-mail.
    #[error("it needs a registration code")]
    NeedsRegistrationCode,
    /// The file uses a feature OpenDict does not read yet.
    #[error("it uses {0}, which OpenDict cannot read yet")]
    Unsupported(String),
}

/// MDict (`.mdx`), read with `opendict-mdict`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MdictFormat;

impl DictionaryFormat for MdictFormat {
    fn recognises(&self, path: &Path) -> bool {
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"))
    }

    fn open(&self, path: &Path) -> Result<Box<dyn DictionaryReader>, OpenError> {
        let mdx = opendict_mdict::Mdx::open(path).map_err(open_error)?;
        Ok(Box::new(MdictReader {
            mdx,
            resource_files: open_resource_files(path),
            folder: path.parent().map(Path::to_owned).unwrap_or_default(),
        }))
    }
}

struct MdictReader {
    mdx: opendict_mdict::Mdx,
    /// `name.mdd`, `name.1.mdd`, `name.2.mdd`, … in that order.
    resource_files: Vec<opendict_mdict::Mdd>,
    /// The folder the MDX is in; stylesheets and scripts beside it belong to the Dictionary.
    folder: PathBuf,
}

impl DictionaryReader for MdictReader {
    fn title(&self) -> &str {
        self.mdx.header().title()
    }

    fn entry_count(&self) -> usize {
        self.mdx.headwords().count()
    }

    fn records(&self, headword: &str) -> Result<Vec<RawRecord>, FormatError> {
        let records = self.mdx.lookup(headword).map_err(mdict_error)?;
        Ok(records
            .into_iter()
            .map(|record| match record {
                opendict_mdict::Record::Text(text) => RawRecord::Text(text),
                opendict_mdict::Record::Link(target) => RawRecord::Link(target),
            })
            .collect())
    }

    /// A file beside the MDX first (so a loose stylesheet can stand in for the one in the
    /// MDD), then each MDD in order.
    fn resource(&self, path: &str) -> Result<Option<Vec<u8>>, FormatError> {
        let Some(relative) = contained_path(path) else {
            return Ok(None);
        };
        if is_loose_resource(&relative) {
            match std::fs::read(self.folder.join(&relative)) {
                Ok(bytes) => return Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(FormatError(error.to_string())),
            }
        }
        let key = relative.to_string_lossy();
        for mdd in &self.resource_files {
            if let Some(bytes) = mdd.resource(&key).map_err(mdict_error)? {
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }
}

/// The MDD files that go with the MDX at `path`. One that cannot be read is skipped: the
/// Entries still open, without its resources.
fn open_resource_files(path: &Path) -> Vec<opendict_mdict::Mdd> {
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let names =
        std::iter::once(format!("{stem}.mdd")).chain((1..).map(|n| format!("{stem}.{n}.mdd")));
    names
        .map(|name| path.with_file_name(name))
        .take_while(|candidate| candidate.is_file())
        .filter_map(|candidate| opendict_mdict::Mdd::open(candidate).ok())
        .collect()
}

/// `path` as a relative path made only of plain names, or `None` if it could reach
/// outside the Dictionary's folder (`..`, a root, a drive prefix).
fn contained_path(path: &str) -> Option<PathBuf> {
    let mut relative = PathBuf::new();
    for part in path.split(['/', '\\']) {
        if part.is_empty() && relative.as_os_str().is_empty() {
            continue; // a leading separator: MDD keys start with one
        }
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(name)), None) if !part.contains(':') => relative.push(name),
            _ => return None,
        }
    }
    (!relative.as_os_str().is_empty()).then_some(relative)
}

/// File types served from beside the MDX. Never the dictionary files themselves, nor
/// documents that could stand in for an Entry.
fn is_loose_resource(path: &Path) -> bool {
    const TYPES: &[&str] = &[
        "css", "js", "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "ttf", "otf", "woff",
        "woff2", "mp3", "ogg", "oga", "spx", "wav", "m4a", "aac",
    ];
    path.extension()
        .is_some_and(|ext| TYPES.iter().any(|t| ext.eq_ignore_ascii_case(t)))
}

fn open_error(error: opendict_mdict::Error) -> OpenError {
    use opendict_mdict::Error;
    match error {
        Error::Io(error) => OpenError::Unreadable(error.to_string()),
        Error::NotMdict(_) => OpenError::NotADictionary,
        Error::UnsupportedVersion(version) => OpenError::UnsupportedVersion(version),
        Error::Corrupt(_) => OpenError::Damaged,
        Error::Encrypted => OpenError::NeedsRegistrationCode,
        Error::Unsupported(feature) => OpenError::Unsupported(feature),
        other => OpenError::Unsupported(other.to_string()),
    }
}

fn mdict_error(error: opendict_mdict::Error) -> FormatError {
    FormatError(error.to_string())
}
