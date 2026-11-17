//! A reader for MDict dictionary files (MDX and MDD).
//!
//! Written from public descriptions of the format. Open an MDX file with [`Mdx::open`],
//! then read its [`Header`], walk its headwords in file order, or look up the
//! records filed under a headword. Open an MDD file with [`Mdd::open`] and fetch its
//! resources by path.

mod block;
mod bytes;
mod error;
mod header;
mod mdd;
mod mdx;

pub use error::{Error, Result};
pub use header::Header;
pub use mdd::Mdd;
pub use mdx::{Mdx, Record};
