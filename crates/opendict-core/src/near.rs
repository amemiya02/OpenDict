//! Near matches: headwords spelt nearly like the query, for when nothing matches it.

/// How far a key may be from the query's key to count as a near match: one slip in a
/// short word, two in a longer one.
pub(crate) fn tolerance(query: &[char]) -> usize {
    if query.len() <= 4 { 1 } else { 2 }
}

/// The number of single-character insertions, deletions, substitutions and swaps of
/// adjacent characters that turn `a` into `b`, if it is at most `max`.
pub(crate) fn distance(a: &[char], b: &[char], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    // Three rows of the edit-distance table: two rows back (for swaps), the last, this.
    let mut before: Vec<usize> = Vec::new();
    let mut last: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut row = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let substitution = last[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            row[j] = substitution.min(last[j] + 1).min(row[j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                row[j] = row[j].min(before[j - 2] + 1);
            }
        }
        if row.iter().all(|&cost| cost > max) {
            return None;
        }
        before = std::mem::replace(&mut last, row);
    }
    Some(last[b.len()]).filter(|&cost| cost <= max)
}
