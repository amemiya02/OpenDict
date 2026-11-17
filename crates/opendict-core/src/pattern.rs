//! Wildcard patterns: `*` stands for any run of characters, `?` for exactly one.

/// A query with wildcards in it, matched against index keys.
pub(crate) struct Pattern {
    chars: Vec<char>,
    /// The characters before the first wildcard. Every match starts with them.
    prefix: String,
}

impl Pattern {
    /// The pattern in `key`, an index key, if it has a wildcard in it.
    pub(crate) fn parse(key: &str) -> Option<Pattern> {
        let first = key.find(['*', '?'])?;
        Some(Pattern {
            chars: key.chars().collect(),
            prefix: key[..first].to_owned(),
        })
    }

    pub(crate) fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Whether the whole of `key` matches the pattern.
    pub(crate) fn matches(&self, key: &str) -> bool {
        let key: Vec<char> = key.chars().collect();
        let (mut p, mut k) = (0, 0);
        // Where the last `*` was, and how much of the key it has taken so far.
        let mut star: Option<(usize, usize)> = None;
        while k < key.len() {
            match self.chars.get(p) {
                Some('*') => {
                    star = Some((p, k));
                    p += 1;
                }
                Some(&c) if c == '?' || c == key[k] => {
                    p += 1;
                    k += 1;
                }
                _ => match star {
                    // Let the last `*` take one more character and try again.
                    Some((star_p, star_k)) => {
                        star = Some((star_p, star_k + 1));
                        p = star_p + 1;
                        k = star_k + 1;
                    }
                    None => return false,
                },
            }
        }
        self.chars[p..].iter().all(|&c| c == '*')
    }
}
