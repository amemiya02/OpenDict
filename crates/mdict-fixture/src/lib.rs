//! Writes small MDict files for OpenDict's tests.
//!
//! The tests own this writer: no real dictionary content is ever committed, so every
//! fixture is generated from entries written in the test itself. It follows the byte
//! layout of MDX format 2.0 (UTF-8 keys and records, zlib-compressed blocks).
//!
//! ```
//! let bytes = opendict_mdict_fixture::MdxBuilder::new()
//!     .title("Sample")
//!     .entry("apple", "<p>a fruit</p>")
//!     .build();
//! assert!(!bytes.is_empty());
//! ```

use std::io::{self, Write};
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;

/// Builds an MDX file (format 2.0, UTF-8, zlib) from headwords and their records.
#[derive(Debug, Clone)]
pub struct MdxBuilder {
    title: String,
    description: String,
    entries: Vec<(String, String)>,
    keys_per_block: usize,
    records_per_block: usize,
}

impl Default for MdxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MdxBuilder {
    pub fn new() -> Self {
        Self {
            title: "Fixture".to_owned(),
            description: "A fixture written by OpenDict's tests".to_owned(),
            entries: Vec::new(),
            keys_per_block: 64,
            records_per_block: 64,
        }
    }

    /// The `Title` attribute of the header.
    pub fn title(mut self, title: &str) -> Self {
        title.clone_into(&mut self.title);
        self
    }

    /// The `Description` attribute of the header.
    pub fn description(mut self, description: &str) -> Self {
        description.clone_into(&mut self.description);
        self
    }

    /// Adds a headword with its record. Entries are written in the order they are added;
    /// the same headword may be added more than once.
    pub fn entry(mut self, headword: &str, record: &str) -> Self {
        self.entries.push((headword.to_owned(), record.to_owned()));
        self
    }

    /// Adds a headword whose record redirects to another headword (`@@@LINK=`).
    pub fn link(self, headword: &str, target: &str) -> Self {
        let record = format!("@@@LINK={target}");
        self.entry(headword, &record)
    }

    /// How many headwords go into each key block (default 64), to exercise multi-block files.
    pub fn keys_per_block(mut self, n: usize) -> Self {
        self.keys_per_block = n.max(1);
        self
    }

    /// How many records go into each record block (default 64).
    pub fn records_per_block(mut self, n: usize) -> Self {
        self.records_per_block = n.max(1);
        self
    }

    /// Writes the file and returns its bytes.
    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(self.header_section());

        // Records, each `utf8(record) ++ "\r\n" ++ NUL`, at running offsets.
        let records: Vec<Vec<u8>> = self
            .entries
            .iter()
            .map(|(_, record)| {
                let mut bytes = record.as_bytes().to_vec();
                bytes.extend_from_slice(b"\r\n\0");
                bytes
            })
            .collect();
        let mut offsets = Vec::with_capacity(records.len());
        let mut running = 0u64;
        for record in &records {
            offsets.push(running);
            running += record.len() as u64;
        }

        out.extend(self.key_section(&offsets));
        out.extend(self.record_section(&records));
        out
    }

    /// Writes the file to `path`.
    pub fn write_to(&self, path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::write(path, self.build())
    }

    fn header_section(&self) -> Vec<u8> {
        let header = format!(
            "<Dictionary GeneratedByEngineVersion=\"2.0\" RequiredEngineVersion=\"2.0\" Encrypted=\"0\" \
             Encoding=\"UTF-8\" Format=\"Html\" Title=\"{}\" Description=\"{}\" KeyCaseSensitive=\"No\" \
             StripKey=\"Yes\" StyleSheet=\"\" CreationDate=\"2026-1-1\" Compact=\"No\" Compat=\"No\" \
             Left2Right=\"Yes\" DataSourceFormat=\"106\"/>\r\n\0",
            escape_xml(&self.title),
            escape_xml(&self.description),
        );
        let header: Vec<u8> = header.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut out = Vec::new();
        out.extend((header.len() as u32).to_be_bytes());
        out.extend(&header);
        out.extend(adler32(&header).to_le_bytes());
        out
    }

    fn key_section(&self, offsets: &[u64]) -> Vec<u8> {
        let mut info = Vec::new();
        let mut blocks = Vec::new();
        let indices: Vec<usize> = (0..self.entries.len()).collect();
        for chunk in indices.chunks(self.keys_per_block) {
            let mut plain = Vec::new();
            for &i in chunk {
                plain.extend(offsets[i].to_be_bytes());
                plain.extend(self.entries[i].0.as_bytes());
                plain.push(0);
            }
            let block = compressed_block(&plain);

            let first = self.entries[chunk[0]].0.as_bytes();
            let last = self.entries[chunk[chunk.len() - 1]].0.as_bytes();
            info.extend((chunk.len() as u64).to_be_bytes());
            info.extend((first.len() as u16).to_be_bytes());
            info.extend(first);
            info.push(0);
            info.extend((last.len() as u16).to_be_bytes());
            info.extend(last);
            info.push(0);
            info.extend((block.len() as u64).to_be_bytes());
            info.extend((plain.len() as u64).to_be_bytes());
            blocks.push(block);
        }
        let info_block = compressed_block(&info);
        let blocks_len: usize = blocks.iter().map(Vec::len).sum();

        let mut head = Vec::new();
        head.extend((blocks.len() as u64).to_be_bytes());
        head.extend((self.entries.len() as u64).to_be_bytes());
        head.extend((info.len() as u64).to_be_bytes());
        head.extend((info_block.len() as u64).to_be_bytes());
        head.extend((blocks_len as u64).to_be_bytes());

        let mut out = head.clone();
        out.extend(adler32(&head).to_be_bytes());
        out.extend(info_block);
        for block in blocks {
            out.extend(block);
        }
        out
    }

    fn record_section(&self, records: &[Vec<u8>]) -> Vec<u8> {
        let mut sizes = Vec::new();
        let mut blocks = Vec::new();
        for chunk in records.chunks(self.records_per_block) {
            let plain = chunk.concat();
            let block = compressed_block(&plain);
            sizes.push((block.len() as u64, plain.len() as u64));
            blocks.push(block);
        }
        let blocks_len: u64 = sizes.iter().map(|(compressed, _)| compressed).sum();

        let mut out = Vec::new();
        out.extend((blocks.len() as u64).to_be_bytes());
        out.extend((records.len() as u64).to_be_bytes());
        out.extend((16 * blocks.len() as u64).to_be_bytes());
        out.extend(blocks_len.to_be_bytes());
        for (compressed, plain) in sizes {
            out.extend(compressed.to_be_bytes());
            out.extend(plain.to_be_bytes());
        }
        for block in blocks {
            out.extend(block);
        }
        out
    }
}

/// `[02 00 00 00] [adler32(data) BE] [zlib(data)]`
fn compressed_block(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .expect("writing to a Vec cannot fail");
    let zlib = encoder.finish().expect("writing to a Vec cannot fail");
    let mut out = vec![2, 0, 0, 0];
    out.extend(adler32(data).to_be_bytes());
    out.extend(zlib);
    out
}

/// Adler-32 as defined in RFC 1950.
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::adler32;

    #[test]
    fn adler32_matches_the_rfc_1950_example() {
        // "Wikipedia" is the common published example: 0x11E60398.
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }
}
