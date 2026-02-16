//! Cell-level masking and hashing utilities.
//!
//! These functions are used by tabular redaction actions to transform
//! individual cell values according to a [`TextRedactionOutput`] variant.

use crate::render::output::TextRedactionOutput;

/// Redact a single cell value according to `output`.
///
/// Dispatches on the [`TextRedactionOutput`] variant:
/// - **Mask**: preserve the last 4 characters, replacing the rest with the
///   mask character from the output.
/// - **Remove**: return an empty string.
/// - **Hash**: return `[HASH:{hex}]` using [`hash_string`].
/// - **Other variants**: use the output's replacement value directly.
pub fn mask_cell(cell: &str, output: &TextRedactionOutput) -> String {
    match output {
        TextRedactionOutput::Mask { mask_char, .. } => {
            let char_count = cell.chars().count();
            if char_count > 4 {
                let masked: String = cell
                    .chars()
                    .take(char_count - 4)
                    .map(|_| *mask_char)
                    .collect();
                let tail: String = cell.chars().skip(char_count - 4).collect();
                format!("{masked}{tail}")
            } else {
                mask_char.to_string().repeat(char_count)
            }
        }
        TextRedactionOutput::Remove => String::new(),
        TextRedactionOutput::Hash { .. } => {
            format!("[HASH:{:x}]", hash_string(cell))
        }
        _ => output
            .replacement_value()
            .unwrap_or_default()
            .to_string(),
    }
}

/// Compute a deterministic 64-bit hash of `s` using [`DefaultHasher`](std::collections::hash_map::DefaultHasher).
pub fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
