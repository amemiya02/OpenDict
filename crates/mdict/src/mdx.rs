use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::block;
use crate::bytes::{Reader, adler32};
use crate::error::{Error, Result};
use crate::header::Header;

/// An open MDX file.
pub struct Mdx {
    map: Mmap,
    header: Header,
    /// Every key in file order with the offset of its record in the record stream.
    keys: Vec<Key>,
    /// Indices into `keys`, sorted by key text, for exact lookup.
    sorted: Vec<usize>,
    /// Every distinct record offset in ascending order: a record ends where the next begins.
    record_starts: Vec<u64>,
    records: RecordSection,
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

struct Key {
    text: String,
    record_offset: u64,
}

impl Mdx {
    /// Opens an MDX file. The file is memory-mapped; the header and the key index are
    /// read now, records only when they are looked up.
    pub fn open(path: impl AsRef<Path>) -> Result<Mdx> {
        let file = File::open(path)?;
        // SAFETY: the map is read-only and owned by this value. OpenDict never writes
        // dictionary files; another process truncating one while it is open is outside
        // what any reader can defend against.
        let map = unsafe { Mmap::map(&file)? };
        let mut reader = Reader::new(&map);
        let header = Header::read(&mut reader)?;
        check_supported(&header)?;
        let keys = read_key_section(&mut reader)?;
        let records = read_record_section(&mut reader, keys.len())?;

        let mut sorted: Vec<usize> = (0..keys.len()).collect();
        sorted.sort_by(|&a, &b| keys[a].text.cmp(&keys[b].text).then(a.cmp(&b)));
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

        Ok(Mdx {
            map,
            header,
            keys,
            sorted,
            record_starts,
            records,
        })
    }

    /// The file's header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Every headword in the order the file stores them. A headword filed more than once
    /// appears more than once.
    pub fn headwords(&self) -> impl Iterator<Item = &str> {
        self.keys.iter().map(|key| key.text.as_str())
    }

    /// The records filed under exactly this headword (no case folding or other
    /// normalisation), in file order. Empty when the file does not have the headword.
    pub fn lookup(&self, headword: &str) -> Result<Vec<Record>> {
        let first = self
            .sorted
            .partition_point(|&i| self.keys[i].text.as_str() < headword);
        self.sorted[first..]
            .iter()
            .take_while(|&&i| self.keys[i].text == headword)
            .map(|&i| self.record(self.keys[i].record_offset))
            .collect()
    }

    fn record(&self, start: u64) -> Result<Record> {
        let next = self.record_starts.partition_point(|&s| s <= start);
        let end = self
            .record_starts
            .get(next)
            .copied()
            .unwrap_or(self.records.total);
        let bytes = self.record_bytes(start, end)?;
        Ok(decode_record(&bytes))
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

/// Decodes a record: UTF-8 text ending in a NUL (and usually CR LF before it).
fn decode_record(bytes: &[u8]) -> Record {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_end_matches(['\0', '\r', '\n']);
    match text.strip_prefix("@@@LINK=") {
        Some(target) => Record::Link(target.lines().next().unwrap_or("").trim().to_owned()),
        None => Record::Text(text.to_owned()),
    }
}

fn check_supported(header: &Header) -> Result<()> {
    let version = header.engine_version().trim();
    match version.parse::<f32>() {
        Ok(v) if (2.0..3.0).contains(&v) => {}
        Ok(v) if v < 2.0 => return Err(Error::Unsupported(format!("format version {version}"))),
        _ => return Err(Error::UnsupportedVersion(version.to_owned())),
    }
    if !matches!(
        header.encoding().trim().to_ascii_uppercase().as_str(),
        "" | "UTF-8" | "UTF8"
    ) {
        return Err(Error::Unsupported(format!(
            "encoding {}",
            header.encoding()
        )));
    }
    if header.encrypted() != 0 {
        return Err(Error::Unsupported("encryption".to_owned()));
    }
    Ok(())
}

/// Reads the key section of a version 2.0 file: a 40-byte header with its checksum, the
/// key block info table, then the key blocks.
fn read_key_section(reader: &mut Reader<'_>) -> Result<Vec<Key>> {
    let head = reader.take(40)?;
    let checksum = reader.u32_be()?;
    if adler32(head) != checksum {
        return Err(Error::Corrupt(
            "the key section header checksum does not match",
        ));
    }
    let mut head = Reader::new(head);
    let block_count = head.size()?;
    let entry_count = head.size()?;
    let info_len = head.size()?;
    let info_stored_len = head.size()?;
    let blocks_len = head.size()?;

    let info = block::decompress(reader.take(info_stored_len)?, info_len)?;
    let mut info = Reader::new(&info);
    let mut keys = Vec::with_capacity(entry_count.min(1 << 20));
    let mut blocks_seen = 0usize;
    for _ in 0..block_count {
        let entries = info.size()?;
        let first_len = usize::from(info.u16_be()?);
        info.take(first_len + 1)?;
        let last_len = usize::from(info.u16_be()?);
        info.take(last_len + 1)?;
        let stored_len = info.size()?;
        let plain_len = info.size()?;

        let plain = block::decompress(reader.take(stored_len)?, plain_len)?;
        blocks_seen += stored_len;
        let mut plain = Reader::new(&plain);
        for _ in 0..entries {
            let record_offset = plain.u64_be()?;
            let text = String::from_utf8_lossy(plain.until_nul()?).into_owned();
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

/// Reads the record section of a version 2.0 file: counts, the size table, then the blocks.
fn read_record_section(reader: &mut Reader<'_>, key_count: usize) -> Result<RecordSection> {
    let block_count = reader.size()?;
    let entry_count = reader.size()?;
    let table_len = reader.size()?;
    let blocks_len = reader.size()?;
    if entry_count != key_count || Some(table_len) != block_count.checked_mul(16) {
        return Err(Error::Corrupt("the record section header does not add up"));
    }

    let mut sizes = Vec::with_capacity(block_count.min(1 << 20));
    for _ in 0..block_count {
        sizes.push((reader.size()?, reader.size()?));
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
        stream_start += plain_len as u64;
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
