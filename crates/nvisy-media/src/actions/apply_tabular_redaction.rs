//! Tabular data redaction action — applies redaction to specific cells.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::TabularData;
use nvisy_ontology::ontology::entity::Entity;
use nvisy_ontology::ontology::redaction::{Redaction, RedactionMethod};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

/// Typed parameters for [`ApplyTabularRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTabularRedactionParams {}

/// Applies pending redactions to tabular data cells.
///
/// For entities with `row_index` and `column_index`, the corresponding cell
/// value is redacted according to the redaction method (mask, replace,
/// remove, hash).
pub struct ApplyTabularRedactionAction;

#[async_trait::async_trait]
impl Action for ApplyTabularRedactionAction {
    type Params = ApplyTabularRedactionParams;

    fn id(&self) -> &str {
        "apply-tabular-redaction"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        _params: Self::Params,
    ) -> Result<u64, Error> {
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let mut tables: Vec<TabularData> = blob.get_artifacts("tabular").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read tabular: {e}"))
            })?;
            let entities: Vec<Entity> = blob.get_artifacts("entities").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read entities: {e}"))
            })?;
            let redactions: Vec<Redaction> = blob.get_artifacts("redactions").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read redactions: {e}"))
            })?;

            // Build entity->redaction map
            let redaction_map: std::collections::HashMap<uuid::Uuid, &Redaction> = redactions
                .iter()
                .filter(|r| !r.applied)
                .map(|r| (r.entity_id, r))
                .collect();

            let mut modified = false;

            for entity in &entities {
                if let (Some(row_idx), Some(col_idx)) =
                    (entity.location.row_index, entity.location.column_index)
                {
                    if let Some(redaction) = redaction_map.get(&entity.data.id) {
                        // Apply to all matching tables
                        for table in &mut tables {
                            if let Some(row) = table.rows.get_mut(row_idx) {
                                if let Some(cell) = row.get_mut(col_idx) {
                                    *cell = apply_cell_redaction(
                                        cell,
                                        redaction.method,
                                        &redaction.replacement_value,
                                    );
                                    modified = true;
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            }

            if modified {
                // Replace tabular artifact
                blob.artifacts.remove("tabular");
                for table in &tables {
                    blob.add_artifact("tabular", table).map_err(|e| {
                        Error::new(ErrorKind::Runtime, format!("failed to add tabular: {e}"))
                    })?;
                }

                // Mark redactions as applied
                let mut updated_redactions: Vec<Redaction> = redactions.clone();
                for r in &mut updated_redactions {
                    if redaction_map.contains_key(&r.entity_id) {
                        r.applied = true;
                    }
                }
                blob.artifacts.remove("redactions");
                for r in &updated_redactions {
                    blob.add_artifact("redactions", r).map_err(|e| {
                        Error::new(ErrorKind::Runtime, format!("failed to add redaction: {e}"))
                    })?;
                }
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}

fn apply_cell_redaction(
    cell: &str,
    method: RedactionMethod,
    replacement: &str,
) -> String {
    match method {
        RedactionMethod::Mask => {
            // Mask all but last 4 characters
            if cell.len() > 4 {
                format!("{}{}", "*".repeat(cell.len() - 4), &cell[cell.len() - 4..])
            } else {
                "*".repeat(cell.len())
            }
        }
        RedactionMethod::Replace => replacement.to_string(),
        RedactionMethod::Remove => String::new(),
        RedactionMethod::Hash => {
            // Simple hash representation
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
