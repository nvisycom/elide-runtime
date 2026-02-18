//! Cell-level masking and hashing utilities.

use super::TextRedactionOutput;

impl TextRedactionOutput {
    /// Redact a single cell value according to `self`.
    ///
    /// Dispatches on the variant:
    /// - **Mask**: preserve the last 4 characters, replacing the rest with the
    ///   mask character from the output.
    /// - **Remove**: return an empty string.
    /// - **Hash**: return `[HASH:{hex}]` using a deterministic hash.
    /// - **Other variants**: use the output's replacement value directly.
    pub fn mask_cell(&self, cell: &str) -> String {
        match self {
            Self::Mask { mask_char, .. } => {
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
            Self::Remove => String::new(),
            Self::Hash { .. } => {
                format!("[HASH:{:x}]", hash_string(cell))
            }
            _ => self
                .replacement_value()
                .unwrap_or_default()
                .to_string(),
        }
    }
}

/// Compute a deterministic 64-bit hash of `s` using [`DefaultHasher`](std::collections::hash_map::DefaultHasher).
fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
