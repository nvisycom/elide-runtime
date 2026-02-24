//! Tabular (CSV) document redaction.

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::handler::CsvHandler;
use nvisy_codec::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::location::Location;
use nvisy_ontology::record::Redaction;
use nvisy_ontology::spec::{RedactionInput, TextRedactionInput};
use nvisy_core::Error;

pub(crate) async fn apply_tabular_doc(
    doc: &Document<CsvHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<CsvHandler>, Error> {
    let mut result = doc.clone();

    for (&entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(&entity_id) {
            Some(e) => e,
            None => continue,
        };

        let tab_loc = match &entity.location {
            Some(Location::Tabular(loc)) => loc,
            _ => continue,
        };

        if !matches!(redaction.spec, RedactionInput::Text(_)) {
            continue;
        }

        let (row_idx, col_idx) = (tab_loc.row_index, tab_loc.column_index);
        if let Some(row) = result.handler_mut().rows_mut().get_mut(row_idx)
            && let Some(cell) = row.get_mut(col_idx)
        {
            *cell = mask_cell(&redaction.spec, &redaction.replacement, cell);
        }
    }

    Ok(result)
}

/// Redact a single cell value according to the spec and replacement.
fn mask_cell(spec: &RedactionInput, replacement: &str, cell: &str) -> String {
    match spec {
        RedactionInput::Text(TextRedactionInput::Mask { mask_char }) => {
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
        RedactionInput::Text(TextRedactionInput::Remove) => String::new(),
        RedactionInput::Text(TextRedactionInput::Hash) => {
            format!("[HASH:{:x}]", hash_string(cell))
        }
        _ => replacement.to_string(),
    }
}

/// Compute a deterministic 64-bit hash of `s`.
fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_cell_mask_long() {
        let spec = RedactionInput::Text(TextRedactionInput::Mask { mask_char: '*' });
        assert_eq!(mask_cell(&spec, "", "1234567890"), "******7890");
    }

    #[test]
    fn mask_cell_mask_short() {
        let spec = RedactionInput::Text(TextRedactionInput::Mask { mask_char: '#' });
        assert_eq!(mask_cell(&spec, "", "abcd"), "####");
    }

    #[test]
    fn mask_cell_mask_exact_four() {
        let spec = RedactionInput::Text(TextRedactionInput::Mask { mask_char: 'X' });
        assert_eq!(mask_cell(&spec, "", "1234"), "XXXX");
    }

    #[test]
    fn mask_cell_remove() {
        let spec = RedactionInput::Text(TextRedactionInput::Remove);
        assert_eq!(mask_cell(&spec, "", "sensitive"), "");
    }

    #[test]
    fn mask_cell_hash() {
        let spec = RedactionInput::Text(TextRedactionInput::Hash);
        let result = mask_cell(&spec, "", "hello");
        assert!(result.starts_with("[HASH:"));
        assert!(result.ends_with(']'));
    }

    #[test]
    fn mask_cell_replace_fallback() {
        let spec = RedactionInput::Text(TextRedactionInput::Replace {
            placeholder: String::new(),
        });
        assert_eq!(mask_cell(&spec, "[REDACTED]", "sensitive"), "[REDACTED]");
    }

    #[test]
    fn hash_string_deterministic() {
        assert_eq!(hash_string("hello"), hash_string("hello"));
        assert_ne!(hash_string("hello"), hash_string("world"));
    }
}
