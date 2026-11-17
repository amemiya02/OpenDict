use std::ops::Range;
use std::path::Path;

use crate::error::Result;
use crate::file::{Kind, Sections};
use crate::header::Header;
use crate::mdx::{Record, decode_record};
use crate::registration::Registration;

/// An MDX file opened without reading its keys, for applications that keep their own
/// index of headwords.
///
/// Opening reads only the header and the tables of the key and record sections, so it
/// costs about the same for a file of any size, and a damaged key block is reported only
/// when [`LazyMdx::keys`] reaches it. Read the keys once to build an index, then read
/// records by where they lie with [`LazyMdx::record`].
///
/// [`Mdx`](crate::Mdx) is the simpler interface when keeping every key in memory is fine.
pub struct LazyMdx {
    sections: Sections,
}

impl LazyMdx {
    /// Opens an MDX file. The file is memory-mapped; only its header and section tables
    /// are read now.
    pub fn open(path: impl AsRef<Path>) -> Result<LazyMdx> {
        Ok(LazyMdx {
            sections: Sections::open(path.as_ref(), Kind::Mdx, None)?,
        })
    }

    /// Opens an MDX file that may need a registration code (`Encrypted="1"`), unlocking
    /// it with `registration`. A file that needs none opens as with [`LazyMdx::open`].
    pub fn open_registered(path: impl AsRef<Path>, registration: &Registration) -> Result<LazyMdx> {
        Ok(LazyMdx {
            sections: Sections::open(path.as_ref(), Kind::Mdx, Some(registration))?,
        })
    }

    /// The file's header.
    pub fn header(&self) -> &Header {
        &self.sections.header
    }

    /// How many keys (headwords, repeats included) the file says it holds.
    pub fn key_count(&self) -> usize {
        self.sections.key_count
    }

    /// Every key in file order: the headword, and where its record starts in the
    /// decompressed record stream. Key blocks are decompressed one at a time as the
    /// iterator reaches them. After an error the iterator ends.
    ///
    /// A record ends where the next larger start among all keys begins, or at
    /// [`LazyMdx::record_stream_len`] for the last record. Several keys may share a start.
    pub fn keys(&self) -> impl Iterator<Item = Result<(String, u64)>> + '_ {
        let mut failed = false;
        (0..self.sections.key_block_count())
            .map_while(move |index| {
                if failed {
                    return None;
                }
                let block = self.sections.key_block(index);
                failed = block.is_err();
                Some(block)
            })
            .flat_map(|block| -> Box<dyn Iterator<Item = Result<(String, u64)>>> {
                match block {
                    Ok(keys) => Box::new(
                        keys.into_iter()
                            .map(|key| Ok((key.text, key.record_offset))),
                    ),
                    Err(error) => Box::new(std::iter::once(Err(error))),
                }
            })
    }

    /// The length of the decompressed record stream: where the last record ends.
    pub fn record_stream_len(&self) -> u64 {
        self.sections.record_stream_len()
    }

    /// The record that lies at `span` of the decompressed record stream, decompressing
    /// only the blocks that hold it. A span outside the stream is reported as damage.
    pub fn record(&self, span: Range<u64>) -> Result<Record> {
        let bytes = self.sections.record_bytes(span.start, span.end)?;
        Ok(decode_record(&self.sections.encoding.decode(&bytes)))
    }
}
