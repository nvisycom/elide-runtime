//! Pattern-based PII/PHI entity detection operation.
//!
//! Operates on text, CSV, HTML, and JSON spans, running both compiled
//! regex patterns and dictionary automata via [`PatternEngine`].

use nvisy_codec::Span;
use nvisy_codec::handler::{CsvSpan, HtmlSpan, JsonPath, TxtSpan};
use nvisy_core::Error;
use nvisy_ontology::entity::{DetectionMethod, Entity, TabularLocation, TextLocation};
use nvisy_pattern::{
    ContextRule, DetectionSource, PatternEngine, PatternEngineBuilder,
    PatternMatch as PatternMatchResult,
};
use serde::Deserialize;
use serde_json::Value;

use crate::operation::{Operation, ParallelContext};

/// Typed parameters for [`PatternMatch`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternDetectionParams {
    #[serde(default)]
    pub confidence_threshold: f64,
    #[serde(default)]
    pub patterns: Option<Vec<String>>,
}

/// Multi-modality input for pattern matching.
pub enum PatternInput {
    Text(Vec<Span<TxtSpan, String>>),
    Csv(Vec<Span<CsvSpan, String>>),

    Html(Vec<Span<HtmlSpan, String>>),
    Json(Vec<Span<JsonPath, Value>>),
}

/// Pattern detection operation backed by [`PatternEngine`].
///
/// Handles both regex and dictionary matches, replacing the former
/// separate `DictionaryDetection`.
pub struct PatternMatch {
    engine: PatternEngine,
}

impl PatternMatch {
    /// Connect and build a pattern match operation from typed parameters.
    pub async fn connect(params: PatternDetectionParams) -> Result<Self, Error> {
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
}

impl Operation for PatternMatch {
    type Input = ParallelContext<PatternInput>;
    type Output = ParallelContext<Vec<Entity>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, Error> {
        input
            .parallel_map(|data| async move {
                match data {
                    PatternInput::Text(spans) => self.detect_text(spans),
                    PatternInput::Csv(spans) => self.detect_csv(spans),
                    PatternInput::Html(spans) => self.detect_html(spans),
                    PatternInput::Json(spans) => self.detect_json(spans),
                }
            })
            .await
    }
}

impl PatternMatch {
    fn detect_text(&self, spans: Vec<Span<TxtSpan, String>>) -> Result<Vec<Entity>, Error> {
        // Phase 1: collect raw matches per span index.
        let span_data: Vec<&str> = spans.iter().map(|s| s.data.as_str()).collect();
        let mut raw_matches: Vec<(usize, PatternMatchResult)> = Vec::new();

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
            .with_location(
                TextLocation {
                    start_offset: m.start,
                    end_offset: m.end,
                    element_id: Some(spans[*span_idx].id.0.to_string()),
                    ..Default::default()
                }
                .into(),
            )
            .with_parent(&spans[*span_idx].source);

            entities.push(entity);
        }

        Ok(entities)
    }

    fn detect_csv(&self, spans: Vec<Span<CsvSpan, String>>) -> Result<Vec<Entity>, Error> {
        // Collect all span data (including headers) for co-occurrence window.
        let span_data: Vec<&str> = spans.iter().map(|s| s.data.as_str()).collect();

        // Phase 1: collect raw matches per span index (skip headers).
        let mut raw_matches: Vec<(usize, PatternMatchResult)> = Vec::new();
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
            .with_location(
                TabularLocation {
                    row_index: span.id.row,
                    column_index: span.id.col,
                    start_offset: Some(m.start),
                    end_offset: Some(m.end),
                    column_name: None,
                    sheet_name: None,
                }
                .into(),
            )
            .with_parent(&span.source);

            entities.push(entity);
        }

        Ok(entities)
    }

    fn detect_html(&self, spans: Vec<Span<HtmlSpan, String>>) -> Result<Vec<Entity>, Error> {
        let span_data: Vec<&str> = spans.iter().map(|s| s.data.as_str()).collect();
        let mut raw_matches: Vec<(usize, PatternMatchResult)> = Vec::new();

        for (idx, span) in spans.iter().enumerate() {
            for m in self.engine.scan_text(&span.data) {
                raw_matches.push((idx, m));
            }
        }

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
            .with_location(
                TextLocation {
                    start_offset: m.start,
                    end_offset: m.end,
                    element_id: Some(spans[*span_idx].id.0.to_string()),
                    ..Default::default()
                }
                .into(),
            )
            .with_parent(&spans[*span_idx].source);

            entities.push(entity);
        }

        Ok(entities)
    }

    fn detect_json(&self, spans: Vec<Span<JsonPath, Value>>) -> Result<Vec<Entity>, Error> {
        // Filter to string-valued spans and collect text for co-occurrence.
        let string_spans: Vec<(usize, &str)> = spans
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| s.data.as_str().map(|text| (idx, text)))
            .collect();

        let span_data: Vec<&str> = string_spans.iter().map(|(_, text)| *text).collect();
        let mut raw_matches: Vec<(usize, PatternMatchResult)> = Vec::new();

        for (co_idx, (_, text)) in string_spans.iter().enumerate() {
            for m in self.engine.scan_text(text) {
                raw_matches.push((co_idx, m));
            }
        }

        let mut entities = Vec::new();
        for (co_idx, m) in &raw_matches {
            let confidence = if let Some(ref ctx) = m.context {
                apply_cooccurrence(&span_data, *co_idx, ctx, m.confidence)
            } else {
                m.confidence
            };

            let method = detection_method(m.source);
            let (orig_idx, _) = string_spans[*co_idx];

            let entity = Entity::new(
                m.category.clone(),
                m.entity_kind,
                &m.value,
                method,
                confidence,
            )
            .with_location(
                TextLocation {
                    start_offset: m.start,
                    end_offset: m.end,
                    element_id: Some(spans[orig_idx].id.pointer.clone()),
                    ..Default::default()
                }
                .into(),
            )
            .with_parent(&spans[orig_idx].source);

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
