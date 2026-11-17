//! Bounds-checked reading of the big-endian numbers and slices MDict is made of.

use crate::error::{Error, Result};

/// A cursor over a byte slice. Every read checks bounds, so a damaged or hostile file
/// produces an error instead of a panic.
#[derive(Clone)]
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    /// How many bytes are left to read.
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(Error::Corrupt("data runs past its end"))?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("take returned N bytes"))
    }

    pub(crate) fn u16_be(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub(crate) fn u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub(crate) fn u64_be(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    /// Text up to (not including) the next NUL code unit of `unit` bytes (1, or 2 for
    /// UTF-16), which is consumed. Code units are counted from the current position, so
    /// a zero byte pair straddling two UTF-16 units is not a terminator.
    pub(crate) fn until_nul(&mut self, unit: usize) -> Result<&'a [u8]> {
        let rest = &self.bytes[self.pos..];
        let len = rest
            .chunks_exact(unit)
            .position(|chunk| chunk.iter().all(|&b| b == 0))
            .ok_or(Error::Corrupt("text is not NUL-terminated"))?
            * unit;
        self.pos += len + unit;
        Ok(&rest[..len])
    }
}

/// Adler-32 as defined in RFC 1950.
pub(crate) fn adler32(data: &[u8]) -> u32 {
    adler2::adler32_slice(data)
}
