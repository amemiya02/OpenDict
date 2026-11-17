//! The layout MDX and MDD files share: header, key section, record section.

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::block;
use crate::bytes::{Reader, adler32};
use crate::error::{Error, Result};
use crate::header::Header;

/// The format version, which sets the width of numbers and the shape of the key index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Version {
    /// 1.x: 32-bit numbers, a raw key index, no key section checksum.
    V1,
    /// 2.x: 64-bit numbers, a compressed and checksummed key index.
    V2,
}

impl Version {
    fn of(header: &Header) -> Result<Version> {
        let written = header.engine_version().trim();
        match written.parse::<f32>() {
            Ok(v) if (1.0..2.0).contains(&v) => Ok(Version::V1),
            Ok(v) if (2.0..3.0).contains(&v) => Ok(Version::V2),
            _ => Err(Error::UnsupportedVersion(written.to_owned())),
        }
    }

    /// Reads a size, count or offset in this version's width.
    fn number(self, reader: &mut Reader<'_>) -> Result<usize> {
        let n = match self {
            Version::V1 => u64::from(reader.u32_be()?),
            Version::V2 => reader.u64_be()?,
        };
        usize::try_from(n).map_err(|_| Error::Corrupt("a size does not fit in memory"))
    }

    fn width(self) -> usize {
        match self {
            Version::V1 => 4,
            Version::V2 => 8,
        }
    }
}

/// The text encoding of keys (and, in an MDX, records).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextEncoding {
    encoding: &'static encoding_rs::Encoding,
    /// Bytes per code unit; also the length of a NUL terminator.
    unit: usize,
}

impl TextEncoding {
    /// MDD keys are always UTF-16LE, whatever the header says.
    pub(crate) const UTF_16LE: TextEncoding = TextEncoding {
        encoding: encoding_rs::UTF_16LE,
        unit: 2,
    };

    /// The encoding an MDX header names. Empty means UTF-8.
    pub(crate) fn of(header: &Header) -> Result<TextEncoding> {
        let name = header.encoding().trim().to_ascii_uppercase();
        let (encoding, unit) = match name.as_str() {
            "" | "UTF-8" | "UTF8" => (encoding_rs::UTF_8, 1),
            "UTF-16" | "UTF16" | "UTF-16LE" => (encoding_rs::UTF_16LE, 2),
            // GB18030 is a superset of GBK, which is a superset of GB2312.
            "GBK" | "GB2312" | "GB18030" => (encoding_rs::GB18030, 1),
            "BIG5" | "BIG-5" => (encoding_rs::BIG5, 1),
            _ => {
                return Err(Error::Unsupported(format!(
                    "encoding {}",
                    header.encoding()
                )));
            }
        };
        Ok(TextEncoding { encoding, unit })
    }

    /// Decodes text, replacing malformed sequences rather than failing.
    pub(crate) fn decode(self, bytes: &[u8]) -> String {
        self.encoding
            .decode_without_bom_handling(bytes)
            .0
            .into_owned()
    }
}

/// Which kind of file is being read; it decides how keys are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Mdx,
    Mdd,
}

/// A key as stored: its text and where its record starts in the record stream.
pub(crate) struct Key {
    pub(crate) text: String,
    pub(crate) record_offset: u64,
}

/// An open MDX or MDD file. The header and every key are read at open; records are
/// decompressed only when asked for.
pub(crate) struct MdictFile {
    map: Mmap,
    pub(crate) header: Header,
    pub(crate) encoding: TextEncoding,
    /// Every key in file order.
    pub(crate) keys: Vec<Key>,
    /// Every distinct record offset in ascending order: a record ends where the next begins.
    record_starts: Vec<u64>,
    records: RecordSection,
}

/// Where the record blocks are and how large each is once decompressed.
struct RecordSection {
    blocks: Vec<RecordBlock>,
    /// Size of the whole decompressed record stream.
    total: u64,
}

struct RecordBlock {
    /// Byte range of the stored block in the file.
    file_start: usize,
    stored_len: usize,
    /// Offset of the block's first byte in the decompressed record stream.
    stream_start: u64,
    plain_len: usize,
}

impl MdictFile {
    pub(crate) fn open(path: &Path, kind: Kind) -> Result<MdictFile> {
        let file = File::open(path)?;
        // SAFETY: the map is read-only and owned by this value. OpenDict never writes
        // dictionary files; another process truncating one while it is open is outside
        // what any reader can defend against.
        let map = unsafe { Mmap::map(&file)? };
        let mut reader = Reader::new(&map);
        let header = Header::read(&mut reader)?;
        let version = Version::of(&header)?;
        let encrypted = header.encrypted();
        if encrypted & 1 != 0 {
            return Err(Error::Encrypted);
        }
        let encrypted_index = encrypted & 2 != 0;
        if encrypted_index && version == Version::V1 {
            return Err(Error::Unsupported(
                "an encrypted key index in format version 1".to_owned(),
            ));
        }
        let encoding = match kind {
            Kind::Mdx => TextEncoding::of(&header)?,
            Kind::Mdd => TextEncoding::UTF_16LE,
        };

        let keys = read_key_section(&mut reader, version, encoding, encrypted_index)?;
        let records = read_record_section(&mut reader, version, keys.len())?;

        let mut record_starts: Vec<u64> = keys.iter().map(|key| key.record_offset).collect();
        record_starts.sort_unstable();
        record_starts.dedup();
        if record_starts
            .last()
            .is_some_and(|&last| last >= records.total)
        {
            return Err(Error::Corrupt(
                "a record offset lies past the record section",
            ));
        }

        Ok(MdictFile {
            map,
            header,
            encoding,
            keys,
            record_starts,
            records,
        })
    }

    /// The raw bytes of the record that starts at `start`: up to the next record's start.
    pub(crate) fn record(&self, start: u64) -> Result<Vec<u8>> {
        let next = self.record_starts.partition_point(|&s| s <= start);
        let end = self
            .record_starts
            .get(next)
            .copied()
            .unwrap_or(self.records.total);
        self.record_bytes(start, end)
    }

    /// The bytes of the record stream in `start..end`, decompressing only the blocks that hold them.
    fn record_bytes(&self, start: u64, end: u64) -> Result<Vec<u8>> {
        let blocks = &self.records.blocks;
        let mut index = blocks
            .partition_point(|b| b.stream_start <= start)
            .saturating_sub(1);
        let mut out = Vec::new();
        let mut at = start;
        while at < end {
            let block = blocks
                .get(index)
                .ok_or(Error::Corrupt("a record runs past the record section"))?;
            let stored = self
                .map
                .get(block.file_start..block.file_start + block.stored_len)
                .ok_or(Error::Corrupt(
                    "a record block lies past the end of the file",
                ))?;
            let plain = block::decompress(stored, block.plain_len)?;
            let from = (at - block.stream_start) as usize;
            let to = ((end - block.stream_start) as usize).min(plain.len());
            out.extend_from_slice(&plain[from..to]);
            at = block.stream_start + to as u64;
            index += 1;
        }
        Ok(out)
    }
}

/// Reads the key section: its header, the key index (key block info), then every key block.
fn read_key_section(
    reader: &mut Reader<'_>,
    version: Version,
    encoding: TextEncoding,
    encrypted_index: bool,
) -> Result<Vec<Key>> {
    let head_len = match version {
        Version::V1 => 16,
        Version::V2 => 40,
    };
    let head = reader.take(head_len)?;
    if version == Version::V2 && adler32(head) != reader.u32_be()? {
        return Err(Error::Corrupt(
            "the key section header checksum does not match",
        ));
    }
    let mut head = Reader::new(head);
    let block_count = version.number(&mut head)?;
    let entry_count = version.number(&mut head)?;
    let info_plain_len = match version {
        Version::V1 => None,
        Version::V2 => Some(version.number(&mut head)?),
    };
    let info_stored_len = version.number(&mut head)?;
    let blocks_len = version.number(&mut head)?;

    let stored_info = reader.take(info_stored_len)?;
    let info = match info_plain_len {
        // Version 1 stores the key index raw.
        None => stored_info.to_vec(),
        Some(plain_len) if encrypted_index => {
            block::decompress(&block::decrypt_key_index(stored_info)?, plain_len)?
        }
        Some(plain_len) => block::decompress(stored_info, plain_len)?,
    };

    let unit = encoding.unit;
    let mut info = Reader::new(&info);
    let mut keys = Vec::with_capacity(entry_count.min(1 << 20));
    let mut blocks_seen = 0usize;
    for _ in 0..block_count {
        let entries = version.number(&mut info)?;
        // The first and last keys of the block: hints only, not always the literal keys.
        for _ in 0..2 {
            let (len, terminator) = match version {
                Version::V1 => (usize::from(info.u8()?), 0),
                Version::V2 => (usize::from(info.u16_be()?), unit),
            };
            info.take(len * unit + terminator)?;
        }
        let stored_len = version.number(&mut info)?;
        let plain_len = version.number(&mut info)?;

        let plain = block::decompress(reader.take(stored_len)?, plain_len)?;
        blocks_seen = blocks_seen.saturating_add(stored_len);
        let mut plain = Reader::new(&plain);
        for _ in 0..entries {
            let record_offset = version.number(&mut plain)? as u64;
            let text = encoding.decode(plain.until_nul(unit)?);
            keys.push(Key {
                text,
                record_offset,
            });
        }
    }
    if keys.len() != entry_count || blocks_seen != blocks_len {
        return Err(Error::Corrupt("the key index does not add up"));
    }
    Ok(keys)
}

/// Reads the record section: counts, the size table, then the blocks.
fn read_record_section(
    reader: &mut Reader<'_>,
    version: Version,
    key_count: usize,
) -> Result<RecordSection> {
    let block_count = version.number(reader)?;
    let entry_count = version.number(reader)?;
    let table_len = version.number(reader)?;
    let blocks_len = version.number(reader)?;
    if entry_count != key_count || Some(table_len) != block_count.checked_mul(2 * version.width()) {
        return Err(Error::Corrupt("the record section header does not add up"));
    }

    let mut sizes = Vec::with_capacity(block_count.min(1 << 20));
    for _ in 0..block_count {
        sizes.push((version.number(reader)?, version.number(reader)?));
    }
    let mut file_start = reader.position();
    let mut stream_start = 0u64;
    let mut blocks = Vec::with_capacity(sizes.len());
    for (stored_len, plain_len) in sizes {
        blocks.push(RecordBlock {
            file_start,
            stored_len,
            stream_start,
            plain_len,
        });
        file_start = file_start
            .checked_add(stored_len)
            .ok_or(Error::Corrupt("record blocks overflow"))?;
        stream_start = stream_start
            .checked_add(plain_len as u64)
            .ok_or(Error::Corrupt("record blocks overflow"))?;
    }
    if file_start - reader.position() != blocks_len {
        return Err(Error::Corrupt("the record block sizes do not add up"));
    }
    reader.take(blocks_len)?;
    Ok(RecordSection {
        blocks,
        total: stream_start,
    })
}
