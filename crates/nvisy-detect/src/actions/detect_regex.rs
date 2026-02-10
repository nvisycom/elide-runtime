//! Regex-based PII/PHI entity detection action.

use regex::Regex;
use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::ontology::entity::{DetectionMethod, Entity, EntityLocation};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

use crate::patterns::{self, PatternDefinition};

/// Typed parameters for [`DetectRegexAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectRegexParams {
    /// Minimum pattern confidence to emit.
    #[serde(default)]
    pub confidence_threshold: f64,
    /// Subset of built-in pattern names to use. `None` means all.
    #[serde(default)]
    pub patterns: Option<Vec<String>>,
}

/// Scans document text against compiled regex patterns to detect PII/PHI entities.
///
/// For each blob the action reads the `"documents"` artifact (or falls back to
/// the raw blob content), runs every active pattern, optionally validates
/// matches, and appends resulting [`Entity`] artifacts.
pub struct DetectRegexAction;

#[async_trait::async_trait]
impl Action for DetectRegexAction {
    type Params = DetectRegexParams;

    fn id(&self) -> &str {
        "detect-regex"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        let confidence_threshold = params.confidence_threshold;
        let requested_patterns = params.patterns;

        // Resolve patterns
        let active_patterns = resolve_patterns(&requested_patterns);

        // Compile regexes
        let compiled: Vec<(&PatternDefinition, Regex)> = active_patterns
            .iter()
            .filter_map(|p| Regex::new(&p.pattern_str).ok().map(|r| (*p, r)))
            .collect();

        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let documents: Vec<Document> = blob.get_artifacts("documents").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read documents artifact: {e}"))
            })?;

            let docs = if documents.is_empty() {
                // No documents artifact -- treat blob content as plain text
                let text = String::from_utf8_lossy(&blob.content).into_owned();
                vec![Document::new(text)]
            } else {
                documents
            };

            for doc in &docs {
                for (pattern, regex) in &compiled {
                    for mat in regex.find_iter(&doc.content) {
                        let value = mat.as_str();

                        if let Some(validate) = pattern.validate {
                            if !validate(value) {
                                continue;
                            }
                        }

                        if pattern.confidence < confidence_threshold {
                            continue;
                        }

                        let mut entity = Entity::new(
                            pattern.category,
                            &pattern.entity_type,
                            value,
                            DetectionMethod::Regex,
                            pattern.confidence,
                            EntityLocation {
                                start_offset: mat.start(),
                                end_offset: mat.end(),
                                element_id: None,
                                page_number: None,
                                bounding_box: None,
                            },
                        );
                        entity.source_id = Some(doc.data.id);
                        entity.data.parent_id = Some(doc.data.id);

                        blob.add_artifact("entities", &entity).map_err(|e| {
                            Error::new(ErrorKind::Runtime, format!("failed to add entity artifact: {e}"))
                        })?;

                        count += 1;
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

/// Resolves the set of active patterns from an optional list of requested names.
///
/// When `requested` is `None` or empty, all built-in patterns are returned.
fn resolve_patterns(requested: &Option<Vec<String>>) -> Vec<&'static PatternDefinition> {
    match requested {
        Some(names) if !names.is_empty() => names
            .iter()
            .filter_map(|n| patterns::get_pattern(n))
            .collect(),
        _ => patterns::get_all_patterns(),
    }
}
