//! `Encrypted="1"` files: the key section header is encrypted with Salsa20/8 under a
//! 16-byte dictionary key that is not in the file. The buyer holds a registration code,
//! which is that key encrypted under RIPEMD-128 of their e-mail address or device ID.

use std::fmt;

use ripemd::{Digest, Ripemd128};

use crate::header::Header;
use crate::salsa;

/// What unlocks a file that needs a registration code: the e-mail address (or device
/// ID) the dictionary was registered to and, unless the file carries it, the code.
///
/// Its `Debug` output shows neither, so it is safe to log.
#[derive(Clone, PartialEq, Eq)]
pub struct Registration {
    identity: String,
    code: Option<[u8; 16]>,
}

impl Registration {
    /// A registration from an e-mail address (or device ID) and a code of 32 hex digits;
    /// spaces and dashes in the code are ignored. `None` when the code is not 32 hex digits.
    pub fn new(identity: &str, code: &str) -> Option<Registration> {
        Some(Registration {
            identity: identity.to_owned(),
            code: Some(parse_code(code)?),
        })
    }

    /// A registration from an e-mail address (or device ID) alone, for files that carry
    /// their own code (the `RegCode` header attribute).
    pub fn without_code(identity: &str) -> Registration {
        Registration {
            identity: identity.to_owned(),
            code: None,
        }
    }

    /// The dictionary keys this registration may stand for, most likely first; `None`
    /// when there is no code, neither given nor in the file. Descriptions of the format
    /// disagree on how the identity is turned into bytes, so there is one key per reading.
    pub(crate) fn candidate_keys(&self, header: &Header) -> Option<Vec<[u8; 16]>> {
        let code = match self.code {
            Some(code) => code,
            None => parse_code(header.attribute("RegCode")?)?,
        };
        let utf16: Vec<u8> = self
            .identity
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let forms = [utf16, self.identity.as_bytes().to_vec()];
        Some(
            forms
                .iter()
                .map(|identity| {
                    let identity_key: [u8; 16] = Ripemd128::digest(identity).into();
                    let mut key = code;
                    salsa::apply_keystream(&identity_key, salsa::MDICT_ROUNDS, &mut key);
                    key
                })
                .collect(),
        )
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registration").finish_non_exhaustive()
    }
}

fn parse_code(text: &str) -> Option<[u8; 16]> {
    let digits: Vec<u8> = text.bytes().filter(|b| !matches!(b, b' ' | b'-')).collect();
    if digits.len() != 32 || !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let mut code = [0u8; 16];
    for (byte, pair) in code.iter_mut().zip(digits.chunks(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(code)
}
