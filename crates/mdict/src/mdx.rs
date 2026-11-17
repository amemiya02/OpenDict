use std::path::Path;

use crate::error::Result;
use crate::file::{Kind, MdictFile};
use crate::header::Header;
use crate::registration::Registration;

/// An open MDX file: headwords and the records filed under them.
pub struct Mdx {
    file: MdictFile,
    /// Indices into the file's keys, sorted by key text, for exact lookup.
    sorted: Vec<usize>,
}

/// What a headword's record says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// The Entry's text (usually HTML), decoded to Unicode.
    Text(String),
    /// A redirect (`@@@LINK=`) to another headword. The parser reports it; following it
    /// is the caller's decision.
    Link(String),
}

impl Mdx {
    /// Opens an MDX file. The file is memory-mapped; the header and the key index are
    /// read now, records only when they are looked up.
    pub fn open(path: impl AsRef<Path>) -> Result<Mdx> {
        Self::from_file(MdictFile::open(path.as_ref(), Kind::Mdx, None)?)
    }

    /// Opens an MDX file that may need a registration code (`Encrypted="1"`), unlocking
    /// it with `registration`. A file that needs none opens as with [`Mdx::open`].
    pub fn open_registered(path: impl AsRef<Path>, registration: &Registration) -> Result<Mdx> {
        Self::from_file(MdictFile::open(
            path.as_ref(),
            Kind::Mdx,
            Some(registration),
        )?)
    }

    fn from_file(file: MdictFile) -> Result<Mdx> {
        let keys = &file.keys;
        let mut sorted: Vec<usize> = (0..keys.len()).collect();
        sorted.sort_by(|&a, &b| keys[a].text.cmp(&keys[b].text).then(a.cmp(&b)));
        Ok(Mdx { file, sorted })
    }

    /// The file's header.
    pub fn header(&self) -> &Header {
        &self.file.header
    }

    /// Every headword in the order the file stores them. A headword filed more than once
    /// appears more than once.
    pub fn headwords(&self) -> impl Iterator<Item = &str> {
        self.file.keys.iter().map(|key| key.text.as_str())
    }

    /// The records filed under exactly this headword (no case folding or other
    /// normalisation), in file order. Empty when the file does not have the headword.
    pub fn lookup(&self, headword: &str) -> Result<Vec<Record>> {
        let keys = &self.file.keys;
        let first = self
            .sorted
            .partition_point(|&i| keys[i].text.as_str() < headword);
        self.sorted[first..]
            .iter()
            .take_while(|&&i| keys[i].text == headword)
            .map(|&i| {
                let bytes = self.file.record(keys[i].record_offset)?;
                Ok(decode_record(&self.file.encoding.decode(&bytes)))
            })
            .collect()
    }
}

/// Reads a decoded record: text ending in a NUL (and usually CR LF before it), or a redirect.
pub(crate) fn decode_record(text: &str) -> Record {
    let text = text.trim_end_matches(['\0', '\r', '\n']);
    match text.strip_prefix("@@@LINK=") {
        Some(target) => Record::Link(target.lines().next().unwrap_or("").trim().to_owned()),
        None => Record::Text(text.to_owned()),
    }
}
