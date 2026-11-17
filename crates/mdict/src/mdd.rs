use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::Result;
use crate::file::{Key, Kind, Sections};
use crate::header::Header;
use crate::registration::Registration;

/// An open MDD file: the resources (images, styles, sounds, fonts) a Dictionary's
/// Entries refer to, filed under Windows-style paths such as `\img\a.png`.
///
/// Opening reads only the header and the section tables, so it costs about the same for a
/// file of any size. MDict files keep their paths sorted, so a resource is found by the
/// first and last paths the key index names for each key block, decompressing only the
/// block that can hold it. A file whose key index or blocks do not bear that out (paths
/// out of order, hints that are not the literal paths) is read whole the first time it is
/// asked for something, and searched in memory from then on.
pub struct Mdd {
    sections: Sections,
    /// The first and last paths of each key block as the key index names them, normalised.
    hints: Vec<(String, String)>,
    /// Whether a resource can still be found from `hints`; cleared for good as soon as
    /// the file turns out not to be filed in the order they promise.
    by_block: AtomicBool,
    /// Every key, read when `by_block` cannot be relied on, or for listing every path.
    whole: OnceLock<Whole>,
    /// The last key block decompressed: the resources of one Entry usually share one.
    last_block: Mutex<Option<(usize, Arc<Vec<Key>>)>>,
}

/// Every key of the file, read in one go.
struct Whole {
    /// Every key in file order.
    keys: Vec<Key>,
    /// `(normalised path, key index)`, sorted, for lookup by path.
    sorted: Vec<(String, usize)>,
    /// Every distinct record start in ascending order: a record ends where the next begins.
    record_starts: Vec<u64>,
}

impl Mdd {
    /// Opens an MDD file. The file is memory-mapped; only its header and section tables
    /// are read now, resource paths and resources when they are asked for.
    pub fn open(path: impl AsRef<Path>) -> Result<Mdd> {
        Ok(Self::from_sections(Sections::open(
            path.as_ref(),
            Kind::Mdd,
            None,
        )?))
    }

    /// Opens an MDD file that may need a registration code (`Encrypted="1"`), unlocking
    /// it with `registration`. A file that needs none opens as with [`Mdd::open`].
    pub fn open_registered(path: impl AsRef<Path>, registration: &Registration) -> Result<Mdd> {
        Ok(Self::from_sections(Sections::open(
            path.as_ref(),
            Kind::Mdd,
            Some(registration),
        )?))
    }

    fn from_sections(sections: Sections) -> Mdd {
        let hints: Vec<(String, String)> = (0..sections.key_block_count())
            .map(|index| {
                let (first, last) = sections.key_block_hints(index);
                (normalise(first), normalise(last))
            })
            .collect();
        let in_order = hints.iter().all(|(first, last)| first <= last)
            && hints.windows(2).all(|pair| pair[0].1 <= pair[1].0);
        Mdd {
            sections,
            hints,
            by_block: AtomicBool::new(in_order),
            whole: OnceLock::new(),
            last_block: Mutex::new(None),
        }
    }

    /// The file's header.
    pub fn header(&self) -> &Header {
        &self.sections.header
    }

    /// Every resource path as the file stores it, in file order. Reads every key block
    /// the first time; a file with a damaged key block lists nothing (asking for a
    /// resource with [`Mdd::resource`] reports the damage).
    pub fn resource_paths(&self) -> impl Iterator<Item = &str> {
        self.whole()
            .ok()
            .into_iter()
            .flat_map(|whole| whole.keys.iter().map(|key| key.text.as_str()))
    }

    /// The bytes of the resource at `path`, or `None` when the file does not have it.
    ///
    /// `path` may use `/` or `\` as separator, with or without a leading one, in any
    /// letter case: `img/a.png` finds `\img\a.png`. It is not percent-decoded; decode
    /// URLs taken from HTML first.
    pub fn resource(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let wanted = normalise(path);
        if self.whole.get().is_none() && self.by_block.load(Ordering::Relaxed) {
            match self.find_by_block(&wanted)? {
                Some(found) => return Ok(found),
                None => self.by_block.store(false, Ordering::Relaxed),
            }
        }
        let whole = self.whole()?;
        let first = whole
            .sorted
            .partition_point(|(key, _)| key.as_str() < wanted.as_str());
        match whole.sorted.get(first) {
            Some((key, i)) if *key == wanted => {
                let start = whole.keys[*i].record_offset;
                let next = whole.record_starts.partition_point(|&s| s <= start);
                let end = whole
                    .record_starts
                    .get(next)
                    .copied()
                    .unwrap_or(self.sections.record_stream_len());
                self.sections.record_bytes(start, end).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Looks `wanted` up in the key blocks whose hints say they could hold it. `None`
    /// when a block read on the way shows the hints cannot be relied on.
    fn find_by_block(&self, wanted: &str) -> Result<Option<Option<Vec<u8>>>> {
        let from = self
            .hints
            .partition_point(|(_, last)| last.as_str() < wanted);
        for index in from..self.hints.len() {
            if self.hints[index].0.as_str() > wanted {
                break;
            }
            let Some(keys) = self.checked_block(index)? else {
                return Ok(None);
            };
            let Some(at) = keys.iter().position(|key| normalise(&key.text) == wanted) else {
                continue;
            };
            let start = keys[at].record_offset;
            let Some(end) = self.record_end(index, &keys[at + 1..], start)? else {
                return Ok(None);
            };
            return self
                .sections
                .record_bytes(start, end)
                .map(|b| Some(Some(b)));
        }
        Ok(Some(None))
    }

    /// Where the record starting at `start` ends: at the next larger start among the keys
    /// after it (`rest`, then the following blocks), or at the end of the record stream.
    /// `None` when the records turn out not to be stored in key order.
    fn record_end(&self, index: usize, rest: &[Key], start: u64) -> Result<Option<u64>> {
        let after = |keys: &[Key]| {
            keys.iter()
                .find(|key| key.record_offset > start)
                .map(|key| key.record_offset)
        };
        if let Some(end) = after(rest) {
            return Ok(Some(end));
        }
        for next in index + 1..self.hints.len() {
            let Some(keys) = self.checked_block(next)? else {
                return Ok(None);
            };
            if let Some(end) = after(&keys) {
                return Ok(Some(end));
            }
        }
        Ok(Some(self.sections.record_stream_len()))
    }

    /// Key block `index`, if it bears out its hints: its first and last paths are the
    /// ones named, every path lies between them, and records follow in key order.
    fn checked_block(&self, index: usize) -> Result<Option<Arc<Vec<Key>>>> {
        let keys = self.key_block(index)?;
        let (first, last) = &self.hints[index];
        let paths: Vec<String> = keys.iter().map(|key| normalise(&key.text)).collect();
        let checks_out = paths.first() == Some(first)
            && paths.last() == Some(last)
            && paths.iter().all(|path| first <= path && path <= last)
            && keys
                .windows(2)
                .all(|pair| pair[0].record_offset <= pair[1].record_offset)
            && keys
                .iter()
                .all(|key| key.record_offset < self.sections.record_stream_len());
        Ok(checks_out.then_some(keys))
    }

    /// Key block `index`, decompressed; the last one is kept.
    fn key_block(&self, index: usize) -> Result<Arc<Vec<Key>>> {
        let mut last = self
            .last_block
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((cached, keys)) = &*last
            && *cached == index
        {
            return Ok(Arc::clone(keys));
        }
        let keys = Arc::new(self.sections.key_block(index)?);
        *last = Some((index, Arc::clone(&keys)));
        Ok(keys)
    }

    /// Every key, read the first time it is needed.
    fn whole(&self) -> Result<&Whole> {
        if let Some(whole) = self.whole.get() {
            return Ok(whole);
        }
        let (keys, record_starts) = self.sections.all_keys()?;
        let mut sorted: Vec<(String, usize)> = keys
            .iter()
            .enumerate()
            .map(|(i, key)| (normalise(&key.text), i))
            .collect();
        sorted.sort();
        Ok(self.whole.get_or_init(|| Whole {
            keys,
            sorted,
            record_starts,
        }))
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
