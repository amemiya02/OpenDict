use std::path::Path;

use crate::error::Result;
use crate::file::{Kind, MdictFile};
use crate::header::Header;
use crate::registration::Registration;

/// An open MDD file: the resources (images, styles, sounds, fonts) a Dictionary's
/// Entries refer to, filed under Windows-style paths such as `\img\a.png`.
pub struct Mdd {
    file: MdictFile,
    /// `(normalised path, key index)`, sorted, for lookup by path.
    sorted: Vec<(String, usize)>,
}

impl Mdd {
    /// Opens an MDD file. The file is memory-mapped; the header and the resource paths
    /// are read now, resources only when they are asked for.
    pub fn open(path: impl AsRef<Path>) -> Result<Mdd> {
        Self::from_file(MdictFile::open(path.as_ref(), Kind::Mdd, None)?)
    }

    /// Opens an MDD file that may need a registration code (`Encrypted="1"`), unlocking
    /// it with `registration`. A file that needs none opens as with [`Mdd::open`].
    pub fn open_registered(path: impl AsRef<Path>, registration: &Registration) -> Result<Mdd> {
        Self::from_file(MdictFile::open(
            path.as_ref(),
            Kind::Mdd,
            Some(registration),
        )?)
    }

    fn from_file(file: MdictFile) -> Result<Mdd> {
        let mut sorted: Vec<(String, usize)> = file
            .keys
            .iter()
            .enumerate()
            .map(|(i, key)| (normalise(&key.text), i))
            .collect();
        sorted.sort();
        Ok(Mdd { file, sorted })
    }

    /// The file's header.
    pub fn header(&self) -> &Header {
        &self.file.header
    }

    /// Every resource path as the file stores it, in file order.
    pub fn resource_paths(&self) -> impl Iterator<Item = &str> {
        self.file.keys.iter().map(|key| key.text.as_str())
    }

    /// The bytes of the resource at `path`, or `None` when the file does not have it.
    ///
    /// `path` may use `/` or `\` as separator, with or without a leading one, in any
    /// letter case: `img/a.png` finds `\img\a.png`. It is not percent-decoded; decode
    /// URLs taken from HTML first.
    pub fn resource(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let wanted = normalise(path);
        let first = self
            .sorted
            .partition_point(|(key, _)| key.as_str() < wanted.as_str());
        match self.sorted.get(first) {
            Some((key, i)) if *key == wanted => {
                let start = self.file.keys[*i].record_offset;
                self.file.record(start).map(Some)
            }
            _ => Ok(None),
        }
    }
}

/// `img/A.png` and `\img\a.png` name the same resource.
fn normalise(path: &str) -> String {
    let path = path.replace('/', "\\").to_lowercase();
    if path.starts_with('\\') {
        path
    } else {
        format!("\\{path}")
    }
}
