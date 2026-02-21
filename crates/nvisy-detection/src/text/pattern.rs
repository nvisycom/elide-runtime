//! Regex-based PII/PHI entity detection layer.
//!
//! Operates on [`TxtSpan`] text spans, running compiled regex patterns
//! against each span independently.  Offsets in the resulting
//! [`TextLocation`] are intra-span (relative to the line).

use regex::Regex;
use serde::Deserialize;

use nvisy_codec::handler::{Span, TxtSpan};
use nvisy_core::Error;
use nvisy_core::path::ContentSource;
use nvisy_pattern::patterns::{self, MatchSource, Pattern};
use nvisy_pattern::validators::ValidatorResolver;

use crate::{DetectionMethod, Entity, TextLocation};

use crate::context::ParallelContext;
use crate::layer::{Detect, DetectionLayer};

/// Typed parameters for [`PatternDetection`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternDetectionParams {
    #[serde(default)]
    pub confidence_threshold: f64,
    #[serde(default)]
    pub patterns: Option<Vec<String>>,
}

/// Regex-pattern detection layer.
///
/// Compiles the requested (or all built-in) regex patterns at construction
/// time and matches them against text spans.  Dictionary-sourced patterns
/// are skipped (handled by a separate layer).
pub struct PatternDetection {
    confidence_threshold: f64,
    compiled: Vec<(&'static dyn Pattern, Regex)>,
    validators: ValidatorResolver,
}

#[async_trait::async_trait]
impl DetectionLayer for PatternDetection {
    type Params = PatternDetectionParams;

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        let active = resolve_patterns(&params.patterns);
        let compiled = active
            .into_iter()
            .filter_map(|p| match p.match_source() {
                MatchSource::Regex(re) => Regex::new(re).ok().map(|r| (p, r)),
                MatchSource::Dictionary(_) => None,
            })
            .collect();
        Ok(Self {
            confidence_threshold: params.confidence_threshold,
            compiled,
            validators: ValidatorResolver::builtins(),
        })
    }
}

#[async_trait::async_trait]
impl Detect<TxtSpan, String> for PatternDetection {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<TxtSpan, String>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            for (pattern, regex) in &self.compiled {
                for mat in regex.find_iter(&span.data) {
                    if let Some(validate) = pattern
                        .validator_name()
                        .and_then(|v| self.validators.resolve(v))
                    {
                        if !validate(mat.as_str()) {
                            continue;
                        }
                    }

                    if pattern.confidence() < self.confidence_threshold {
                        continue;
                    }

                    let entity = Entity::new(
                        pattern.category().clone(),
                        pattern.entity_kind().to_string(),
                        mat.as_str(),
                        DetectionMethod::Regex,
                        pattern.confidence(),
                    )
                    .with_text_location(TextLocation {
                        start_offset: mat.start(),
                        end_offset: mat.end(),
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    })
                    .with_parent(source);

                    entities.push(entity);
                }
            }
        }

        Ok(entities)
    }
}

fn resolve_patterns(requested: &Option<Vec<String>>) -> Vec<&'static dyn Pattern> {
    let reg = patterns::builtin_registry();
    match requested {
        Some(names) if !names.is_empty() => names
            .iter()
            .filter_map(|n| reg.get(n))
            .collect(),
        _ => reg.values(),
    }
}
