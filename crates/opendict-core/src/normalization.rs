//! Normalization: the differences in how a word is written that do not tell headwords
//! apart. The index files every headword under its normalised form and Lookup normalises
//! the query the same way, through the one function [`normalize`].
//!
//! Traditional characters fold to Simplified ones by OpenCC's character table
//! (`data/opencc-TSCharacters.txt`, Apache-2.0, see `data/opencc-LICENSE`).

use std::collections::HashMap;
use std::sync::OnceLock;

use unicode_normalization::UnicodeNormalization;

/// OpenCC's Traditional-to-Simplified character table: a Traditional character, a tab,
/// then its Simplified forms separated by spaces, the usual one first.
const TRADITIONAL_TO_SIMPLIFIED: &str = include_str!("../data/opencc-TSCharacters.txt");

/// `text` with the differences that do not distinguish headwords removed: Unicode
/// compatibility variants (full-width and half-width forms among them), letter case,
/// diacritics, the difference between katakana and hiragana, and the difference between
/// Traditional and Simplified Chinese characters.
pub fn normalize(text: &str) -> String {
    let folded: String = text.nfkd().filter(|&c| !is_diacritic(c)).nfc().collect();
    let simplified = simplified();
    folded
        .to_lowercase()
        .chars()
        .map(|c| simplified.get(&c).copied().unwrap_or_else(|| hiragana(c)))
        .collect()
}

fn simplified() -> &'static HashMap<char, char> {
    static TABLE: OnceLock<HashMap<char, char>> = OnceLock::new();
    TABLE.get_or_init(|| {
        TRADITIONAL_TO_SIMPLIFIED
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| {
                let (traditional, simplified) = line.split_once('\t')?;
                let mut traditional = traditional.chars();
                let from = traditional
                    .next()
                    .filter(|_| traditional.as_str().is_empty())?;
                let to = simplified.chars().next()?;
                Some((from, to))
            })
            .collect()
    })
}

/// The hiragana for a katakana letter; any other character as it is.
fn hiragana(c: char) -> char {
    match c {
        // ァ..ヶ and the iteration marks ヽ ヾ sit 0x60 above their hiragana.
        'ァ'..='ヶ' | 'ヽ' | 'ヾ' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
        _ => c,
    }
}

/// Combining marks that only add a diacritic to a letter. The kana voicing marks are
/// not among them: か and が are different words.
fn is_diacritic(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'
        | '\u{1AB0}'..='\u{1AFF}'
        | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}'
        | '\u{FE20}'..='\u{FE2F}')
}
