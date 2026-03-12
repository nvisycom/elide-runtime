//! Pattern-based PII/PHI entity detection operation.
//!
//! Scans type-erased text spans (`Span<usize, TextData>`) using compiled
//! regex patterns and dictionary automata via [`PatternEngine`].

use nvisy_codec::Span;
use nvisy_codec::handler::TextData;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::TextLocation;
use nvisy_pattern::patterns::ContextRule;
use nvisy_pattern::{PatternEngine, PatternEngineBuilder, RawMatch, ScanContext};
use serde::Deserialize;

use crate::operation::envelope::DetectedEntities;
use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::pattern_match";

/// Typed parameters for [`PatternMatch`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternDetectionParams {
    #[serde(default)]
    pub confidence_threshold: f64,
    #[serde(default)]
    pub patterns: Option<Vec<String>>,
}

/// Pattern detection operation backed by [`PatternEngine`].
///
/// Accepts type-erased text spans from any [`TextHandler`] (plain text,
/// CSV, HTML, JSON, etc.) and detects entities using regex and dictionary
/// patterns with co-occurrence boosting.
///
/// [`TextHandler`]: nvisy_codec::handler::TextHandler
pub struct PatternMatch {
    engine: PatternEngine,
}

impl PatternMatch {
    /// Connect and build a pattern match operation from typed parameters.
    pub async fn connect(params: PatternDetectionParams) -> Result<Self> {
        let mut builder =
            PatternEngineBuilder::default().with_confidence_threshold(params.confidence_threshold);
        if let Some(ref names) = params.patterns {
            builder = builder.with_patterns(names);
        }
        let engine = builder
            .build()
            .map_err(|e| Error::validation(e.to_string(), "pattern-detection"))?;
        Ok(Self { engine })
    }

    fn detect(&self, spans: Vec<Span<usize, TextData>>) -> Result<DetectedEntities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "scanning for patterns");

        let span_data: Vec<&str> = spans.iter().map(|s| s.data.as_str()).collect();
        let mut raw_matches: Vec<(usize, RawMatch)> = Vec::new();
        let scan_ctx = ScanContext::default();

        for (idx, span) in spans.iter().enumerate() {
            for m in self.engine.scan_text(span.data.as_str(), &scan_ctx) {
                raw_matches.push((idx, m));
            }
        }

        let mut entities = Vec::new();
        for (span_idx, m) in raw_matches {
            let confidence = if let Some(ref ctx) = m.context {
                apply_cooccurrence(&span_data, span_idx, ctx, m.confidence)
            } else {
                m.confidence
            };
            let start = m.start;
            let end = m.end;
            let element_id = spans[span_idx].id.to_string();
            let source = spans[span_idx].source;

            let mut entity = m.into_entity();
            entity.confidence = confidence;
            let entity = entity
                .with_location(
                    TextLocation {
                        start_offset: start,
                        end_offset: end,
                        element_id: Some(element_id),
                        ..Default::default()
                    }
                    .into(),
                )
                .with_parent(&source);

            entities.push(entity);
        }

        Ok(DetectedEntities(entities.into()))
    }
}

impl Operation for PatternMatch {
    type Input = ParallelContext<Vec<Span<usize, TextData>>>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| async { self.detect(data) }).await
    }
}

/// Apply co-occurrence scoring: boost confidence when context keywords
/// appear in nearby spans within the sliding window.
fn apply_cooccurrence(spans: &[&str], span_idx: usize, rule: &ContextRule, base: f64) -> f64 {
    let start = span_idx.saturating_sub(rule.window);
    let end = (span_idx + rule.window + 1).min(spans.len());

    for span in &spans[start..end] {
        let found = if rule.case_sensitive {
            rule.keywords.iter().any(|kw| span.contains(kw.as_str()))
        } else {
            let lower = span.to_lowercase();
            rule.keywords
                .iter()
                .any(|kw| lower.contains(&kw.to_lowercase()))
        };
        if found {
            return (base + rule.boost).clamp(0.0, 1.0);
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context_rule(keywords: Vec<&str>, window: usize, boost: f64) -> ContextRule {
        ContextRule {
            keywords: keywords.into_iter().map(String::from).collect(),
            window,
            boost,
            case_sensitive: false,
        }
    }

    #[test]
    fn cooccurrence_boost_when_keyword_in_window() {
        let spans = vec![
            "Employee Information",
            "Social Security Number",
            "123-45-6789",
            "Date of Hire",
        ];
        let rule = make_context_rule(vec!["social security"], 3, 0.1);
        let result = apply_cooccurrence(&spans, 2, &rule, 0.9);
        assert!(
            (result - 1.0).abs() < f64::EPSILON,
            "expected 1.0 (0.9 + 0.1), got {result}"
        );
    }

    #[test]
    fn cooccurrence_no_boost_without_keyword() {
        let spans = vec![
            "some random text",
            "another line",
            "123-45-6789",
            "more data",
        ];
        let rule = make_context_rule(vec!["social security", "ssn"], 3, 0.1);
        let result = apply_cooccurrence(&spans, 2, &rule, 0.9);
        assert!(
            (result - 0.9).abs() < f64::EPSILON,
            "expected 0.9 (no boost), got {result}"
        );
    }

    #[test]
    fn cooccurrence_window_boundary_excludes_far_keywords() {
        // Window of 1: only spans at indices 1 and 3 are in range for span 2.
        let spans = vec![
            "Social Security info", // index 0 — outside window=1
            "header row",           // index 1
            "123-45-6789",          // index 2 — match span
            "footer row",           // index 3
            "SSN listed here",      // index 4 — outside window=1
        ];
        let rule = make_context_rule(vec!["social security", "ssn"], 1, 0.1);
        let result = apply_cooccurrence(&spans, 2, &rule, 0.9);
        assert!(
            (result - 0.9).abs() < f64::EPSILON,
            "keywords outside window should not boost; got {result}"
        );
    }

    #[test]
    fn cooccurrence_boost_clamped_to_one() {
        let spans = vec!["SSN", "123-45-6789"];
        let rule = make_context_rule(vec!["ssn"], 3, 0.5);
        let result = apply_cooccurrence(&spans, 1, &rule, 0.9);
        assert!(
            (result - 1.0).abs() < f64::EPSILON,
            "boosted confidence should be clamped to 1.0, got {result}"
        );
    }

    #[test]
    fn cooccurrence_case_insensitive() {
        let spans = vec!["SOCIAL SECURITY", "123-45-6789"];
        let rule = make_context_rule(vec!["social security"], 3, 0.1);
        let result = apply_cooccurrence(&spans, 1, &rule, 0.9);
        assert!(
            (result - 1.0).abs() < f64::EPSILON,
            "case-insensitive keyword should match; got {result}"
        );
    }
}
