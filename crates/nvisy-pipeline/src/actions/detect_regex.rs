//! Regex-based PII/PHI entity detection action.

use regex::Regex;
use serde::Deserialize;

use nvisy_ingest::handler::FormatHandler;
use nvisy_ingest::document::Document;
use nvisy_ontology::ontology::entity::{DetectionMethod, Entity, EntityLocation};
use nvisy_core::error::Error;
use nvisy_pattern::patterns::{self, PatternDefinition};

use crate::action::Action;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectRegexParams {
    #[serde(default)]
    pub confidence_threshold: f64,
    #[serde(default)]
    pub patterns: Option<Vec<String>>,
}

pub struct DetectRegexAction {
    params: DetectRegexParams,
}

#[async_trait::async_trait]
impl Action for DetectRegexAction {
    type Params = DetectRegexParams;
    type Input = Vec<Document<FormatHandler>>;
    type Output = Vec<Entity>;

    fn id(&self) -> &str {
        "detect-regex"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        documents: Self::Input,
    ) -> Result<Vec<Entity>, Error> {
        let confidence_threshold = self.params.confidence_threshold;
        let requested_patterns = &self.params.patterns;

        let active_patterns = resolve_patterns(requested_patterns);

        let compiled: Vec<(&PatternDefinition, Regex)> = active_patterns
            .iter()
            .filter_map(|p| Regex::new(&p.pattern_str).ok().map(|r| (*p, r)))
            .collect();

        let mut entities = Vec::new();

        for doc in &documents {
            let content = match &doc.content {
                Some(c) => c,
                None => continue,
            };

            for (pattern, regex) in &compiled {
                for mat in regex.find_iter(content) {
                    let value = mat.as_str();

                    if let Some(validate) = pattern.validate {
                        if !validate(value) {
                            continue;
                        }
                    }

                    if pattern.confidence < confidence_threshold {
                        continue;
                    }

                    let entity = Entity::new(
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
                            row_index: None,
                            column_index: None,
                            image_id: None,
                        },
                    )
                    .with_parent(&doc.source);

                    entities.push(entity);
                }
            }
        }

        Ok(entities)
    }
}

fn resolve_patterns(requested: &Option<Vec<String>>) -> Vec<&'static PatternDefinition> {
    match requested {
        Some(names) if !names.is_empty() => names
            .iter()
            .filter_map(|n| patterns::get_pattern(n))
            .collect(),
        _ => patterns::get_all_patterns(),
    }
}
