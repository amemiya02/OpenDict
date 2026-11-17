//! MDD resource files: images, sounds, fonts and stylesheets that go with an MDX.
//!
//! The layout is the MDX layout with two differences: keys are always UTF-16LE (lengths in
//! 16-bit units, two-byte terminators) and records are raw bytes with no terminator.

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::block;
use crate::bytes::{Reader, adler32};
use crate::error::{Error, Result};
use crate::header::Header;
use crate::mdx::{RecordSection, read_record_section};

/// An open MDD file.
pub struct Mdd {
    map: Mmap,
    header: Header,
    /// `(normalised path, record offset)`, sorted by path.
    keys: Vec<(String, u64)>,
    /// Every distinct record offset in ascending order: a record ends where the next begins.
    record_starts: Vec<u64>,
    records: RecordSection,
}

impl Mdd {
    /// Opens an MDD file. The file is memory-mapped; the key index is read now, resources
    /// only when they are asked for.
    pub fn open(path: impl AsRef<Path>) -> Result<Mdd> {
        let file = File::open(path)?;
        // SAFETY: as for `Mdx::open`: a read-only map owned by this value.
        let map = unsafe { Mmap::map(&file)? };
        let mut reader = Reader::new(&map);
        let header = Header::read(&mut reader)?;
        check_supported(&header)?;
        let mut keys = read_key_section(&mut reader)?;
        let records = read_record_section(&mut reader, keys.len())?;

        let mut record_starts: Vec<u64> = keys.iter().map(|&(_, offset)| offset).collect();
        record_starts.sort_unstable();
        record_starts.dedup();
        if record_starts
            .last()
            .is_some_and(|&last| last > records.total)
        {
            return Err(Error::Corrupt(
                "a record offset lies past the record section",
            ));
        }
        keys.sort();
        Ok(Mdd {
            map,
            header,
            keys,
            record_starts,
            records,
        })
    }

    /// The file's header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The bytes of the resource at `path`, or `None` if the file has no such resource.
    ///
    /// MDict stores paths Windows-style (`\img\a.png`). `path` may also be written the way
    /// HTML refers to it (`img/a.png`, `/img/a.png`); letter case is ignored. Percent-escapes
    /// are the caller's to decode.
    pub fn resource(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let wanted = normalise(path);
        let at = self
            .keys
            .partition_point(|(key, _)| key.as_str() < wanted.as_str());
        match self.keys.get(at) {
            Some((key, offset)) if *key == wanted => self.record(*offset).map(Some),
            _ => Ok(None),
        }
    }

    fn record(&self, start: u64) -> Result<Vec<u8>> {
        let next = self.record_starts.partition_point(|&s| s <= start);
        let end = self
            .record_starts
            .get(next)
            .copied()
            .unwrap_or(self.records.total);
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

/// `\`-separated, starting with `\`, lower case.
fn normalise(path: &str) -> String {
    let path = path.replace('/', "\\").to_lowercase();
    if path.starts_with('\\') {
        path
    } else {
        format!("\\{path}")
    }
}

fn check_supported(header: &Header) -> Result<()> {
    let version = header.engine_version().trim();
    match version.parse::<f32>() {
        Ok(v) if (2.0..3.0).contains(&v) => {}
        Ok(v) if v < 2.0 => return Err(Error::Unsupported(format!("format version {version}"))),
        _ => return Err(Error::UnsupportedVersion(version.to_owned())),
    }
    if header.encrypted() != 0 {
        return Err(Error::Unsupported("encryption".to_owned()));
    }
    Ok(())
}

/// Reads the key section of a version 2.0 MDD: like an MDX, but with UTF-16LE keys.
fn read_key_section(reader: &mut Reader<'_>) -> Result<Vec<(String, u64)>> {
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
        for _ in 0..2 {
            // First and last key of the block: hints only, in 16-bit units plus a terminator.
            let units = usize::from(info.u16_be()?);
            info.take((units + 1) * 2)?;
        }
        let stored_len = info.size()?;
        let plain_len = info.size()?;

        let plain = block::decompress(reader.take(stored_len)?, plain_len)?;
        blocks_seen += stored_len;
        let mut plain = Reader::new(&plain);
        for _ in 0..entries {
            let offset = plain.u64_be()?;
            let mut units = Vec::new();
            loop {
                let unit = u16::from_le_bytes(plain.array()?);
                if unit == 0 {
                    break;
                }
                units.push(unit);
            }
            keys.push((normalise(&String::from_utf16_lossy(&units)), offset));
        }
    }
    if keys.len() != entry_count || blocks_seen != blocks_len {
        return Err(Error::Corrupt("the key index does not add up"));
    }
    Ok(keys)
}
