//! [`Recognizer`] trait + [`RecognizerKind`] enum — the
//! per-recognizer abstraction the [`DetectionEngine`] dispatches
//! against, and the enum used by workflow nodes and config sections
//! to refer to recognizers by kind.
//!
//! [`DetectionEngine`]: super::DetectionEngine

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Recognize entities given a per-recognizer context.
///
/// `Context` is associated rather than a trait parameter so each
/// impl declares exactly what it consumes — pattern recognizers
/// need allow/deny lists, NER recognizers need language hints, LLM
/// recognizers need their own per-call config. No common context
/// type appears in this trait.
///
/// Implementations are independent — the orchestrator
/// ([`DetectionEngine`]) runs each on its own and merges results.
///
/// Async because realistic impls dispatch to ONNX inference on a
/// blocking pool or call remote services.
///
/// [`DetectionEngine`]: super::DetectionEngine
#[async_trait]
pub trait Recognizer: Send + Sync {
    /// The per-call context this recognizer consumes.
    type Context;

    /// Detect entities in `ctx`. Offsets in returned entities are
    /// relative to whatever text `ctx` carries — callers rebase
    /// when integrating into a larger document.
    async fn run(&self, ctx: &Self::Context) -> Result<Entities>;

    /// Reset per-document state, called by the orchestrator at
    /// document boundaries. The default is a no-op — stateless
    /// recognizers don't need to override it.
    ///
    /// LLM-backed recognizers override this to clear coreference
    /// state between documents so per-document entity references
    /// don't bleed across runs.
    async fn reset(&self) {}
}

/// Which built-in recognizer to dispatch.
///
/// Used by [`Detection`] workflow nodes to enable/disable specific
/// recognizers and by [`Recognizers`] to look one up.
///
/// [`Detection`]: super::Detection
/// [`Recognizers`]: super::Recognizers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecognizerKind {
    /// LLM-backed recognizer (see [`LlmRecognizer`]).
    ///
    /// [`LlmRecognizer`]: super::LlmRecognizer
    Llm,
    /// NLP-engine recognizer (see [`NlpRecognizer`]).
    ///
    /// [`NlpRecognizer`]: super::NlpRecognizer
    Nlp,
    /// Pattern-based recognizer (see [`PatternRecognizer`]).
    ///
    /// [`PatternRecognizer`]: super::PatternRecognizer
    Pattern,
}
