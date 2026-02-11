//! Aho-Corasick dictionary-based entity detection action.

use aho_corasick::AhoCorasick;
use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::{Document, TabularData};
use nvisy_ontology::ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityLocation};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

use crate::dictionaries;

/// Definition of a single dictionary for matching.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryDef {
    /// Dictionary name — `"builtin:first_names"` for built-in, or a custom name.
    pub name: String,
    /// Entity category for matches from this dictionary.
    pub category: EntityCategory,
    /// Entity type label for matches (e.g. `"first_name"`, `"medical_term"`).
    pub entity_type: String,
    /// Custom values — empty when using a builtin dictionary.
    #[serde(default)]
    pub values: Vec<String>,
    /// Whether matching should be case-sensitive.
    #[serde(default)]
    pub case_sensitive: bool,
}

/// Typed parameters for [`DetectDictionaryAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectDictionaryParams {
    /// One or more dictionaries to match against.
    pub dictionaries: Vec<DictionaryDef>,
    /// Confidence score assigned to dictionary matches.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.85
}

/// Scans document text and tabular cells against Aho-Corasick automata
/// built from user-provided word lists and/or built-in gazetteers.
pub struct DetectDictionaryAction;

#[async_trait::async_trait]
impl Action for DetectDictionaryAction {
    type Params = DetectDictionaryParams;

    fn id(&self) -> &str {
        "detect-dictionary"
    }

    fn validate_params(&self, params: &Self::Params) -> Result<(), Error> {
        if params.dictionaries.is_empty() {
            return Err(Error::new(
                ErrorKind::Validation,
                "at least one dictionary definition is required",
            ));
        }
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        // Build automata for each dictionary
        let automata = build_automata(&params.dictionaries)?;
        let confidence = params.confidence;
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            // Scan documents
            let documents: Vec<Document> = blob.get_artifacts("documents").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read documents artifact: {e}"))
            })?;

            for doc in &documents {
                for (def, ac, values) in &automata {
                    for mat in ac.find_iter(&doc.content) {
                        let value = &values[mat.pattern().as_usize()];
                        let entity = Entity::new(
                            def.category,
                            &def.entity_type,
                            value.as_str(),
                            DetectionMethod::Dictionary,
                            confidence,
                            EntityLocation {
                                start_offset: mat.start(),
                                end_offset: mat.end(),
                                element_id: None,
                                page_number: None,
                                bounding_box: None,
                                row_index: None,
                                column_index: None,
                                image_id: None,
                            },
                        )
                        .with_source_id(doc.data.id);
                        blob.add_artifact("entities", &entity).map_err(|e| {
                            Error::new(ErrorKind::Runtime, format!("failed to add entity: {e}"))
                        })?;
                        count += 1;
                    }
                }
            }

            // Scan tabular data
            let tables: Vec<TabularData> = blob.get_artifacts("tabular").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read tabular artifact: {e}"))
            })?;

            for table in &tables {
                for (row_idx, row) in table.rows.iter().enumerate() {
                    for (col_idx, cell) in row.iter().enumerate() {
                        if cell.is_empty() {
                            continue;
                        }
                        for (def, ac, values) in &automata {
                            for mat in ac.find_iter(cell) {
                                let value = &values[mat.pattern().as_usize()];
                                let entity = Entity::new(
                                    def.category,
                                    &def.entity_type,
                                    value.as_str(),
                                    DetectionMethod::Dictionary,
                                    confidence,
                                    EntityLocation {
                                        start_offset: mat.start(),
                                        end_offset: mat.end(),
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

/// Resolve dictionary values (builtin or custom) and build Aho-Corasick automata.
fn build_automata(
    defs: &[DictionaryDef],
) -> Result<Vec<(&DictionaryDef, AhoCorasick, Vec<String>)>, Error> {
    let mut result = Vec::with_capacity(defs.len());

    for def in defs {
        let values: Vec<String> = if def.name.starts_with("builtin:") {
            let builtin = dictionaries::get_builtin(&def.name).ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    format!("unknown builtin dictionary: {}", def.name),
                )
            })?;
            builtin.to_vec()
        } else {
            def.values.clone()
        };

        if values.is_empty() {
            continue;
        }

        let ac = aho_corasick::AhoCorasickBuilder::new()
            .ascii_case_insensitive(!def.case_sensitive)
            .build(&values)
            .map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to build automaton: {e}"))
            })?;

        result.push((def, ac, values));
    }

    Ok(result)
}
