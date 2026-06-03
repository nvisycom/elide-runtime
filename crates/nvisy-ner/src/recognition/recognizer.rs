//! [`NerRecognizer`]: unified NER recognizer that drives any
//! [`NerBackend`] backend.
//!
//! Holds an `Arc<dyn NerBackend>` plus a [`NerModel`] (label-map +
//! ignore set + low-score demotion knobs) plus the recognizer's
//! advertised [`supported_kinds`]. On each `recognize` call it asks
//! the backend for spans (passing `Some(&supported_kinds)` when
//! non-empty for zero-shot backends, `None` when empty for
//! fixed-label backends), then normalizes through the model and
//! emits entities.
//!
//! Implements [`EntityRecognizer<Text>`] so it composes with the
//! rest of the platform through the same trait every other text
//! recognizer uses.
//!
//! [`supported_kinds`]: NerRecognizer::supported_kinds

use std::sync::Arc;

use async_trait::async_trait;
use derive_builder::Builder;
use nvisy_core::entity::{Entity, EntityKind, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_core::modality::{Text, TextLocation};
use nvisy_core::primitive::Confidence;
use nvisy_core::{EntityRecognizer, Error, RecognizerInput, RecognizerOutput, Result};

use super::config::NerModel;
use crate::backend::{NerBackend, NerRequest, RawNerSpan};

/// Trait-driven NER recognizer.
#[derive(Clone, Builder)]
#[builder(
    name = "NerRecognizerBuilder",
    pattern = "owned",
    setter(into, prefix = "with"),
    build_fn(error = "Error", name = "try_build", private)
)]
pub struct NerRecognizer {
    /// Recognizer name. Surfaced in trail provenance and used as
    /// the key the [`ContextEnhancer`] looks up to find the
    /// recognizer's [`default_context`].
    ///
    /// [`ContextEnhancer`]: nvisy_core::context::ContextEnhancer
    /// [`default_context`]: NerModel::default_context
    name: String,
    /// Backend that turns `(text, kinds)` into raw spans. Required.
    /// Set via [`with_engine`], which accepts any concrete
    /// [`NerBackend`] impl by value and wraps it in `Arc` internally.
    ///
    /// [`with_engine`]: NerRecognizerBuilder::with_engine
    #[builder(setter(custom))]
    engine: Arc<dyn NerBackend>,
    /// Kinds the recognizer advertises. When non-empty, the
    /// recognizer asks the backend for only this subset on every
    /// call (zero-shot path). When empty, the backend is asked for
    /// whatever it natively produces (fixed-label path).
    #[builder(default)]
    supported_kinds: Vec<EntityKind>,
    /// Normalization knobs applied to the backend's raw output
    /// before entities are emitted.
    #[builder(default)]
    model: NerModel,
}

impl NerRecognizer {
    /// Start the chainable builder. `name` and `engine` are
    /// required — calling [`build`] without them returns a
    /// validation error.
    ///
    /// [`build`]: NerRecognizerBuilder::build
    #[must_use]
    pub fn builder() -> NerRecognizerBuilder {
        NerRecognizerBuilder::default()
    }

    /// Recognizer name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Kinds this recognizer advertises.
    #[must_use]
    pub fn supported_kinds(&self) -> &[EntityKind] {
        &self.supported_kinds
    }

    /// Borrow the normalization config.
    #[must_use]
    pub fn model(&self) -> &NerModel {
        &self.model
    }

    fn build_entity(&self, span: &RawNerSpan, kind: EntityKind) -> Entity<Text> {
        let raw_confidence =
            Confidence::try_clamped(span.score).unwrap_or(self.model.default_score);
        let confidence = if self.model.low_score_kinds.contains(&kind) {
            let demoted = raw_confidence.get() * self.model.low_score_multiplier;
            Confidence::try_clamped(demoted).unwrap_or(self.model.default_score)
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
            .with_location(TextLocation::new(span.offset.start, span.offset.end))
            .build()
            .expect("required fields provided")
    }
}

impl NerRecognizerBuilder {
    /// Set the [`NerBackend`] backend that powers this recognizer.
    /// Accepts any concrete impl by value and wraps it in `Arc`.
    /// Required — `build` errors when this hasn't been called.
    #[must_use]
    pub fn with_engine<E: NerBackend>(mut self, engine: E) -> Self {
        self.engine = Some(Arc::new(engine));
        self
    }

    /// Finish the builder. Errors when `name` or `engine` is unset.
    pub fn build(self) -> Result<NerRecognizer> {
        self.try_build()
    }
}

#[async_trait]
impl EntityRecognizer<Text> for NerRecognizer {
    async fn recognize(&self, input: &RecognizerInput<Text>) -> Result<RecognizerOutput<Text>> {
        let kinds = if self.supported_kinds.is_empty() {
            None
        } else {
            Some(self.supported_kinds.as_slice())
        };
        let request = NerRequest {
            text: input.data.text.as_str(),
            kinds,
            language: input.language.as_ref(),
            correlation_id: input.correlation_id,
        };
        let response = self.engine.predict(request).await?;

        let entities: Vec<Entity<Text>> = response
            .spans
            .iter()
            .filter(|s| !self.model.labels_to_ignore.contains(s.label.as_str()))
            .filter_map(|s| {
                self.model
                    .label_map
                    .lookup(&s.label)
                    .filter(|k| self.supported_kinds.is_empty() || self.supported_kinds.contains(k))
                    .map(|k| self.build_entity(s, k))
            })
            .collect();
        Ok(RecognizerOutput::new(entities))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::TextData;

    use super::*;
    use crate::backend::NoopBackend;

    #[tokio::test]
    async fn noop_engine_yields_no_entities() {
        let rec = NerRecognizer::builder()
            .with_name("test")
            .with_engine(NoopBackend)
            .with_supported_kinds(vec![EntityKind::PersonName, EntityKind::EmailAddress])
            .build()
            .expect("builder succeeds");
        let input = RecognizerInput::new(TextData::new("Alice Smith"));
        let out = rec.recognize(&input).await.unwrap();
        assert!(out.entities.is_empty());
    }

    #[tokio::test]
    async fn empty_supported_kinds_passes_none_to_engine() {
        let rec = NerRecognizer::builder()
            .with_name("test")
            .with_engine(NoopBackend)
            .build()
            .expect("builder succeeds");
        let input = RecognizerInput::new(TextData::new("Alice Smith"));
        let out = rec.recognize(&input).await.unwrap();
        assert!(out.entities.is_empty());
    }
}
