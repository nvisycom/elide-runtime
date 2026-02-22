//! Pattern-based PII/PHI entity detection layer.
//!
//! Operates on [`TxtSpan`] text spans and [`CsvSpan`] tabular spans,
//! running both compiled regex patterns and dictionary automata via
//! [`PatternEngine`].

use serde::Deserialize;

use nvisy_codec::handler::{CsvSpan, Span, TxtSpan};
use nvisy_core::Error;
use nvisy_core::path::ContentSource;
use nvisy_pattern::{PatternEngine, PatternEngineBuilder, DetectionSource};

use crate::{DetectionMethod, Entity, TabularLocation, TextLocation};
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
        let mut entities = Vec::new();

        for span in &spans {
            for m in self.engine.scan_text(&span.data) {
                let method = match m.source {
                    DetectionSource::Regex => DetectionMethod::Regex,
                    DetectionSource::Dictionary => DetectionMethod::Dictionary,
                };

                let entity = Entity::new(
                    m.category,
                    m.entity_kind,
                    &m.value,
                    method,
                    m.confidence,
                )
                .with_text_location(TextLocation {
                    start_offset: m.start,
                    end_offset: m.end,
                    element_id: Some(span.id.0.to_string()),
                    ..Default::default()
                })
                .with_parent(source);

                entities.push(entity);
            }
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
        let mut entities = Vec::new();

        for span in &spans {
            if span.id.header || span.data.is_empty() {
                continue;
            }

            for m in self.engine.scan_text(&span.data) {
                let method = match m.source {
                    DetectionSource::Regex => DetectionMethod::Regex,
                    DetectionSource::Dictionary => DetectionMethod::Dictionary,
                };

                let entity = Entity::new(
                    m.category,
                    m.entity_kind,
                    &m.value,
                    method,
                    m.confidence,
                )
                .with_tabular_location(TabularLocation {
                    row_index: span.id.row,
                    column_index: span.id.col,
                    start_offset: Some(m.start),
                    end_offset: Some(m.end),
                })
                .with_parent(source);

                entities.push(entity);
            }
        }

        Ok(entities)
    }
}
