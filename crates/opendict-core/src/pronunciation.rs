//! How a headword is said, read from its Entry where the Entry shows that plainly.
//!
//! Dictionary formats carry no pronunciation field: it is part of each Entry's HTML. This
//! looks for it the way a reader would, in three places, and gives up rather than guess:
//!
//! 1. an element the Dictionary marks as a pronunciation (`class="phon"`, `"pron"`, `"ipa"`,
//!    `"hatsuon"`, `"pinyin"`, …, or `data-type="発音"`);
//! 2. a phonetic transcription between slashes (`/ˈserɪf/`), recognised by its IPA letters;
//! 3. a Japanese reading in kana before the headword's brackets (`たべる【食べる】`).
//!
//! The result is plain text, never markup.

/// Class names Dictionaries give the element that holds a pronunciation. Compared without
/// regard to case.
const MARKS: &[&str] = &[
    "phon",
    "phonetic",
    "phonetics",
    "pron",
    "pronunciation",
    "pr",
    "ph",
    "ipa",
    "hatsuon",
    "pinyin",
    "ty_pinyin",
    "yomi",
];

/// Longer than this, a marked element holds more than a pronunciation.
const MAX_CHARS: usize = 64;
/// The longest transcription looked for between slashes.
const MAX_SLASHED: usize = 48;

/// How the headword is said, as this Entry's HTML shows it, if it shows it plainly.
pub fn pronunciation(html: &str) -> Option<String> {
    let tokens = tokenize(html);
    marked(&tokens)
        .or_else(|| slashed(&tokens))
        .or_else(|| reading(&tokens))
}

/// The text of the first element marked as a pronunciation that holds a short one.
fn marked(tokens: &[Token]) -> Option<String> {
    for (start, token) in tokens.iter().enumerate() {
        let Token::Open { marked: true, .. } = token else {
            continue;
        };
        let mut depth = 0usize;
        let mut text = String::new();
        for inner in &tokens[start..] {
            match inner {
                Token::Open { .. } => depth += 1,
                Token::Close => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Token::Text(t) => text.push_str(t),
            }
        }
        if let Some(found) = short(&text) {
            return Some(found);
        }
    }
    None
}

/// The first `/…/` in the Entry's text that holds IPA letters.
fn slashed(tokens: &[Token]) -> Option<String> {
    let text = all_text(tokens);
    let mut rest = text.as_str();
    while let Some(open) = rest.find('/') {
        let after = &rest[open + 1..];
        let close = after.find('/')?;
        let inner = &after[..close];
        if !inner.trim().is_empty()
            && inner.chars().count() <= MAX_SLASHED
            && !inner.contains('\n')
            && inner.chars().any(is_ipa_letter)
        {
            return short(&format!("/{}/", inner.trim()));
        }
        rest = after;
    }
    None
}

/// The kana before `【` when the Entry opens with its headword written the Japanese way.
fn reading(tokens: &[Token]) -> Option<String> {
    let text = all_text(tokens);
    let text = text.trim_start();
    let (kana, _) = text.split_once('【')?;
    let kana = kana.trim();
    (!kana.is_empty() && kana.chars().all(is_kana)).then(|| kana.to_owned())
}

fn all_text(tokens: &[Token]) -> String {
    tokens
        .iter()
        .filter_map(|token| match token {
            Token::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// `text` with its white space collapsed, if it is short enough to be a pronunciation.
fn short(text: &str) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!collapsed.is_empty() && collapsed.chars().count() <= MAX_CHARS).then_some(collapsed)
}

/// Letters found in IPA transcriptions and almost nowhere else.
fn is_ipa_letter(c: char) -> bool {
    // Stress and length marks, the few IPA letters outside the IPA Extensions block, then
    // that block (ə ɪ ʊ ʃ ʒ ɑ ɔ ʌ …).
    matches!(c, 'ˈ' | 'ˌ' | 'ː' | 'ˑ' | 'θ' | 'ð' | 'ŋ' | 'æ')
        || ('\u{0250}'..='\u{02AF}').contains(&c)
}

fn is_kana(c: char) -> bool {
    // Hiragana, then Katakana (with ・ and ー).
    matches!(c, '\u{3041}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}')
}

#[derive(Debug)]
enum Token {
    /// A start tag that will be closed: void elements and `<x/>` are not tokens.
    Open {
        marked: bool,
    },
    Close,
    Text(String),
}

/// Enough of HTML to find text and elements: tags, comments, and the content of `script`
/// and `style` (skipped). Never fails; odd markup gives odd but bounded results.
fn tokenize(html: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            tokens.push(Token::Text(decode(rest)));
            break;
        };
        if lt > 0 {
            tokens.push(Token::Text(decode(&rest[..lt])));
        }
        rest = &rest[lt..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let end = tag_end(rest);
        let tag = &rest[1..end.saturating_sub(1).max(1)];
        rest = &rest[end..];
        if let Some(name) = tag.strip_prefix('/') {
            if !is_void(&tag_name(name)) {
                tokens.push(Token::Close);
            }
            continue;
        }
        let name = tag_name(tag);
        if name.is_empty() || name.starts_with('!') || name.starts_with('?') {
            continue; // <!DOCTYPE …>, <?xml …?>, or a stray "<"
        }
        if name == "script" || name == "style" {
            let close = format!("</{name}");
            let lower = rest.to_ascii_lowercase();
            rest = match lower.find(&close) {
                Some(at) => &rest[at + tag_end(&rest[at..])..],
                None => "",
            };
            continue;
        }
        if is_void(&name) || tag.trim_end().ends_with('/') {
            continue;
        }
        tokens.push(Token::Open {
            marked: is_marked(tag),
        });
    }
    tokens
}

/// The index just past the `>` that ends the tag at the start of `text`, minding quotes.
fn tag_end(text: &str) -> usize {
    let mut quote = None;
    for (i, c) in text.char_indices().skip(1) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return i + 1,
            (None, _) => {}
        }
    }
    text.len()
}

fn tag_name(tag: &str) -> String {
    tag.trim_start()
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Whether the tag's attributes mark the element as a pronunciation.
fn is_marked(tag: &str) -> bool {
    attributes(tag).any(|(name, value)| match name.as_str() {
        "class" => value
            .split_whitespace()
            .any(|class| MARKS.iter().any(|mark| class.eq_ignore_ascii_case(mark))),
        "data-type" => value.trim() == "発音",
        _ => false,
    })
}

/// The tag's attributes, names lowercased.
fn attributes(tag: &str) -> impl Iterator<Item = (String, String)> + '_ {
    let mut rest = tag.trim_start();
    // Skip the element's name.
    rest = rest.trim_start_matches(|c: char| !c.is_whitespace() && c != '/');
    std::iter::from_fn(move || {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if rest.is_empty() {
            return None;
        }
        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '=' || c == '/')
            .unwrap_or(rest.len());
        let name = rest[..name_end].to_ascii_lowercase();
        rest = rest[name_end..].trim_start();
        let Some(after) = rest.strip_prefix('=') else {
            return Some((name, String::new()));
        };
        let after = after.trim_start();
        let (value, remainder) = match after.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let body = &after[1..];
                let end = body.find(q).unwrap_or(body.len());
                (&body[..end], body.get(end + 1..).unwrap_or(""))
            }
            _ => {
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                (&after[..end], &after[end..])
            }
        };
        rest = remainder;
        Some((name, decode(value)))
    })
}

/// `text` with character references replaced; unknown ones are left as written.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').filter(|&end| end <= 10).and_then(|end| {
            let name = &rest[1..end];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
