//! Column-based rule matching for tabular data.

use regex::Regex;
use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::TabularData;
use nvisy_ontology::ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityLocation};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

/// A rule that matches column headers to classify entire columns.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnRule {
    /// Regex pattern to match against column names.
    pub column_name_pattern: String,
    /// Entity category for matches in the column.
    pub category: EntityCategory,
    /// Entity type label for matches.
    pub entity_type: String,
}

/// Typed parameters for [`DetectTabularAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectTabularParams {
    /// Column-matching rules.
    pub column_rules: Vec<ColumnRule>,
}

/// Matches column headers against rules and marks every non-empty cell
/// in matched columns as an entity.
pub struct DetectTabularAction;

#[async_trait::async_trait]
impl Action for DetectTabularAction {
    type Params = DetectTabularParams;

    fn id(&self) -> &str {
        "detect-tabular"
    }

    fn validate_params(&self, params: &Self::Params) -> Result<(), Error> {
        for rule in &params.column_rules {
            Regex::new(&rule.column_name_pattern).map_err(|e| {
                Error::new(
                    ErrorKind::Validation,
                    format!("invalid column_name_pattern '{}': {e}", rule.column_name_pattern),
                )
            })?;
        }
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        // Compile column-name regexes
        let compiled_rules: Vec<(Regex, &ColumnRule)> = params
            .column_rules
            .iter()
            .filter_map(|r| Regex::new(&r.column_name_pattern).ok().map(|re| (re, r)))
            .collect();

        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let tables: Vec<TabularData> = blob.get_artifacts("tabular").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read tabular artifact: {e}"))
            })?;

            for table in &tables {
                // For each column, check if any rule matches the column name
                for (col_idx, col_name) in table.columns.iter().enumerate() {
                    for (regex, rule) in &compiled_rules {
                        if !regex.is_match(col_name) {
                            continue;
                        }

                        // Mark every non-empty cell in this column
                        for (row_idx, row) in table.rows.iter().enumerate() {
                            if let Some(cell) = row.get(col_idx) {
                                if cell.is_empty() {
                                    continue;
                                }

                                let entity = Entity::new(
                                    rule.category,
                                    &rule.entity_type,
                                    cell.as_str(),
                                    DetectionMethod::Composite,
                                    0.9,
                                    EntityLocation {
                                        start_offset: 0,
                                        end_offset: cell.len(),
                                        element_id: None,
                                        page_number: None,
                                        bounding_box: None,
                                        row_index: Some(row_idx),
                                        column_index: Some(col_idx),
                                        image_id: None,
                                    },
                                )
                                .with_source_id(table.data.id);

                                blob.add_artifact("entities", &entity).map_err(|e| {
                                    Error::new(
                                        ErrorKind::Runtime,
                                        format!("failed to add entity: {e}"),
                                    )
                                })?;
                                count += 1;
                            }
                        }

                        // Only apply first matching rule per column
                        break;
                    }
                }
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}
