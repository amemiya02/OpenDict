//! MDD resource files: the same layout as an MDX, with UTF-16LE keys and raw records.

use std::io;
use std::path::Path;

use crate::{adler32, compressed_block};

/// Builds an MDD file (format 2.0, zlib) from resource paths and their bytes.
#[derive(Debug, Clone)]
pub struct MddBuilder {
    resources: Vec<(String, Vec<u8>)>,
    keys_per_block: usize,
}

impl Default for MddBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MddBuilder {
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            keys_per_block: 64,
        }
    }

    /// Adds a resource under `path`, written as MDict stores it (e.g. `\img\a.png`).
    pub fn resource(mut self, path: &str, bytes: &[u8]) -> Self {
        self.resources.push((path.to_owned(), bytes.to_vec()));
        self
    }

    /// How many keys go into each key block (default 64).
    pub fn keys_per_block(mut self, n: usize) -> Self {
        self.keys_per_block = n.max(1);
        self
    }

    /// Writes the file and returns its bytes.
    pub fn build(&self) -> Vec<u8> {
        let mut out = header_section();
        let mut offsets = Vec::with_capacity(self.resources.len());
        let mut running = 0u64;
        for (_, bytes) in &self.resources {
            offsets.push(running);
            running += bytes.len() as u64;
        }
        out.extend(self.key_section(&offsets));
        out.extend(self.record_section());
        out
    }

    /// Writes the file to `path`.
    pub fn write_to(&self, path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::write(path, self.build())
    }

    fn key_section(&self, offsets: &[u64]) -> Vec<u8> {
        let mut info = Vec::new();
        let mut blocks = Vec::new();
        let indices: Vec<usize> = (0..self.resources.len()).collect();
        for chunk in indices.chunks(self.keys_per_block) {
            let mut plain = Vec::new();
            for &i in chunk {
                plain.extend(offsets[i].to_be_bytes());
                plain.extend(utf16(&self.resources[i].0));
                plain.extend([0, 0]);
            }
            let block = compressed_block(&plain);
            let first = &self.resources[chunk[0]].0;
            let last = &self.resources[chunk[chunk.len() - 1]].0;
            info.extend((chunk.len() as u64).to_be_bytes());
            for word in [first, last] {
                info.extend((word.encode_utf16().count() as u16).to_be_bytes());
                info.extend(utf16(word));
                info.extend([0, 0]);
            }
            info.extend((block.len() as u64).to_be_bytes());
            info.extend((plain.len() as u64).to_be_bytes());
            blocks.push(block);
        }
        let info_block = compressed_block(&info);
        let blocks_len: usize = blocks.iter().map(Vec::len).sum();

        let mut head = Vec::new();
        head.extend((blocks.len() as u64).to_be_bytes());
        head.extend((self.resources.len() as u64).to_be_bytes());
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

    fn record_section(&self) -> Vec<u8> {
        let plain: Vec<u8> = self
            .resources
            .iter()
            .flat_map(|(_, bytes)| bytes.iter().copied())
            .collect();
        let block = compressed_block(&plain);
        let mut out = Vec::new();
        out.extend(1u64.to_be_bytes());
        out.extend((self.resources.len() as u64).to_be_bytes());
        out.extend(16u64.to_be_bytes());
        out.extend((block.len() as u64).to_be_bytes());
        out.extend((block.len() as u64).to_be_bytes());
        out.extend((plain.len() as u64).to_be_bytes());
        out.extend(block);
        out
    }
}

fn header_section() -> Vec<u8> {
    let header = "<Library_Data GeneratedByEngineVersion=\"2.0\" RequiredEngineVersion=\"2.0\" \
                  Encrypted=\"0\" Format=\"\" Title=\"\" Description=\"\" KeyCaseSensitive=\"No\" \
                  StripKey=\"No\" CreationDate=\"2026-1-1\"/>\r\n\0";
    let header = utf16(header);
    let mut out = Vec::new();
    out.extend((header.len() as u32).to_be_bytes());
    out.extend(&header);
    out.extend(adler32(&header).to_le_bytes());
    out
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
