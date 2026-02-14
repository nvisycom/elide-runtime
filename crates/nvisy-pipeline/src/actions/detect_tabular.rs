//! Column-based rule matching for tabular data.

use regex::Regex;
use serde::Deserialize;

use nvisy_ingest::handler::FormatHandler;
use nvisy_ingest::document::Document;
use nvisy_ontology::entity::{
    DetectionMethod, Entity, EntityCategory, EntityLocation, TabularLocation,
};
use nvisy_core::error::{Error, ErrorKind};

use crate::action::Action;

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
pub struct DetectTabularAction {
    params: DetectTabularParams,
    compiled_rules: Vec<(Regex, ColumnRule)>,
}

#[async_trait::async_trait]
impl Action for DetectTabularAction {
    type Params = DetectTabularParams;
    type Input = Vec<Document<FormatHandler>>;
    type Output = Vec<Entity>;

    fn id(&self) -> &str {
        "detect-tabular"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        let compiled_rules = params
            .column_rules
            .iter()
            .map(|r| {
                let re = Regex::new(&r.column_name_pattern).map_err(|e| {
                    Error::new(
                        ErrorKind::Validation,
                        format!(
                            "invalid column_name_pattern '{}': {e}",
                            r.column_name_pattern
                        ),
                    )
                })?;
                Ok((re, r.clone()))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self {
            params,
            compiled_rules,
        })
    }

    async fn execute(
        &self,
        documents: Self::Input,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for doc in &documents {
            let tabular = match doc.tabular() {
                Some(t) => t,
                None => continue,
            };

            for (col_idx, col_name) in tabular.columns.iter().enumerate() {
                for (regex, rule) in &self.compiled_rules {
                    if !regex.is_match(col_name) {
                        continue;
                    }

                    for (row_idx, row) in tabular.rows.iter().enumerate() {
                        if let Some(cell) = row.get(col_idx) {
                            if cell.is_empty() {
                                continue;
                            }

                            let entity = Entity::new(
                                rule.category.clone(),
                                &rule.entity_type,
                                cell.as_str(),
                                DetectionMethod::Composite,
                                0.9,
                                EntityLocation::Tabular(TabularLocation {
                                    row_index: row_idx,
                                    column_index: col_idx,
                                    start_offset: Some(0),
                                    end_offset: Some(cell.len()),
                                }),
                            )
                            .with_parent(&doc.source);

                            entities.push(entity);
                        }
                    }

                    // Only apply first matching rule per column
                    break;
                }
            }
        }

        Ok(entities)
    }
}
