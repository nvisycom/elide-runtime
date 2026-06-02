//! [`NlpEngine`]: the producer-side trait that builds
//! [`NlpArtifacts`](nvisy_core::nlp::NlpArtifacts) for one or more
//! texts.
//!
//! Pluggable so different deployment shapes (pure language
//! detection, hosted full-NLP service, future in-process model) can
//! be wired interchangeably. The orchestrator calls `process` (or
//! `process_batch`) once per scan, wraps the result in an `Arc`,
//! and hands it to every text [`Recognizer`](nvisy_core::Recognizer)
//! plus the
//! [`ContextEnhancer`](nvisy_core::context::ContextEnhancer).

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::nlp::{NlpArtifacts, NlpCapabilities};
use nvisy_ontology::primitive::LanguageTag;

/// Builds [`NlpArtifacts`] for the orchestrator's shared-NLP pass.
///
/// Engines advertise their capabilities via
/// [`capabilities`](Self::capabilities) so the orchestrator can
/// refuse impossible compositions at construction time (e.g.
/// wiring a lemma-aware enhancer to an engine that doesn't produce
/// lemmas). Engines also advertise the languages they support so a
/// future per-language registry can route correctly.
///
/// `Send + Sync + 'static` — engines live behind `Arc<dyn _>` in
/// the orchestrator and are shared across recognition tasks.
#[async_trait]
pub trait NlpEngine: Send + Sync + 'static {
    /// Languages this engine can produce artifacts for. Empty when
    /// the engine is language-agnostic (e.g. a tokenizer that
    /// works on bytes alone).
    fn supported_languages(&self) -> &[LanguageTag];

    /// What the engine can produce. Advisory — consumers can still
    /// call `process` even when capabilities are off, they just
    /// get the default-empty fields back.
    fn capabilities(&self) -> NlpCapabilities;

    /// Process one text. Returns the artifact bundle.
    ///
    /// `hint` is the caller-asserted language; engines that can
    /// skip detection when given a hint should do so.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when the underlying detection or
    /// inference call fails. Empty input is not an error — engines
    /// should produce an empty artifact (or whatever's defensible
    /// per capability).
    async fn process(&self, text: &str, hint: Option<&LanguageTag>) -> Result<NlpArtifacts>;

    /// Process a batch of texts. The default fans out via
    /// [`process`](Self::process) concurrently; engines with
    /// native batching (`capabilities().batch_native == true`)
    /// should override.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered.
    async fn process_batch(
        &self,
        texts: &[&str],
        hint: Option<&LanguageTag>,
    ) -> Result<Vec<NlpArtifacts>> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.process(text, hint).await?);
        }
        Ok(out)
    }
}
