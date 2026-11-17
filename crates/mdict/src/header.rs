use std::collections::BTreeMap;

use crate::bytes::{Reader, adler32};
use crate::error::{Error, Result};

/// The header of an MDict file: the attributes of its `<Dictionary .../>` element.
#[derive(Debug, Clone)]
pub struct Header {
    attributes: BTreeMap<String, String>,
}

impl Header {
    /// The `Title` attribute, as written. May be empty.
    pub fn title(&self) -> &str {
        self.attribute("Title").unwrap_or("")
    }

    /// The `Description` attribute (often HTML), with XML entities unescaped.
    pub fn description(&self) -> &str {
        self.attribute("Description").unwrap_or("")
    }

    /// `GeneratedByEngineVersion`, the format version, e.g. `"2.0"`.
    pub fn engine_version(&self) -> &str {
        self.attribute("GeneratedByEngineVersion").unwrap_or("")
    }

    /// The `Encoding` attribute as written, e.g. `"UTF-8"`. Empty means UTF-8.
    pub fn encoding(&self) -> &str {
        self.attribute("Encoding").unwrap_or("")
    }

    /// The `Encrypted` attribute as a bit set; empty or `No` is 0.
    pub fn encrypted(&self) -> u8 {
        match self.attribute("Encrypted").unwrap_or("").trim() {
            "" | "No" => 0,
            "Yes" => 1,
            other => other.parse().unwrap_or(u8::MAX),
        }
    }

    /// Any header attribute by its exact name.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    /// Every header attribute as `(name, value)`, sorted by name.
    pub fn attributes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attributes
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Reads the header section: `u32 BE length`, UTF-16LE text, `u32 LE adler32`.
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Header> {
        let len = reader
            .u32_be()
            .map_err(|_| Error::NotMdict("the file is too short for a header"))?
            as usize;
        let text = reader
            .take(len)
            .map_err(|_| Error::NotMdict("the header runs past the end of the file"))?;
        let checksum = u32::from_le_bytes(
            reader
                .array()
                .map_err(|_| Error::NotMdict("the header has no checksum"))?,
        );
        if !len.is_multiple_of(2) || adler32(text) != checksum {
            return Err(Error::NotMdict("the header checksum does not match"));
        }
        let (pairs, _) = text.as_chunks::<2>();
        let units: Vec<u16> = pairs.iter().map(|&pair| u16::from_le_bytes(pair)).collect();
        let text = String::from_utf16_lossy(&units);
        let text = text.trim_end_matches(['\0', '\r', '\n', ' ']);
        if !text.starts_with("<Dictionary") && !text.starts_with("<Library_Data") {
            return Err(Error::NotMdict("the header is not a Dictionary element"));
        }
        Ok(Header {
            attributes: parse_attributes(text),
        })
    }
}

/// Reads `Name="value"` pairs leniently: real headers contain unescaped `<` and `&` inside values.
fn parse_attributes(element: &str) -> BTreeMap<String, String> {
    let mut attributes = BTreeMap::new();
    let mut rest = element;
    while let Some(eq) = rest.find("=\"") {
        let name = rest[..eq]
            .rsplit(|c: char| c.is_whitespace() || c == '<')
            .next()
            .unwrap_or("");
        let value_start = eq + 2;
        let Some(value_len) = rest[value_start..].find('"') else {
            break;
        };
        let value = &rest[value_start..value_start + value_len];
        if !name.is_empty() {
            attributes.insert(name.to_owned(), unescape_xml(value));
        }
        rest = &rest[value_start + value_len + 1..];
    }
    attributes
}

fn unescape_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
