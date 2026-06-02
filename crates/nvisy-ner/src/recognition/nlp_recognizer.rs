//! [`NlpRecognizer`]: the dumb-adapter recognizer.
//!
//! Doesn't run a model itself — reads
//! [`NlpArtifacts::ner`](nvisy_core::nlp::NlpArtifacts::ner)
//! produced by an upstream
//! [`NlpEngine`](crate::nlp::NlpEngine), normalizes the raw labels
//! through its [`LabelMap`](super::LabelMap), drops ignored labels,
//! demotes low-score kinds, filters down to the recognizer's
//! supported-kind list, and emits entities. Mirrors the
//! "adapter recognizer" role Presidio's `SpacyRecognizer` plays.
//!
//! Requires the [`Context<TextData>`](nvisy_core::Context) to
//! carry [`artifacts`](nvisy_core::TextData::artifacts) — the
//! orchestrator is expected to run an `NlpEngine` once per scan
//! and stamp the result on the context before fanning out to
//! recognizers.

use async_trait::async_trait;
use nvisy_core::nlp::RawNerSpan;
use nvisy_core::{Context as CoreContext, Error, Recognizer, Result, TextData};
use nvisy_ontology::entity::{Entity, EntityKind, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_ontology::modality::Text;
use nvisy_ontology::primitive::Confidence;

use super::config::NerModelConfiguration;

/// Adapter recognizer that reads shared NLP artifacts.
///
/// Owns no model state — the upstream
/// [`NlpEngine`](crate::nlp::NlpEngine) does the work and stamps
/// the result on the context. This recognizer's job is
/// normalization: label → kind, label-ignore, low-score
/// demotion, supported-kind filtering.
pub struct NlpRecognizer {
    name: String,
    supported_kinds: Vec<EntityKind>,
    config: NerModelConfiguration,
}

impl NlpRecognizer {
    /// Construct a recognizer.
    ///
    /// `name` is what gets stamped on the entity's recognition
    /// step provenance and is the key the
    /// [`ContextEnhancer`](nvisy_core::context::ContextEnhancer)
    /// uses to look up the recognizer's
    /// [`default_context`](super::NerModelConfiguration::default_context).
    pub fn new(
        name: impl Into<String>,
        supported_kinds: Vec<EntityKind>,
        config: NerModelConfiguration,
    ) -> Self {
        Self {
            name: name.into(),
            supported_kinds,
            config,
        }
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
impl Recognizer<Text> for NlpRecognizer {
    async fn recognize(&self, ctx: &CoreContext<TextData>) -> Result<Vec<Entity<Text>>> {
        let artifacts = ctx.data.artifacts.as_ref().ok_or_else(|| {
            Error::validation(
                "NlpRecognizer requires NlpArtifacts on TextData (run an NlpEngine first)",
                "nvisy-ner",
            )
        })?;

        let entities: Vec<Entity<Text>> = artifacts
            .ner
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
    use std::sync::Arc;

    use nvisy_core::nlp::NlpArtifacts;

    use super::*;

    fn artifacts_with_one_span(label: &str, score: f64) -> Arc<NlpArtifacts> {
        let mut artifacts = NlpArtifacts::default();
        artifacts.ner.push(RawNerSpan::new(label, score, 0..5));
        Arc::new(artifacts)
    }

    #[tokio::test]
    async fn emits_entities_for_supported_kinds() {
        let rec = NlpRecognizer::new(
            "ner",
            vec![EntityKind::PersonName],
            NerModelConfiguration::default(),
        );
        let artifacts = artifacts_with_one_span("person_name", 0.9);
        let ctx = CoreContext::new(TextData::new("Alice").with_artifacts(artifacts));
        let out = rec.recognize(&ctx).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].entity_kind, EntityKind::PersonName);
    }

    #[tokio::test]
    async fn drops_unsupported_kinds() {
        let rec = NlpRecognizer::new(
            "ner",
            vec![EntityKind::PersonName],
            NerModelConfiguration::default(),
        );
        let artifacts = artifacts_with_one_span("address", 0.9);
        let ctx = CoreContext::new(TextData::new("123 Main St").with_artifacts(artifacts));
        let out = rec.recognize(&ctx).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn ignored_labels_drop_out() {
        let config = NerModelConfiguration::default().with_labels_to_ignore(["misc".to_owned()]);
        let rec = NlpRecognizer::new("ner", vec![EntityKind::PersonName], config);
        let mut artifacts = NlpArtifacts::default();
        artifacts.ner.push(RawNerSpan::new("misc", 0.9, 0..5));
        let ctx = CoreContext::new(TextData::new("anything").with_artifacts(Arc::new(artifacts)));
        assert!(rec.recognize(&ctx).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn low_score_kinds_get_demoted() {
        let config = NerModelConfiguration::default()
            .with_low_score_kinds([EntityKind::PersonName])
            .with_low_score_multiplier(0.5);
        let rec = NlpRecognizer::new("ner", vec![EntityKind::PersonName], config);
        let artifacts = artifacts_with_one_span("person_name", 0.9);
        let ctx = CoreContext::new(TextData::new("Alice").with_artifacts(artifacts));
        let out = rec.recognize(&ctx).await.unwrap();
        assert_eq!(out.len(), 1);
        // 0.9 * 0.5 = 0.45
        assert!((out[0].confidence.get() - 0.45).abs() < 0.01);
    }

    #[tokio::test]
    async fn errors_without_artifacts() {
        let rec = NlpRecognizer::new(
            "ner",
            vec![EntityKind::PersonName],
            NerModelConfiguration::default(),
        );
        let ctx = CoreContext::new(TextData::new("Alice"));
        match rec.recognize(&ctx).await {
            Ok(_) => panic!("expected error when artifacts missing"),
            Err(e) => assert!(e.to_string().contains("NlpArtifacts")),
        }
    }
}
