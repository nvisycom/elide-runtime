//! Cell-level masking and hashing utilities.
//!
//! These functions are used by tabular redaction actions to transform
//! individual cell values according to a [`TextRedactionOutput`] variant.

use nvisy_ontology::redaction::TextRedactionOutput;

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
        TextRedactionOutput::Remove => String::new(),
        TextRedactionOutput::Hash { .. } => {
            format!("[HASH:{:x}]", hash_string(cell))
        }
        TextRedactionOutput::Replace { replacement }
        | TextRedactionOutput::Synthesize { replacement }
        | TextRedactionOutput::Aggregate { replacement }
        | TextRedactionOutput::Generalize { replacement, .. }
        | TextRedactionOutput::DateShift { replacement, .. } => replacement.clone(),
        TextRedactionOutput::Encrypt { ciphertext, .. } => ciphertext.clone(),
        TextRedactionOutput::Pseudonymize { pseudonym } => pseudonym.clone(),
        TextRedactionOutput::Tokenize { token, .. } => token.clone(),
    }
}

/// Compute a deterministic 64-bit hash of `s` using [`DefaultHasher`](std::collections::hash_map::DefaultHasher).
pub fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
