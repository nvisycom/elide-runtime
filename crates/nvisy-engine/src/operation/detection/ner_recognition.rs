//! NER over the [`nvisy_nlp::NerBackend`] trait.
//!
//! Runs at **phase 2**, after extraction. Iterates the document's
//! text spans and dispatches each through a configured `NerBackend`
//! — typically an ONNX-backed local model today, but the trait is
//! transport-agnostic and any conforming impl works. Span-relative
//! offsets are rebased onto document coordinates before the entities
//! are appended to the envelope.
//!
//! The LLM-driven counterpart lives in
//! [`LlmRecognition`](super::llm_recognition::LlmRecognition); the
//! two run sequentially within the detection phase and their outputs
//! merge downstream.

use std::sync::Arc;

use nvisy_codec::Span;
use nvisy_codec::handler::TextData;
use nvisy_core::{Error, Result};
use nvisy_nlp::NerBackend;
use nvisy_ontology::entity::{Entities, TextLocation};
use nvisy_ontology::primitive::LanguageTag;

use super::rebase_entities::RebaseEntities;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::ner_recognition";

/// NER operation backed by a [`NerBackend`].
///
/// Backends are constructed once (typically at engine init) and
/// shared across runs via `Arc`. The operation itself is cheap to
/// build per-document.
pub struct NerRecognition {
    backend: Arc<dyn NerBackend>,
    language: Option<LanguageTag>,
}

impl NerRecognition {
    /// Construct from a shared backend.
    pub fn new(backend: Arc<dyn NerBackend>) -> Self {
        Self {
            backend,
            language: None,
        }
    }

    /// Attach a language hint forwarded to the backend's
    /// [`NerBackend::recognize`]. Most backends treat it as advisory.
    #[allow(dead_code)] // wired when language plumbing reaches the orchestrator
    pub fn with_language(mut self, language: LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    async fn detect(&self, spans: &[Span<TextLocation, TextData>]) -> Result<Entities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "running NER backend");
        let mut entities = Entities::new();

        for span in spans {
            let detected: Entities = self
                .backend
                .recognize(span.data.as_str(), self.language.as_ref())
                .await
                .map_err(|e| Error::from(e).with_component("ner-backend"))?;

            entities.extend(detected.rebase_offsets(span));
        }

        Ok(entities)
    }
}

impl Operation for NerRecognition {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = envelope.document.collect_text_spans().await;
        if !spans.is_empty() {
            let detected = self.detect(&spans).await?;
            tracing::debug!(
                target: TARGET,
                detected = detected.len(),
                "appending NER backend entities",
            );
            envelope.add_entities(detected).await;
        }
        Ok(())
    }
}
