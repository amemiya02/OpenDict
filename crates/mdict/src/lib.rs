//! A reader for MDict dictionary files (MDX and MDD).
//!
//! Written from public descriptions of the format. Reads format versions 1.2 and 2.0,
//! plain, zlib or LZO blocks, UTF-8, UTF-16, GBK and Big5 text, and files whose key
//! index is encrypted (`Encrypted="2"`).
//!
//! Open an MDX file with [`Mdx::open`], then read its [`Header`], walk its headwords in
//! file order, or look up the records filed under a headword. Open an MDD resource file
//! with [`Mdd::open`] and fetch resources by path.

mod block;
mod bytes;
mod error;
mod file;
mod header;
mod mdd;
mod mdx;

pub use error::{Error, Result};
pub use header::Header;
pub use mdd::Mdd;
pub use mdx::{Mdx, Record};
