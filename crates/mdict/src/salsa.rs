//! Salsa20 with a 16-byte key, written from D. J. Bernstein's public-domain
//! specification ("Salsa20 specification", cr.yp.to/snuffle/spec.pdf). MDict uses it
//! with 8 rounds (Salsa20/8) to encrypt the key section header of `Encrypted="1"` files.
//!
//! RustCrypto's `salsa20` crate only takes 32-byte keys, hence this small core.

/// Salsa20/8: the round count MDict uses.
pub(crate) const MDICT_ROUNDS: usize = 8;

/// XORs `data` with the Salsa20 key stream for `key` under an all-zero nonce, counting
/// blocks from 0. Encryption and decryption are the same operation.
pub(crate) fn apply_keystream(key: &[u8; 16], rounds: usize, data: &mut [u8]) {
    for (block, chunk) in data.chunks_mut(64).enumerate() {
        // n = nonce (8 bytes, all zero) followed by the block number, little-endian.
        let mut n = [0u8; 16];
        n[8..].copy_from_slice(&(block as u64).to_le_bytes());
        let stream = expand(key, &n, rounds);
        for (byte, key_byte) in chunk.iter_mut().zip(stream) {
            *byte ^= key_byte;
        }
    }
}

/// The expansion function for a 16-byte key (specification section 9):
/// `hash(tau0, k, tau1, n, tau2, k, tau3)` with tau = "expand 16-byte k".
fn expand(key: &[u8; 16], n: &[u8; 16], rounds: usize) -> [u8; 64] {
    const TAU: &[u8; 16] = b"expand 16-byte k";
    let mut input = [0u8; 64];
    input[0..4].copy_from_slice(&TAU[0..4]);
    input[4..20].copy_from_slice(key);
    input[20..24].copy_from_slice(&TAU[4..8]);
    input[24..40].copy_from_slice(n);
    input[40..44].copy_from_slice(&TAU[8..12]);
    input[44..60].copy_from_slice(key);
    input[60..64].copy_from_slice(&TAU[12..16]);
    hash(&input, rounds)
}

/// The Salsa20 hash function (specification section 8) with `rounds` rounds (an even
/// number): `rounds / 2` double rounds over the input words, then the input added back.
fn hash(input: &[u8; 64], rounds: usize) -> [u8; 64] {
    let (chunks, _) = input.as_chunks::<4>();
    let x: [u32; 16] = std::array::from_fn(|i| u32::from_le_bytes(chunks[i]));
    let mut z = x;
    for _ in 0..rounds / 2 {
        // Column round, then row round.
        quarter_round(&mut z, [0, 4, 8, 12]);
        quarter_round(&mut z, [5, 9, 13, 1]);
        quarter_round(&mut z, [10, 14, 2, 6]);
        quarter_round(&mut z, [15, 3, 7, 11]);
        quarter_round(&mut z, [0, 1, 2, 3]);
        quarter_round(&mut z, [5, 6, 7, 4]);
        quarter_round(&mut z, [10, 11, 8, 9]);
        quarter_round(&mut z, [15, 12, 13, 14]);
    }
    let mut out = [0u8; 64];
    for (i, word) in out.chunks_exact_mut(4).enumerate() {
        word.copy_from_slice(&z[i].wrapping_add(x[i]).to_le_bytes());
    }
    out
}

/// The quarterround function (specification section 3) on the words at `[a, b, c, d]`.
fn quarter_round(y: &mut [u32; 16], [a, b, c, d]: [usize; 4]) {
    y[b] ^= y[a].wrapping_add(y[d]).rotate_left(7);
    y[c] ^= y[b].wrapping_add(y[a]).rotate_left(9);
    y[d] ^= y[c].wrapping_add(y[b]).rotate_left(13);
    y[a] ^= y[d].wrapping_add(y[c]).rotate_left(18);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        let digits: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
        digits
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    // Section 8 of the specification: the Salsa20 hash function (20 rounds).
    #[test]
    fn the_hash_of_zeros_is_zeros() {
        assert_eq!(hash(&[0; 64], 20), [0; 64]);
    }

    #[test]
    fn the_hash_matches_the_specification_example() {
        let input = [
            211, 159, 13, 115, 76, 55, 82, 183, 3, 117, 222, 37, 191, 187, 234, 136, 49, 237, 179,
            48, 1, 106, 178, 219, 175, 199, 166, 48, 86, 16, 179, 207, 31, 240, 32, 63, 15, 83,
            93, 161, 116, 147, 48, 113, 238, 55, 204, 36, 79, 201, 235, 79, 3, 81, 156, 47, 203,
            26, 244, 243, 88, 118, 104, 54,
        ];
        let output = [
            109, 42, 178, 168, 156, 240, 248, 238, 168, 196, 190, 203, 26, 110, 170, 154, 29, 29,
            150, 26, 150, 30, 235, 249, 190, 163, 251, 48, 69, 144, 51, 57, 118, 40, 152, 157,
            180, 57, 27, 94, 107, 42, 236, 35, 27, 111, 114, 114, 219, 236, 232, 135, 111, 155,
            110, 18, 24, 232, 95, 158, 179, 19, 48, 202,
        ];
        assert_eq!(hash(&input, 20), output);
    }

    // RFC 7914, section 8: the Salsa20/8 core, i.e. the same hash with 8 rounds.
    #[test]
    fn the_eight_round_hash_matches_rfc_7914() {
        let input = hex(
            "7e879a214f3ec9867ca940e641718f26baee555b8c61c1b50df846116dcd3b1d
             ee24f319df9b3d8514121e4b5ac5aa3276021d2909c74829edebc68db8b8c25e",
        );
        let output = hex(
            "a41f859c6608cc993b81cacb020cef05044b2181a2fd337dfd7b1c6396682f29
             b4393168e3c9e6bcfe6bc5b7a06d96bae424cc102c91745c24ad673dc7618f81",
        );
        assert_eq!(hash(&input.try_into().unwrap(), 8).to_vec(), output);
    }

    // Section 9 of the specification: expansion of a 16-byte key k0 = (1..=16) with
    // n = (101..=116).
    #[test]
    fn a_sixteen_byte_key_expands_as_the_specification_shows() {
        let key: [u8; 16] = std::array::from_fn(|i| i as u8 + 1);
        let n: [u8; 16] = std::array::from_fn(|i| i as u8 + 101);
        let output = [
            39, 173, 46, 248, 30, 200, 82, 17, 48, 67, 254, 239, 37, 18, 13, 247, 241, 200, 61,
            144, 10, 55, 50, 185, 6, 47, 246, 253, 143, 86, 187, 225, 134, 85, 110, 246, 161, 163,
            43, 235, 231, 94, 171, 51, 145, 214, 112, 29, 14, 232, 5, 16, 151, 140, 183, 141, 171,
            9, 122, 181, 104, 182, 177, 193,
        ];
        assert_eq!(expand(&key, &n, 20), output);
    }

    #[test]
    fn the_key_stream_is_the_expansion_of_nonce_zero_and_each_block_number() {
        let key: [u8; 16] = std::array::from_fn(|i| i as u8 * 7);
        let mut data = vec![0u8; 150];
        apply_keystream(&key, 8, &mut data);

        for (block, chunk) in data.chunks(64).enumerate() {
            let mut n = [0u8; 16];
            n[8..].copy_from_slice(&(block as u64).to_le_bytes());
            assert_eq!(chunk, &expand(&key, &n, 8)[..chunk.len()], "block {block}");
        }
    }
}
