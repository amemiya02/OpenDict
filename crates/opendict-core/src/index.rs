//! The headword index: every Dictionary's headwords in index order, each with where its
//! record lies, stored once per Dictionary in the data directory.
//!
//! Index order is by [`index_key`], then by the headword itself, then by the order the
//! file stores them. Lookup finds a headword's records through the index, and the
//! headword column reads the index from a point onward through [`HeadwordIndex`].

use std::path::Path;

use memmap2::Mmap;

use crate::format::Location;
use crate::normalization::normalize;
use crate::scan::Stamp;

/// The key a headword is filed under in index order: its [`normalize`]d form. The same
/// function orders the index and places a query in it. Changing it changes every stored
/// index: bump [`INDEX_VERSION`] with it so old indexes are rebuilt.
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

/// The version of the stored index layout and of [`index_key`]. An index stored with
/// another version is rebuilt.
pub(crate) const INDEX_VERSION: u32 = 2;

const MAGIC: &[u8; 4] = b"ODXI";

/// Stored layout, all numbers little-endian:
///
/// ```text
/// header   magic "ODXI", version u32, file size u64, modified seconds i64,
///          modified nanoseconds u32, reserved u32, signature u64,
///          index length u64, row count u64
/// table    one u64 per row: where the row starts in the index
/// rows     start u64, end u64, key length u32, headword length u32 (u32::MAX: the
///          headword is the key), key bytes, headword bytes
/// ```
const HEADER_LEN: usize = 56;
const SAME_AS_KEY: u32 = u32::MAX;

/// A Dictionary's index as stored, memory-mapped (or held in memory for a Library that
/// is never saved).
pub struct StoredIndex {
    bytes: Bytes,
    rows: usize,
}

enum Bytes {
    Mapped(Mmap),
    Owned(Vec<u8>),
}

impl std::ops::Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Mapped(map) => map,
            Bytes::Owned(bytes) => bytes,
        }
    }
}

/// One row of the index.
struct Row<'a> {
    key: &'a str,
    headword: &'a str,
    location: Location,
}

impl StoredIndex {
    /// Lays out the index of `headwords` (in file order) for a Dictionary whose file
    /// carries `stamp`.
    pub(crate) fn encode(headwords: Vec<(String, Location)>, stamp: &Stamp) -> Vec<u8> {
        let mut rows: Vec<(String, String, Location)> = headwords
            .into_iter()
            .map(|(headword, location)| (index_key(&headword), headword, location))
            // A headword of 4 GiB or more cannot be stored; no real Dictionary has one.
            .filter(|(key, headword, _)| fits(key) && fits(headword))
            .collect();
        // Stable: rows with the same key and headword keep the file's order.
        rows.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));

        let mut table = Vec::with_capacity(rows.len() * 8);
        let mut body = Vec::new();
        let body_start = HEADER_LEN + rows.len() * 8;
        for (key, headword, location) in &rows {
            table.extend_from_slice(&((body_start + body.len()) as u64).to_le_bytes());
            body.extend_from_slice(&location.start.to_le_bytes());
            body.extend_from_slice(&location.end.to_le_bytes());
            body.extend_from_slice(&(key.len() as u32).to_le_bytes());
            if key == headword {
                body.extend_from_slice(&SAME_AS_KEY.to_le_bytes());
                body.extend_from_slice(key.as_bytes());
            } else {
                body.extend_from_slice(&(headword.len() as u32).to_le_bytes());
                body.extend_from_slice(key.as_bytes());
                body.extend_from_slice(headword.as_bytes());
            }
        }

        let total = body_start + body.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        out.extend_from_slice(&stamp.size.to_le_bytes());
        out.extend_from_slice(&stamp.modified_seconds.to_le_bytes());
        out.extend_from_slice(&stamp.modified_nanos.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&stamp.signature.to_le_bytes());
        out.extend_from_slice(&(total as u64).to_le_bytes());
        out.extend_from_slice(&(rows.len() as u64).to_le_bytes());
        out.extend_from_slice(&table);
        out.extend_from_slice(&body);
        out
    }

    /// The index stored at `path`, if there is one, it is whole, and it was built from a
    /// file carrying `stamp`. Anything else (missing, older layout, another edition of
    /// the file, damaged) is `None`: the index must be built again.
    pub(crate) fn open(path: &Path, stamp: &Stamp) -> Option<StoredIndex> {
        let file = std::fs::File::open(path).ok()?;
        // SAFETY: the map is read-only. Index files are written only by OpenDict, beside
        // the final name and then renamed into place, so a mapped file is never rewritten.
        let map = unsafe { Mmap::map(&file) }.ok()?;
        Self::check(Bytes::Mapped(map), stamp)
    }

    /// An index held in memory, as [`StoredIndex::encode`] laid it out.
    pub(crate) fn from_bytes(bytes: Vec<u8>, stamp: &Stamp) -> Option<StoredIndex> {
        Self::check(Bytes::Owned(bytes), stamp)
    }

    fn check(bytes: Bytes, stamp: &Stamp) -> Option<StoredIndex> {
        let header = bytes.get(..HEADER_LEN)?;
        let u32_at = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        let u64_at = |at: usize| u64::from_le_bytes(header[at..at + 8].try_into().unwrap());
        let whole = &header[..4] == MAGIC
            && u32_at(4) == INDEX_VERSION
            && u64_at(8) == stamp.size
            && u64_at(16) == stamp.modified_seconds as u64
            && u32_at(24) == stamp.modified_nanos
            && u64_at(32) == stamp.signature
            && u64_at(40) == bytes.len() as u64;
        let rows = usize::try_from(u64_at(48)).ok()?;
        let table_end = rows.checked_mul(8)?.checked_add(HEADER_LEN)?;
        (whole && table_end <= bytes.len()).then_some(StoredIndex { bytes, rows })
    }

    fn row(&self, n: usize) -> Result<Row<'_>, IndexError> {
        let damaged = || IndexError("the stored index is damaged; rescan to rebuild it".into());
        let bytes = &self.bytes[..];
        let read = |at: usize, len: usize| bytes.get(at..at.checked_add(len)?);
        let u32_at = |at| read(at, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let u64_at = |at| read(at, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));

        let at = usize::try_from(u64_at(HEADER_LEN + n * 8).ok_or_else(damaged)?)
            .map_err(|_| damaged())?;
        let start = u64_at(at).ok_or_else(damaged)?;
        let end = u64_at(at + 8).ok_or_else(damaged)?;
        let key_len = u32_at(at + 16).ok_or_else(damaged)? as usize;
        let headword_len = u32_at(at + 20).ok_or_else(damaged)?;
        let text = |at: usize, len: usize| {
            read(at, len)
                .and_then(|b| std::str::from_utf8(b).ok())
                .ok_or_else(damaged)
        };
        let key = text(at + 24, key_len)?;
        let headword = if headword_len == SAME_AS_KEY {
            key
        } else {
            text(at + 24 + key_len, headword_len as usize)?
        };
        Ok(Row {
            key,
            headword,
            location: Location { start, end },
        })
    }

    /// The first row for which `before` is false; rows for which it is true come first.
    fn partition_point(&self, before: impl Fn(&Row<'_>) -> bool) -> Result<usize, IndexError> {
        let (mut low, mut high) = (0, self.rows);
        while low < high {
            let middle = low + (high - low) / 2;
            if before(&self.row(middle)?) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }

    /// Where the records filed under exactly `headword` lie, in the file's order.
    pub(crate) fn locations(&self, headword: &str) -> Result<Vec<Location>, IndexError> {
        let key = index_key(headword);
        let first = self.partition_point(|row| (row.key, row.headword) < (&key, headword))?;
        let mut locations = Vec::new();
        for n in first..self.rows {
            let row = self.row(n)?;
            if row.key != key || row.headword != headword {
                break;
            }
            locations.push(row.location);
        }
        Ok(locations)
    }
}

impl HeadwordIndex for StoredIndex {
    fn headwords(
        &self,
        start: Start<'_>,
        limit: usize,
    ) -> Result<Vec<IndexedHeadword>, IndexError> {
        let first = match start {
            Start::AtKey(key) => self.partition_point(|row| row.key < key)?,
            Start::After(after) => self.partition_point(|row| {
                (row.key, row.headword) <= (after.key.as_str(), after.headword.as_str())
            })?,
        };
        let mut headwords: Vec<IndexedHeadword> = Vec::new();
        for n in first..self.rows {
            if headwords.len() == limit {
                break;
            }
            let row = self.row(n)?;
            let repeat = headwords
                .last()
                .is_some_and(|last| last.key == row.key && last.headword == row.headword);
            if !repeat {
                headwords.push(IndexedHeadword {
                    key: row.key.to_owned(),
                    headword: row.headword.to_owned(),
                });
            }
        }
        Ok(headwords)
    }
}

fn fits(text: &str) -> bool {
    text.len() < SAME_AS_KEY as usize
}
