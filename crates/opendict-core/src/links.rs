//! Reading the links out of Entry HTML without rendering it.
//!
//! Entry HTML is untrusted and often malformed, so this is a forgiving scan for `href`
//! attributes, not an HTML parser. It only reads; nothing it returns is ever put back
//! into a document.

/// The values of the `href` attributes in `html`, in order, with the common character
/// references decoded.
pub(crate) fn href_values(html: &str) -> impl Iterator<Item = String> + '_ {
    let mut rest = html;
    std::iter::from_fn(move || {
        loop {
            let at = find_ignore_case(rest, "href")?;
            // Only an attribute name counts: it follows whitespace (or a quote or slash
            // in sloppy markup), never a letter as in `data-href` or `ahref`.
            let before = rest[..at].chars().next_back();
            let after = &rest[at + "href".len()..];
            rest = after;
            if !before.is_some_and(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '/')) {
                continue;
            }
            let Some(value) = after.trim_start().strip_prefix('=') else {
                continue;
            };
            let value = value.trim_start();
            let (raw, remainder) = match value.chars().next() {
                Some(quote @ ('"' | '\'')) => {
                    let inner = &value[1..];
                    match inner.find(quote) {
                        Some(end) => (&inner[..end], &inner[end + 1..]),
                        None => (inner, ""),
                    }
                }
                _ => {
                    let end = value
                        .find(|c: char| c.is_whitespace() || c == '>')
                        .unwrap_or(value.len());
                    (&value[..end], &value[end..])
                }
            };
            rest = remainder;
            return Some(decode_references(raw.trim()));
        }
    })
}

fn find_ignore_case(text: &str, needle: &str) -> Option<usize> {
    text.as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Decodes the character references that turn up in attribute values.
fn decode_references(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        rest = &rest[at..];
        let reference = rest.find(';').map(|end| (&rest[1..end], end + 1));
        let character = reference.and_then(|(name, _)| match name {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            _ => {
                let number = name.strip_prefix('#')?;
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => number.parse().ok()?,
                };
                char::from_u32(code)
            }
        });
        match (character, reference) {
            (Some(character), Some((_, length))) => {
                decoded.push(character);
                rest = &rest[length..];
            }
            _ => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quoted_unquoted_and_referenced_values_but_not_lookalikes() {
        let html =
            r#"<a HREF = "one">x</a><a data-href="no"><a href='two&#39;s'><a href=three&amp;four>"#;
        let values: Vec<_> = href_values(html).collect();
        assert_eq!(values, ["one", "two's", "three&four"]);
    }

    #[test]
    fn survives_unterminated_markup() {
        let values: Vec<_> = href_values(r#"<a href="sound://a.mp3"#).collect();
        assert_eq!(values, ["sound://a.mp3"]);
        assert_eq!(href_values("<a href").count(), 0);
        assert_eq!(
            decode_references("a & b &bogus; &#xZZ;"),
            "a & b &bogus; &#xZZ;"
        );
    }
}
