//! Pattern-based PII/PHI entity detection layer.
//!
//! Operates on [`TxtSpan`] text spans and [`CsvSpan`] tabular spans,
//! running both compiled regex patterns and dictionary automata via
//! [`PatternEngine`].

use serde::Deserialize;

use nvisy_codec::handler::{CsvSpan, Span, TxtSpan};
use nvisy_core::Error;
use nvisy_core::path::ContentSource;
use nvisy_pattern::{ContextRule, PatternEngine, PatternEngineBuilder, DetectionSource, PatternMatch};

use crate::{DetectionMethod, Entity, Location, TabularLocation, TextLocation};
use crate::{ParallelContext, Detect, DetectionLayer};

/// Typed parameters for [`PatternDetection`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternDetectionParams {
    #[serde(default)]
    pub confidence_threshold: f64,
    #[serde(default)]
    pub patterns: Option<Vec<String>>,
}

/// Pattern detection layer backed by [`PatternEngine`].
///
/// Handles both regex and dictionary matches in a single layer,
/// replacing the former separate `DictionaryDetection`.
pub struct PatternDetection {
    engine: PatternEngine,
}

#[async_trait::async_trait]
impl DetectionLayer for PatternDetection {
    type Params = PatternDetectionParams;

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        let mut builder = PatternEngineBuilder::default()
            .confidence_threshold(params.confidence_threshold);
        if let Some(ref names) = params.patterns {
            builder = builder.patterns(names);
        }
        let engine = builder.build().map_err(|e| {
            Error::validation(e.to_string(), "pattern-detection")
        })?;
        Ok(Self { engine })
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
        // Phase 1: collect raw matches per span index.
        let span_data: Vec<&str> = spans.iter().map(|s| s.data.as_str()).collect();
        let mut raw_matches: Vec<(usize, PatternMatch)> = Vec::new();

        for (idx, span) in spans.iter().enumerate() {
            for m in self.engine.scan_text(&span.data) {
                raw_matches.push((idx, m));
            }
        }

        // Phase 2: apply co-occurrence boost and build entities.
        let mut entities = Vec::new();
        for (span_idx, m) in &raw_matches {
            let confidence = if let Some(ref ctx) = m.context {
                apply_cooccurrence(&span_data, *span_idx, ctx, m.confidence)
            } else {
                m.confidence
            };

            let method = detection_method(m.source);

            let entity = Entity::new(
                m.category.clone(),
                m.entity_kind,
                &m.value,
                method,
                confidence,
            )
            .with_location(Location::Text(TextLocation {
                start_offset: m.start,
                end_offset: m.end,
                element_id: Some(spans[*span_idx].id.0.to_string()),
                ..Default::default()
            }))
            .with_parent(source);

            entities.push(entity);
        }

        Ok(entities)
    }
}

#[async_trait::async_trait]
impl Detect<CsvSpan, String> for PatternDetection {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<CsvSpan, String>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error> {
        // Collect all span data (including headers) for co-occurrence window.
        let span_data: Vec<&str> = spans.iter().map(|s| s.data.as_str()).collect();

        // Phase 1: collect raw matches per span index (skip headers).
        let mut raw_matches: Vec<(usize, PatternMatch)> = Vec::new();
        for (idx, span) in spans.iter().enumerate() {
            if span.id.header || span.data.is_empty() {
                continue;
            }
            for m in self.engine.scan_text(&span.data) {
                raw_matches.push((idx, m));
            }
        }

        // Phase 2: apply co-occurrence boost and build entities.
        let mut entities = Vec::new();
        for (span_idx, m) in &raw_matches {
            let confidence = if let Some(ref ctx) = m.context {
                apply_cooccurrence(&span_data, *span_idx, ctx, m.confidence)
            } else {
                m.confidence
            };

            let method = detection_method(m.source);
            let span = &spans[*span_idx];

            let entity = Entity::new(
                m.category.clone(),
                m.entity_kind,
                &m.value,
                method,
                confidence,
            )
            .with_location(Location::Tabular(TabularLocation {
                row_index: span.id.row,
                column_index: span.id.col,
                start_offset: Some(m.start),
                end_offset: Some(m.end),
            }))
            .with_parent(source);

            entities.push(entity);
        }

        Ok(entities)
    }
}

/// Map a [`DetectionSource`] to a [`DetectionMethod`].
fn detection_method(source: DetectionSource) -> DetectionMethod {
    match source {
        DetectionSource::Regex => DetectionMethod::Regex,
        DetectionSource::Dictionary => DetectionMethod::Dictionary,
        DetectionSource::DenyList => DetectionMethod::Dictionary,
    }
}

/// Apply co-occurrence scoring: boost confidence when context keywords
/// appear in nearby spans within the sliding window.
fn apply_cooccurrence(
    spans: &[&str],
    span_idx: usize,
    rule: &ContextRule,
    base: f64,
) -> f64 {
    let start = span_idx.saturating_sub(rule.window);
    let end = (span_idx + rule.window + 1).min(spans.len());

    for span in &spans[start..end] {
        let lower = span.to_lowercase();
        if rule.keywords.iter().any(|kw| lower.contains(&kw.to_lowercase())) {
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
            "Social Security info",  // index 0 — outside window=1
            "header row",            // index 1
            "123-45-6789",           // index 2 — match span
            "footer row",            // index 3
            "SSN listed here",       // index 4 — outside window=1
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
