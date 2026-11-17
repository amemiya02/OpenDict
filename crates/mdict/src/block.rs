//! Compressed blocks: `[type: 4 bytes][adler32 of the decompressed data, BE][payload]`.

use std::io::Read;

use flate2::read::ZlibDecoder;

use crate::bytes::adler32;
use crate::error::{Error, Result};

/// The largest block this parser will decompress. Real blocks are around 64 KiB; the cap
/// stops a damaged or hostile file from claiming an enormous size.
const MAX_BLOCK: usize = 64 * 1024 * 1024;

/// Decompresses one block whose decompressed size is known from an index table.
pub(crate) fn decompress(block: &[u8], expected_len: usize) -> Result<Vec<u8>> {
    if expected_len > MAX_BLOCK {
        return Err(Error::Corrupt("a block claims an implausible size"));
    }
    let (head, payload) = block
        .split_at_checked(8)
        .ok_or(Error::Corrupt("a block is shorter than its header"))?;
    let kind = u32::from_le_bytes(head[0..4].try_into().expect("4 bytes"));
    let checksum = u32::from_be_bytes(head[4..8].try_into().expect("4 bytes"));

    let data = match kind {
        0 => payload.to_vec(),
        2 => {
            let mut data = Vec::with_capacity(expected_len);
            ZlibDecoder::new(payload)
                .take(expected_len as u64 + 1)
                .read_to_end(&mut data)
                .map_err(|_| Error::Corrupt("a zlib block does not decompress"))?;
            data
        }
        1 => return Err(Error::Unsupported("LZO compression".to_owned())),
        other => return Err(Error::Unsupported(format!("block type {other:#x}"))),
    };

    if data.len() != expected_len {
        return Err(Error::Corrupt("a block decompresses to the wrong size"));
    }
    if adler32(&data) != checksum {
        return Err(Error::Corrupt("a block checksum does not match"));
    }
    Ok(data)
}
