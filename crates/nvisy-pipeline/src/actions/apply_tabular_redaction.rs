//! Tabular data redaction action -- applies redaction to specific cells.

use serde::Deserialize;

use nvisy_ingest::handler::FormatHandler;
use nvisy_ingest::document::Document;
use nvisy_ontology::ontology::entity::Entity;
use nvisy_ontology::ontology::redaction::{Redaction, RedactionMethod};
use nvisy_core::error::Error;

use crate::action::Action;

/// Typed parameters for [`ApplyTabularRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTabularRedactionParams {}

/// Applies pending redactions to tabular data cells.
///
/// For entities with `row_index` and `column_index`, the corresponding cell
/// value is redacted according to the redaction method (mask, replace,
/// remove, hash).
pub struct ApplyTabularRedactionAction {
    params: ApplyTabularRedactionParams,
}

#[async_trait::async_trait]
impl Action for ApplyTabularRedactionAction {
    type Params = ApplyTabularRedactionParams;
    type Input = (Vec<Document<FormatHandler>>, Vec<Entity>, Vec<Redaction>);
    type Output = Vec<Document<FormatHandler>>;

    fn id(&self) -> &str {
        "apply-tabular-redaction"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let (mut documents, entities, redactions) = input;

        // Build entity->redaction map
        let redaction_map: std::collections::HashMap<uuid::Uuid, &Redaction> = redactions
            .iter()
            .filter(|r| !r.applied)
            .map(|r| (r.entity_id, r))
            .collect();

        for entity in &entities {
            if let (Some(row_idx), Some(col_idx)) =
                (entity.location.row_index, entity.location.column_index)
            {
                if let Some(redaction) = redaction_map.get(&entity.source.as_uuid()) {
                    for doc in &mut documents {
                        if let Some(rows) = &mut doc.rows {
                            if let Some(row) = rows.get_mut(row_idx) {
                                if let Some(cell) = row.get_mut(col_idx) {
                                    *cell = apply_cell_redaction(
                                        cell,
                                        redaction.method,
                                        &redaction.replacement_value,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(documents)
    }
}

fn apply_cell_redaction(cell: &str, method: RedactionMethod, replacement: &str) -> String {
    match method {
        RedactionMethod::Mask => {
            // Mask all but last 4 characters
            if cell.len() > 4 {
                format!(
                    "{}{}",
                    "*".repeat(cell.len() - 4),
                    &cell[cell.len() - 4..]
                )
            } else {
                "*".repeat(cell.len())
            }
        }
        RedactionMethod::Replace => replacement.to_string(),
        RedactionMethod::Remove => String::new(),
        RedactionMethod::Hash => {
            format!("[HASH:{:x}]", hash_string(cell))
        }
        _ => replacement.to_string(),
    }
}

fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
