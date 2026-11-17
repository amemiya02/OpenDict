//! A reader for MDict dictionary files (MDX and MDD).
//!
//! Written from public descriptions of the format. Open an MDX file with [`Mdx::open`],
//! then read its [`Header`], walk its headwords in file order, or look up the
//! records filed under a headword.

mod block;
mod bytes;
mod error;
mod header;
mod mdx;

pub use error::{Error, Result};
pub use header::Header;
pub use mdx::{Mdx, Record};
