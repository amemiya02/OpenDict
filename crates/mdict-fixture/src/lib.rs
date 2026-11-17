//! Writes small MDict files for OpenDict's tests.
//!
//! The tests own this writer: no real dictionary content is ever committed, so every
//! fixture is generated from entries written in the test itself. It writes MDX
//! ([`MdxBuilder`]) and MDD ([`MddBuilder`]) files in format versions 1.2 and 2.0, with
//! blocks stored plain, zlib- or LZO-compressed, in UTF-8, UTF-16, GBK or Big5, with or
//! without an encrypted key index (`Encrypted="2"`) or a registration code
//! (`Encrypted="1"`), and can damage what it writes.
//!
//! ```
//! use opendict_mdict_fixture::{Compression, Encoding, MdxBuilder, Version};
//!
//! let bytes = MdxBuilder::new()
//!     .title("Sample")
//!     .version(Version::V1_2)
//!     .encoding(Encoding::Gbk)
//!     .compression(Compression::Lzo)
//!     .entry("apple", "<p>a fruit</p>")
//!     .build();
//! assert!(!bytes.is_empty());
//! ```

use std::io::{self, Write};
use std::path::Path;

use flate2::write::ZlibEncoder;
use ripemd::{Digest, Ripemd128};

/// The MDict format version a file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// `GeneratedByEngineVersion="1.2"`: 32-bit numbers, a raw key index.
    V1_2,
    /// `GeneratedByEngineVersion="2.0"`: 64-bit numbers, a compressed, checksummed key index.
    V2_0,
}

/// The text encoding of an MDX file's headwords and records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    /// UTF-16 little-endian, written `Encoding="UTF-16"`.
    Utf16,
    Gbk,
    Big5,
}

/// How the writer stores each block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Block type 0: stored as is.
    None,
    /// Block type 1: an LZO1X stream.
    Lzo,
    /// Block type 2: a zlib stream.
    Zlib,
}

/// How a dictionary's publisher turned the buyer's e-mail or device ID into bytes before
/// hashing it into the key that decrypts the registration code. Public descriptions of
/// MDict disagree, so readers try both and the tests write both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// UTF-16 little-endian.
    Utf16,
    /// The bytes as written (ASCII for any real e-mail address or device ID).
    Ascii,
}

/// A way to damage a written file, for testing that readers report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Damage {
    /// The header's checksum no longer matches its text.
    HeaderChecksum,
    /// A byte of the key index (the key block info table) is changed.
    KeyIndex,
    /// A byte in the middle of the first key block's payload is changed.
    KeyBlock,
    /// A byte in the middle of the first record block's payload is changed.
    RecordBlock,
    /// The last `n` bytes of the file are cut off.
    Truncate(usize),
}

impl Version {
    pub const ALL: [Version; 2] = [Version::V1_2, Version::V2_0];

    fn engine_version(self) -> &'static str {
        match self {
            Version::V1_2 => "1.2",
            Version::V2_0 => "2.0",
        }
    }

    /// Appends a size, count or offset in this version's width (u32 or u64, big-endian).
    fn put(self, out: &mut Vec<u8>, n: usize) {
        match self {
            Version::V1_2 => out.extend(u32::try_from(n).expect("fits in u32").to_be_bytes()),
            Version::V2_0 => out.extend((n as u64).to_be_bytes()),
        }
    }
}

impl Encoding {
    pub const ALL: [Encoding; 4] = [
        Encoding::Utf8,
        Encoding::Utf16,
        Encoding::Gbk,
        Encoding::Big5,
    ];

    /// The `Encoding` header attribute.
    fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf16 => "UTF-16",
            Encoding::Gbk => "GBK",
            Encoding::Big5 => "BIG5",
        }
    }

    /// Bytes per code unit, which is also the length of a NUL terminator.
    fn unit(self) -> usize {
        match self {
            Encoding::Utf16 => 2,
            _ => 1,
        }
    }

    /// Encodes `text`; panics if the encoding cannot represent it, so a fixture never
    /// silently holds replacement characters.
    fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Encoding::Utf8 => text.as_bytes().to_vec(),
            Encoding::Utf16 => utf16le(text),
            Encoding::Gbk | Encoding::Big5 => {
                let encoding = if self == Encoding::Gbk {
                    encoding_rs::GBK
                } else {
                    encoding_rs::BIG5
                };
                let (bytes, _, unmappable) = encoding.encode(text);
                assert!(!unmappable, "{text:?} cannot be written in {self:?}");
                bytes.into_owned()
            }
        }
    }
}

impl Compression {
    pub const ALL: [Compression; 3] = [Compression::None, Compression::Lzo, Compression::Zlib];
}

/// The layout settings MDX and MDD files share.
#[derive(Debug, Clone)]
struct Layout {
    version: Version,
    compression: Compression,
    encrypt_key_index: bool,
    /// The key the key section header is encrypted with (`Encrypted="1"`), if any.
    header_key: Option<[u8; 16]>,
    keys_per_block: usize,
    records_per_block: usize,
    /// Header attributes that replace or add to the written ones, in order.
    attributes: Vec<(String, String)>,
}

impl Layout {
    fn new() -> Self {
        Self {
            version: Version::V2_0,
            compression: Compression::Zlib,
            encrypt_key_index: false,
            header_key: None,
            keys_per_block: 64,
            records_per_block: 64,
            attributes: Vec::new(),
        }
    }

    fn encrypted_attribute(&self) -> &'static str {
        match (self.header_key.is_some(), self.encrypt_key_index) {
            (false, false) => "0",
            (true, false) => "1",
            (false, true) => "2",
            (true, true) => "3",
        }
    }
}

/// Setters and build methods both builders share.
macro_rules! layout_methods {
    () => {
        /// The format version (default 2.0).
        pub fn version(mut self, version: Version) -> Self {
            self.layout.version = version;
            self
        }

        /// How blocks are stored (default zlib). Applies to key blocks, record blocks
        /// and, in version 2.0, the key index.
        pub fn compression(mut self, compression: Compression) -> Self {
            self.layout.compression = compression;
            self
        }

        /// Encrypts the key index as `Encrypted="2"` files do. Version 2.0 only: a
        /// version 1.2 key index is stored raw, so asking for both panics at build time.
        pub fn encrypt_key_index(mut self) -> Self {
            self.layout.encrypt_key_index = true;
            self
        }

        /// Encrypts the key section header as `Encrypted="1"` files do, so that the file
        /// opens only with `code` (32 hex digits) and the e-mail or device ID `identity`
        /// it was issued to. The code is not written into the file; add it with
        /// `.attribute("RegCode", code)` to write a file that carries its own code.
        pub fn registered_to(mut self, identity: &str, code: &str, form: Identity) -> Self {
            self.layout.header_key = Some(dictionary_key(identity, code, form));
            self
        }

        /// Sets a header attribute, replacing the one the writer would write. Use it to
        /// write headers the writer cannot produce otherwise (`Encrypted="1"`,
        /// `GeneratedByEngineVersion="3.0"`); the rest of the file is not changed to match.
        pub fn attribute(mut self, name: &str, value: &str) -> Self {
            self.layout
                .attributes
                .push((name.to_owned(), value.to_owned()));
            self
        }

        /// How many keys go into each key block (default 64), to exercise multi-block files.
        pub fn keys_per_block(mut self, n: usize) -> Self {
            self.layout.keys_per_block = n.max(1);
            self
        }

        /// How many records go into each record block (default 64).
        pub fn records_per_block(mut self, n: usize) -> Self {
            self.layout.records_per_block = n.max(1);
            self
        }

        /// Writes the file and returns its bytes.
        pub fn build(&self) -> Vec<u8> {
            self.build_marked().0
        }

        /// Writes the file, then damages it.
        pub fn build_damaged(&self, damage: Damage) -> Vec<u8> {
            let (mut bytes, marks) = self.build_marked();
            marks.apply(&mut bytes, damage);
            bytes
        }

        /// Writes the file to `path`.
        pub fn write_to(&self, path: impl AsRef<Path>) -> io::Result<()> {
            std::fs::write(path, self.build())
        }
    };
}

/// Builds an MDX file from headwords and their records.
#[derive(Debug, Clone)]
pub struct MdxBuilder {
    title: String,
    description: String,
    encoding: Encoding,
    entries: Vec<(String, String)>,
    layout: Layout,
}

impl Default for MdxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MdxBuilder {
    /// A version 2.0, UTF-8, zlib MDX with no entries.
    pub fn new() -> Self {
        Self {
            title: "Fixture".to_owned(),
            description: "A fixture written by OpenDict's tests".to_owned(),
            encoding: Encoding::Utf8,
            entries: Vec::new(),
            layout: Layout::new(),
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

    /// The encoding of headwords and records (default UTF-8).
    pub fn encoding(mut self, encoding: Encoding) -> Self {
        self.encoding = encoding;
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

    layout_methods!();

    fn build_marked(&self) -> (Vec<u8>, Marks) {
        let version = self.layout.version.engine_version();
        let attributes = [
            ("GeneratedByEngineVersion", version),
            ("RequiredEngineVersion", version),
            ("Encrypted", self.layout.encrypted_attribute()),
            ("Encoding", self.encoding.name()),
            ("Format", "Html"),
            ("Title", &self.title),
            ("Description", &self.description),
            ("KeyCaseSensitive", "No"),
            ("StripKey", "Yes"),
            ("StyleSheet", ""),
            ("CreationDate", "2026-1-1"),
            ("Compact", "No"),
            ("Compat", "No"),
            ("Left2Right", "Yes"),
            ("DataSourceFormat", "106"),
        ];
        let keys: Vec<Vec<u8>> = self
            .entries
            .iter()
            .map(|(key, _)| self.encoding.encode(key))
            .collect();
        // Each record is its text, CR LF, then a NUL in the record's encoding.
        let records: Vec<Vec<u8>> = self
            .entries
            .iter()
            .map(|(_, record)| self.encoding.encode(&format!("{record}\r\n\0")))
            .collect();
        write_file(
            "Dictionary",
            &attributes,
            &self.layout,
            self.encoding.unit(),
            &keys,
            &records,
        )
    }
}

/// Builds an MDD resource file from resource paths and their bytes.
#[derive(Debug, Clone)]
pub struct MddBuilder {
    resources: Vec<(String, Vec<u8>)>,
    layout: Layout,
}

impl Default for MddBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MddBuilder {
    /// A version 2.0, zlib MDD with no resources.
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            layout: Layout::new(),
        }
    }

    /// Adds a resource under `path`, written as given (MDD paths look like `\img\a.png`).
    pub fn resource(mut self, path: &str, bytes: &[u8]) -> Self {
        self.resources.push((path.to_owned(), bytes.to_vec()));
        self
    }

    layout_methods!();

    fn build_marked(&self) -> (Vec<u8>, Marks) {
        let version = self.layout.version.engine_version();
        let attributes = [
            ("GeneratedByEngineVersion", version),
            ("RequiredEngineVersion", version),
            ("Encrypted", self.layout.encrypted_attribute()),
            ("Format", ""),
            ("Title", ""),
            ("Description", ""),
            ("KeyCaseSensitive", "No"),
            ("StripKey", "No"),
            ("CreationDate", "2026-1-1"),
        ];
        // Keys are always UTF-16LE; records are the raw bytes, with no terminator.
        let keys: Vec<Vec<u8>> = self
            .resources
            .iter()
            .map(|(path, _)| utf16le(path))
            .collect();
        let records: Vec<Vec<u8>> = self
            .resources
            .iter()
            .map(|(_, bytes)| bytes.clone())
            .collect();
        write_file(
            "Library_Data",
            &attributes,
            &self.layout,
            2,
            &keys,
            &records,
        )
    }
}

/// Where the bytes a [`Damage`] changes ended up in the written file.
struct Marks {
    header_checksum: usize,
    key_index: usize,
    first_key_block: usize,
    first_record_block: usize,
}

impl Marks {
    fn apply(&self, bytes: &mut Vec<u8>, damage: Damage) {
        match damage {
            Damage::HeaderChecksum => bytes[self.header_checksum] ^= 0xFF,
            Damage::KeyIndex => bytes[self.key_index] ^= 0x55,
            Damage::KeyBlock => bytes[self.first_key_block] ^= 0xFF,
            Damage::RecordBlock => bytes[self.first_record_block] ^= 0xFF,
            Damage::Truncate(n) => bytes.truncate(bytes.len().saturating_sub(n)),
        }
    }
}

/// Writes a whole file. `keys` are encoded without terminators; `records` are complete.
fn write_file(
    root: &str,
    attributes: &[(&str, &str)],
    layout: &Layout,
    unit: usize,
    keys: &[Vec<u8>],
    records: &[Vec<u8>],
) -> (Vec<u8>, Marks) {
    assert_eq!(keys.len(), records.len());
    assert!(
        !(layout.encrypt_key_index && layout.version == Version::V1_2),
        "a version 1.2 key index is stored raw and cannot be encrypted"
    );
    let mut out = header_section(root, attributes, &layout.attributes);
    let header_checksum = out.len() - 4;

    let mut offsets = Vec::with_capacity(records.len());
    let mut running = 0usize;
    for record in records {
        offsets.push(running);
        running += record.len();
    }

    let keys_start = out.len();
    let (key_section, key_index, first_key_block) = key_section(layout, unit, keys, &offsets);
    out.extend(key_section);
    let records_start = out.len();
    let (record_section, first_record_block) = record_section(layout, records);
    out.extend(record_section);

    let marks = Marks {
        header_checksum,
        key_index: keys_start + key_index,
        first_key_block: keys_start + first_key_block,
        first_record_block: records_start + first_record_block,
    };
    (out, marks)
}

fn header_section(
    root: &str,
    attributes: &[(&str, &str)],
    overrides: &[(String, String)],
) -> Vec<u8> {
    let mut pairs: Vec<(String, String)> = attributes
        .iter()
        .map(|&(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
    for (name, value) in overrides {
        match pairs.iter_mut().find(|(n, _)| n == name) {
            Some(pair) => pair.1.clone_from(value),
            None => pairs.push((name.clone(), value.clone())),
        }
    }
    let mut text = format!("<{root}");
    for (name, value) in &pairs {
        text.push_str(&format!(" {name}=\"{}\"", escape_xml(value)));
    }
    text.push_str("/>\r\n\0");

    let header = utf16le(&text);
    let mut out = Vec::new();
    out.extend((header.len() as u32).to_be_bytes());
    out.extend(&header);
    out.extend(adler32(&header).to_le_bytes());
    out
}

/// The middle byte of a block's payload (after its 8-byte header), relative to the block.
fn payload_middle(block: &[u8]) -> usize {
    assert!(block.len() > 8, "the block has no payload to damage");
    8 + (block.len() - 8) / 2
}

/// Returns the key section, the position in it of the byte [`Damage::KeyIndex`] changes,
/// and the position of the byte [`Damage::KeyBlock`] changes.
fn key_section(
    layout: &Layout,
    unit: usize,
    keys: &[Vec<u8>],
    offsets: &[usize],
) -> (Vec<u8>, usize, usize) {
    let version = layout.version;
    let terminator = vec![0u8; unit];
    let mut info = Vec::new();
    let mut blocks = Vec::new();
    let indices: Vec<usize> = (0..keys.len()).collect();
    for chunk in indices.chunks(layout.keys_per_block) {
        let mut plain = Vec::new();
        for &i in chunk {
            version.put(&mut plain, offsets[i]);
            plain.extend(&keys[i]);
            plain.extend(&terminator);
        }
        let block = block(&plain, layout.compression);

        version.put(&mut info, chunk.len());
        for word in [&keys[chunk[0]], &keys[chunk[chunk.len() - 1]]] {
            let units = word.len() / unit;
            match version {
                Version::V1_2 => {
                    info.push(u8::try_from(units).expect("a v1.2 index word fits in 255 units"));
                    info.extend(word);
                }
                Version::V2_0 => {
                    info.extend(u16::try_from(units).expect("fits in u16").to_be_bytes());
                    info.extend(word);
                    info.extend(&terminator);
                }
            }
        }
        version.put(&mut info, block.len());
        version.put(&mut info, plain.len());
        blocks.push(block);
    }
    let (stored_info, info_damage) = match version {
        // The table is stored raw; its first byte is the high byte of the first block's
        // entry count, so changing it cannot go unnoticed.
        Version::V1_2 => (info.clone(), 0),
        Version::V2_0 => {
            let mut stored = block(&info, layout.compression);
            if layout.encrypt_key_index {
                encrypt_key_index(&mut stored);
            }
            let middle = payload_middle(&stored);
            (stored, middle)
        }
    };
    let blocks_len: usize = blocks.iter().map(Vec::len).sum();

    let mut head = Vec::new();
    version.put(&mut head, blocks.len());
    version.put(&mut head, keys.len());
    if version == Version::V2_0 {
        version.put(&mut head, info.len());
    }
    version.put(&mut head, stored_info.len());
    version.put(&mut head, blocks_len);

    let mut out = head.clone();
    if let Some(key) = &layout.header_key {
        salsa20_8(key, &mut out);
    }
    // The checksum is of the header in the clear.
    if version == Version::V2_0 {
        out.extend(adler32(&head).to_be_bytes());
    }
    let key_index = out.len() + info_damage;
    out.extend(stored_info);
    let first_block = out.len() + blocks.first().map_or(0, |block| payload_middle(block));
    for block in blocks {
        out.extend(block);
    }
    (out, key_index, first_block)
}

/// Returns the record section and the position in it of the byte
/// [`Damage::RecordBlock`] changes.
fn record_section(layout: &Layout, records: &[Vec<u8>]) -> (Vec<u8>, usize) {
    let version = layout.version;
    let mut sizes = Vec::new();
    let mut blocks = Vec::new();
    for chunk in records.chunks(layout.records_per_block) {
        let plain = chunk.concat();
        let block = block(&plain, layout.compression);
        sizes.push((block.len(), plain.len()));
        blocks.push(block);
    }
    let blocks_len: usize = sizes.iter().map(|(stored, _)| stored).sum();
    let number_width = match version {
        Version::V1_2 => 4,
        Version::V2_0 => 8,
    };

    let mut out = Vec::new();
    version.put(&mut out, blocks.len());
    version.put(&mut out, records.len());
    version.put(&mut out, 2 * number_width * blocks.len());
    version.put(&mut out, blocks_len);
    for (stored, plain) in sizes {
        version.put(&mut out, stored);
        version.put(&mut out, plain);
    }
    let first_block = out.len() + blocks.first().map_or(0, |block| payload_middle(block));
    for block in blocks {
        out.extend(block);
    }
    (out, first_block)
}

/// `[type, u32 LE] [adler32(data), u32 BE] [payload]`
fn block(data: &[u8], compression: Compression) -> Vec<u8> {
    let (kind, payload) = match compression {
        Compression::None => (0u32, data.to_vec()),
        Compression::Lzo => (
            1,
            lzokay::compress::compress(data).expect("LZO compression of a fixture"),
        ),
        Compression::Zlib => {
            let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder
                .write_all(data)
                .expect("writing to a Vec cannot fail");
            (2, encoder.finish().expect("writing to a Vec cannot fail"))
        }
    };
    let mut out = kind.to_le_bytes().to_vec();
    out.extend(adler32(data).to_be_bytes());
    out.extend(payload);
    out
}

/// Encrypts a stored key index block in place, as `Encrypted="2"` files store it: the
/// payload after the 8-byte block header goes through a byte-wise cipher keyed by
/// RIPEMD-128 of the block's checksum followed by `95 36 00 00`.
fn encrypt_key_index(block: &mut [u8]) {
    let mut seed = block[4..8].to_vec();
    seed.extend([0x95, 0x36, 0x00, 0x00]);
    let key = Ripemd128::digest(&seed);
    let mut previous = 0x36u8;
    for (i, byte) in block[8..].iter_mut().enumerate() {
        let mixed = *byte ^ previous ^ (i as u8) ^ key[i % 16];
        *byte = mixed.rotate_left(4);
        previous = *byte;
    }
}

/// The key that decrypts the key section header: the registration code decrypted with
/// Salsa20/8 under RIPEMD-128 of the identity. (Writing a file, we choose the code and
/// derive the key; the publisher of a real file chose the key and derived the code. The
/// cipher is its own inverse, so both directions agree.)
fn dictionary_key(identity: &str, code: &str, form: Identity) -> [u8; 16] {
    let digits: Vec<u8> = code.bytes().filter(u8::is_ascii_hexdigit).collect();
    assert_eq!(digits.len(), 32, "a registration code is 32 hex digits");
    let mut key = [0u8; 16];
    for (byte, pair) in key.iter_mut().zip(digits.chunks(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII"), 16).expect("hex");
    }
    let identity = match form {
        Identity::Utf16 => utf16le(identity),
        Identity::Ascii => identity.as_bytes().to_vec(),
    };
    let identity_key: [u8; 16] = Ripemd128::digest(&identity).into();
    salsa20_8(&identity_key, &mut key);
    key
}

/// Salsa20/8 with a 16-byte key, an all-zero nonce and the block counter from 0, XORed
/// into `data`. From the public-domain Salsa20 specification.
fn salsa20_8(key: &[u8; 16], data: &mut [u8]) {
    let word = |bytes: &[u8]| u32::from_le_bytes(bytes.try_into().expect("4 bytes"));
    let tau = b"expand 16-byte k";
    for (counter, chunk) in data.chunks_mut(64).enumerate() {
        let counter = counter as u64;
        let input: [u32; 16] = [
            word(&tau[0..4]),
            word(&key[0..4]),
            word(&key[4..8]),
            word(&key[8..12]),
            word(&key[12..16]),
            word(&tau[4..8]),
            0,
            0,
            counter as u32,
            (counter >> 32) as u32,
            word(&tau[8..12]),
            word(&key[0..4]),
            word(&key[4..8]),
            word(&key[8..12]),
            word(&key[12..16]),
            word(&tau[12..16]),
        ];
        let mut x = input;
        for _ in 0..4 {
            for [a, b, c, d] in [
                [0, 4, 8, 12],
                [5, 9, 13, 1],
                [10, 14, 2, 6],
                [15, 3, 7, 11],
                [0, 1, 2, 3],
                [5, 6, 7, 4],
                [10, 11, 8, 9],
                [15, 12, 13, 14],
            ] {
                x[b] ^= x[a].wrapping_add(x[d]).rotate_left(7);
                x[c] ^= x[b].wrapping_add(x[a]).rotate_left(9);
                x[d] ^= x[c].wrapping_add(x[b]).rotate_left(13);
                x[a] ^= x[d].wrapping_add(x[c]).rotate_left(18);
            }
        }
        let stream: Vec<u8> = x
            .iter()
            .zip(input)
            .flat_map(|(&z, x)| z.wrapping_add(x).to_le_bytes())
            .collect();
        for (byte, k) in chunk.iter_mut().zip(stream) {
            *byte ^= k;
        }
    }
}

fn utf16le(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
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
