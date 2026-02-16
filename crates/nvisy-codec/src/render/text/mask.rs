//! Cell-level masking and hashing utilities.
//!
//! These functions are used by tabular redaction actions to transform
//! individual cell values according to a [`RedactionOutput`] variant.

use nvisy_ontology::redaction::{RedactionOutput, TextRedactionOutput};

/// Redact a single cell value according to `output`.
///
/// Dispatches on the [`RedactionOutput`] variant:
/// - **Mask**: preserve the last 4 characters, replacing the rest with the
///   mask character from the output.
/// - **Remove**: return an empty string.
/// - **Hash**: return `[HASH:{hex}]` using [`hash_string`].
/// - **Other text variants**: use [`replacement_value()`](RedactionOutput::replacement_value),
///   falling back to repeating `default_mask` for the cell length.
pub fn mask_cell(cell: &str, output: &RedactionOutput, default_mask: char) -> String {
    match output {
        RedactionOutput::Text(TextRedactionOutput::Mask { mask_char, .. }) => {
            if cell.len() > 4 {
                format!(
                    "{}{}",
                    mask_char.to_string().repeat(cell.len() - 4),
                    &cell[cell.len() - 4..]
                )
            } else {
                mask_char.to_string().repeat(cell.len())
            }
        }
        RedactionOutput::Text(TextRedactionOutput::Remove) => String::new(),
        RedactionOutput::Text(TextRedactionOutput::Hash { .. }) => {
            format!("[HASH:{:x}]", hash_string(cell))
        }
        _ => output
            .replacement_value()
            .map(|v| v.to_string())
            .unwrap_or_else(|| default_mask.to_string().repeat(cell.len())),
    }
}

/// Compute a deterministic 64-bit hash of `s` using [`DefaultHasher`](std::collections::hash_map::DefaultHasher).
pub fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
