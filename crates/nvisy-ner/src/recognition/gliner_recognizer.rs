//! [`GlinerRecognizer`]: zero-shot NER recognizer that bypasses
//! shared NLP artifacts and calls a [`GlinerBackend`] directly.
//!
//! Implements [`EntityRecognizer<Text>`] uniformly with every other text
//! recognizer in the platform. Pulls the requested
//! [`EntityKind`] list from
//! [`Context::candidate_languages`] — wait,
//! correction: from the per-call list the engine layer threads in
//! (see the engine-side context fields). Today the call shape is:
//! `Context<TextData>` carries the kind allowlist via
//! `ctx.entity_kinds` once the engine layer adds it. Until that
//! field is added we fall back to the recognizer's full supported
//! kind list.
//!
//! The recognizer is named — its name is what the
//! [`ContextEnhancer`]
//! looks up in its registry to find the recognizer's
//! [`default_context`].
//!
//! [`Context::candidate_languages`]: nvisy_core::Context
//! [`ContextEnhancer`]: nvisy_core::context::ContextEnhancer
//! [`default_context`]: super::NerModelConfiguration::default_context

use std::sync::Arc;

use async_trait::async_trait;
use nvisy_core::nlp::RawNerSpan;
use nvisy_core::{Context as CoreContext, EntityRecognizer, Result, TextData};
use nvisy_ontology::entity::{Entity, EntityKind, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_ontology::modality::Text;
use nvisy_ontology::primitive::Confidence;

use super::config::NerModelConfiguration;
use crate::backend::{GlinerBackend, GlinerRequest};

/// Zero-shot NER recognizer.
///
/// Construct via [`GlinerRecognizer::new`] with a name, the
/// backend, the supported kinds, and a configuration. Implements
/// [`EntityRecognizer<Text>`] so it composes with the rest of the
/// pipeline through the same trait every other text recognizer
/// uses.
pub struct GlinerRecognizer {
    name: String,
    backend: Arc<dyn GlinerBackend>,
    supported_kinds: Vec<EntityKind>,
    config: NerModelConfiguration,
}

impl GlinerRecognizer {
    /// Construct a recognizer.
    ///
    /// `name` is what gets stamped on the entity's recognition
    /// step provenance and is the key the
    /// [`ContextEnhancer`]
    /// uses to look up the recognizer's
    /// [`default_context`].
    ///
    /// `supported_kinds` is the recognizer's advertised kind
    /// allowlist. When the per-call context restricts to a subset,
    /// the recognizer asks the backend for only that subset;
    /// otherwise it asks for everything in `supported_kinds`.
    ///
    /// [`ContextEnhancer`]: nvisy_core::context::ContextEnhancer
    /// [`default_context`]: super::NerModelConfiguration::default_context
    pub fn new<B: GlinerBackend>(
        name: impl Into<String>,
        backend: B,
        supported_kinds: Vec<EntityKind>,
        config: NerModelConfiguration,
    ) -> Self {
        Self {
            name: name.into(),
            backend: Arc::new(backend),
            supported_kinds,
            config,
        }
    }

    /// Recognizer name. Surfaced in trail provenance and used as
    /// the context-registry key.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Kinds this recognizer advertises.
    #[must_use]
    pub fn supported_kinds(&self) -> &[EntityKind] {
        &self.supported_kinds
    }

    /// Borrow the configuration.
    #[must_use]
    pub fn config(&self) -> &NerModelConfiguration {
        &self.config
    }

    fn build_entity(&self, span: &RawNerSpan, kind: EntityKind) -> Entity<Text> {
        let raw_confidence =
            Confidence::try_clamped(span.score).unwrap_or(self.config.default_score);
        let confidence = if self.config.low_score_kinds.contains(&kind) {
            let demoted = raw_confidence.get() * self.config.low_score_multiplier;
            Confidence::try_clamped(demoted).unwrap_or(self.config.default_score)
        } else {
            raw_confidence
        };
        let provenance = TrailProvenance::Model(ModelProvenance::new(self.name.clone()));
        let reason = format!("recognizer `{}` identified {kind}", self.name);
        let step = TrailStep::recognition("ner", confidence, provenance, reason);
        Entity::builder()
            .with_entity_kind(kind)
            .with_trail(vec![step])
            .with_confidence(confidence)
            .with_location(Text::new(span.offset.start, span.offset.end))
            .build()
            .expect("required fields provided")
    }
}

#[async_trait]
impl EntityRecognizer<Text> for GlinerRecognizer {
    async fn recognize(&self, ctx: &CoreContext<TextData>) -> Result<Vec<Entity<Text>>> {
        // Zero-shot needs requested kinds; if the recognizer has
        // nothing to look for, short-circuit.
        if self.supported_kinds.is_empty() {
            return Ok(Vec::new());
        }

        let request = GlinerRequest {
            text: ctx.data.text.as_str(),
            kinds: &self.supported_kinds,
            language: ctx.language.as_ref(),
            correlation_id: ctx.correlation_id,
        };
        let spans = self.backend.predict(request).await?;

        let entities: Vec<Entity<Text>> = spans
            .iter()
            .filter(|s| !self.config.labels_to_ignore.contains(s.label.as_str()))
            .filter_map(|s| {
                self.config
                    .label_map
                    .lookup(&s.label)
                    .filter(|k| self.supported_kinds.contains(k))
                    .map(|k| self.build_entity(s, k))
            })
            .collect();
        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NoopBackend;

    #[tokio::test]
    async fn noop_yields_no_entities() {
        let rec = GlinerRecognizer::new(
            "test",
            NoopBackend,
            vec![EntityKind::PersonName, EntityKind::EmailAddress],
            NerModelConfiguration::default(),
        );
        let ctx = CoreContext::new(TextData::new("Alice Smith"));
        let out = rec.recognize(&ctx).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn empty_supported_kinds_short_circuits() {
        let rec = GlinerRecognizer::new(
            "test",
            NoopBackend,
            Vec::new(),
            NerModelConfiguration::default(),
        );
        let ctx = CoreContext::new(TextData::new("Alice Smith"));
        let out = rec.recognize(&ctx).await.unwrap();
        assert!(out.is_empty());
    }
}
