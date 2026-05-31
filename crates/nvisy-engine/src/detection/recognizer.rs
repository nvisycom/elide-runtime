//! [`Recognizer`] trait + [`RecognizerKind`] enum — the
//! per-recognizer abstraction the [`DetectionEngine`] dispatches
//! against, and the enum used by plan nodes and config sections
//! to refer to recognizers by kind.
//!
//! The trait is modality-parameterized via [`Recognizer::Modality`]:
//! text recognizers emit `Vec<Entity<Text>>`, image recognizers emit
//! `Vec<Entity<Image>>`. Each impl declares its `Context` (the
//! per-call config + input bundle it consumes); the engine driver's
//! per-modality dispatch path knows how to build that context from
//! the document envelope and never crosses modalities.
//!
//! [`DetectionEngine`]: super::DetectionEngine

use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Modality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Recognize entities for one modality, given a per-call context.
///
/// `Modality` is the modality this recognizer scans + emits entities
/// for. `Context` carries everything the recognizer needs for a
/// single call — typically the input payload (text bytes, image
/// bytes + dimensions) plus per-call filters (entity-kind allowlist,
/// confidence threshold, etc.). Each impl declares both.
///
/// Recognizers are independent — the orchestrator
/// ([`DetectionEngine`]) runs each one on its own and merges
/// results across the plan's enabled set for the modality
/// being scanned.
///
/// Async because realistic impls dispatch to ONNX inference on a
/// blocking pool or call remote services.
///
/// [`DetectionEngine`]: super::DetectionEngine
#[async_trait::async_trait]
pub trait Recognizer: Send + Sync {
    /// The modality this recognizer scans + emits entities for.
    type Modality: Modality;

    /// The per-call config + input bundle this recognizer
    /// consumes. Associated rather than passed via a fat shared
    /// struct so each impl declares exactly what it needs.
    type Context: Send + Sync;

    /// Detect entities given the per-call context. Returned
    /// entities are in modality-local coordinates; callers rebase
    /// into document coordinates.
    async fn run(&self, ctx: &Self::Context) -> Result<Vec<Entity<Self::Modality>>>;

    /// Reset per-document state, called by the orchestrator at
    /// document boundaries. The default is a no-op — stateless
    /// recognizers don't need to override it.
    ///
    /// LLM-backed recognizers override this to clear cumulative
    /// usage trackers between documents.
    async fn reset(&self) {}
}

/// Which built-in recognizer to dispatch.
///
/// Used by [`Detection`] plan nodes to enable/disable specific
/// recognizers. Each kind belongs to a single modality; the engine
/// driver dispatches kinds against the registry matching their
/// modality.
///
/// [`Detection`]: super::Detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecognizerKind {
    /// LLM-backed text recognizer — wraps an [`LlmNerPipeline`].
    ///
    /// [`LlmNerPipeline`]: nvisy_agent::pipeline::LlmNerPipeline
    Llm,
    /// NER-engine text recognizer (see [`NerRecognizer`]).
    ///
    /// [`NerRecognizer`]: super::NerRecognizer
    Ner,
    /// Pattern-based text recognizer (see [`PatternRecognizer`]).
    ///
    /// [`PatternRecognizer`]: super::PatternRecognizer
    Pattern,
    /// VLM-backed image recognizer — wraps a [`VlmPipeline`].
    ///
    /// [`VlmPipeline`]: nvisy_agent::pipeline::VlmPipeline
    Vlm,
}
