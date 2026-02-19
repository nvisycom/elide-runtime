//! Column-based rule matching for tabular data.
//!
//! Two-phase detection: scan header spans for column-name matches,
//! then emit entities from every non-empty data cell in matched
//! columns.

use std::collections::HashMap;

use regex::Regex;
use serde::Deserialize;

use nvisy_codec::handler::{CsvSpan, Span};
use nvisy_core::data::EntityCategory;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::path::ContentSource;

use crate::ontology::{DetectionMethod, Entity, TabularLocation};

use crate::detection::context::ParallelContext;
use crate::detection::layer::{Detect, DetectionLayer};

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

/// Typed parameters for [`TabularDetection`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabularDetectionParams {
    /// Column-matching rules.
    pub column_rules: Vec<ColumnRule>,
}

/// Matches column headers against rules and marks every non-empty cell
/// in matched columns as an entity.
pub struct TabularDetection {
    compiled_rules: Vec<(Regex, ColumnRule)>,
}

#[async_trait::async_trait]
impl DetectionLayer for TabularDetection {
    type Params = TabularDetectionParams;

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
        Ok(Self { compiled_rules })
    }
}

#[async_trait::async_trait]
impl Detect<CsvSpan, String> for TabularDetection {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<CsvSpan, String>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error> {
        // Phase 1: identify matched columns from header spans.
        let mut matched_columns: HashMap<usize, &ColumnRule> = HashMap::new();

        for span in &spans {
            if !span.id.header {
                continue;
            }
            // Only apply first matching rule per column.
            if matched_columns.contains_key(&span.id.col) {
                continue;
            }
            for (regex, rule) in &self.compiled_rules {
                if regex.is_match(&span.data) {
                    matched_columns.insert(span.id.col, rule);
                    break;
                }
            }
        }

        if matched_columns.is_empty() {
            return Ok(Vec::new());
        }

        // Phase 2: emit entities from data cells in matched columns.
        let mut entities = Vec::new();

        for span in &spans {
            if span.id.header || span.data.is_empty() {
                continue;
            }

            if let Some(rule) = matched_columns.get(&span.id.col) {
                let entity = Entity::new(
                    rule.category.clone(),
                    &rule.entity_type,
                    span.data.as_str(),
                    DetectionMethod::Composite,
                    0.9,
                )
                .with_tabular_location(TabularLocation {
                    row_index: span.id.row,
                    column_index: span.id.col,
                    start_offset: Some(0),
                    end_offset: Some(span.data.len()),
                })
                .with_parent(source);

                entities.push(entity);
            }
        }

        Ok(entities)
    }
}
