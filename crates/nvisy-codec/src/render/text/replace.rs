//! Byte-offset text replacement engine.
//!
//! Provides a simple but correct algorithm for applying multiple
//! non-overlapping replacements to a string by processing them
//! right-to-left (descending start offset). This ensures that each
//! substitution does not invalidate the byte offsets of earlier
//! (leftward) replacements.

/// A single text replacement that has been resolved but not yet applied.
pub struct PendingReplacement {
    /// Byte offset where the replacement starts in the original text.
    pub start: usize,
    /// Byte offset where the replacement ends (exclusive) in the original text.
    pub end: usize,
    /// The string that will replace the original span.
    pub value: String,
}

/// Apply a set of pending replacements to `text`, returning the result.
///
/// Replacements are applied right-to-left (descending start offset) so that
/// earlier byte offsets remain valid after each substitution. Out-of-range
/// offsets are clamped to the text length and empty spans are skipped.
pub fn apply_replacements(text: &str, pending: &mut [PendingReplacement]) -> String {
    // Sort by start offset descending (right-to-left) to preserve positions
    pending.sort_by(|a, b| b.start.cmp(&a.start));

    let mut result = text.to_string();
    for replacement in pending.iter() {
        let start = replacement.start.min(result.len());
        let end = replacement.end.min(result.len());
        if start >= end {
            continue;
        }

        result = format!(
            "{}{}{}",
            &result[..start],
            replacement.value,
            &result[end..]
        );
    }
    result
}
