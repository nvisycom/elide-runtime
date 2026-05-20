//! [`Recognizer`] trait and built-in implementations.
//!
//! A recognizer is the unit of entity-detection plug-in: it
//! consumes a [`DetectionContext`] and produces [`Entities`].
//! Recognizers run independently — combination, dedup, and
//! threshold filtering happen at higher layers (the
//! [`DetectionEngine`] and, downstream, the redaction pipeline).
//!
//! Built-in recognizers cover the three detection sources today:
//!
//! - [`NerRecognizer`] wraps `nvisy_nlp::Engine` (NER + optional
//!   language detection, tokens, keywords).
//! - [`PatternRecognizer`] wraps `nvisy_pattern::PatternEngine`
//!   (regex, dictionary, allow/deny, context-aware boosting).
//! - [`LlmRecognizer`] wraps `nvisy_rig::agent::NerAgent`
//!   (LLM-driven detection with coreference state).
//!
//! [`DetectionContext`]: crate::DetectionContext
//! [`Entities`]: nvisy_ontology::entity::Entities
//! [`DetectionEngine`]: crate::DetectionEngine

mod language_model;
mod named_entity;
mod pattern;

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;

pub use self::language_model::LlmRecognizer;
pub use self::named_entity::NerRecognizer;
pub use self::pattern::PatternRecognizer;
use crate::DetectionContext;
use crate::error::Result;

/// Recognize entities in the text carried by a
/// [`DetectionContext`].
///
/// Implementations are independent — the engine runs each on its
/// own and merges results. Each recognizer is responsible only for
/// detecting; per-recognizer filtering happens via the
/// [`DetectionContext`] fields the recognizer chooses to honor.
///
/// Async because realistic implementations dispatch to ONNX
/// inference on a blocking pool or call remote services.
///
/// [`DetectionContext`]: crate::DetectionContext
#[async_trait]
pub trait Recognizer: Send + Sync {
    /// Stable identifier surfaced in tracing spans and (eventually)
    /// `recognition_metadata.recognizer_name`. Should be short
    /// (kebab-case is conventional in this crate: `"ner"`,
    /// `"pattern"`, `"llm"`).
    fn name(&self) -> &str;

    /// Detect entities in `ctx.text`. Offsets in returned entities
    /// are relative to `ctx.text` — the caller rebases when
    /// integrating into a larger document.
    async fn recognize(&self, ctx: &DetectionContext<'_>) -> Result<Entities>;

    /// Reset per-document state, called by the orchestrator at
    /// document boundaries. The default is a no-op — stateless
    /// recognizers don't need to override it.
    ///
    /// LLM-backed recognizers override this to clear coreference
    /// state between documents so per-document entity references
    /// don't bleed across runs.
    async fn reset(&self) {}
}
