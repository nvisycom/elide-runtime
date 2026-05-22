//! [`Recognizer`] trait, shared cross-recognizer config, and
//! built-in implementations.
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
//! [`DetectionParams`] carries the workflow-level hints that apply
//! to *every* recognizer (entity-kind allowlist, confidence
//! threshold). Recognizer-specific knobs live alongside each
//! recognizer: [`NerDetection`] for NER, [`LlmDetection`] for the
//! LLM path, [`PatternDetection`] for the pattern path.
//! [`NerDetection`] is currently empty — every NER knob today is
//! already shared via [`DetectionParams`] — but is wired through
//! so future NER-only config has an obvious home.
//!
//! [`DetectionContext`]: crate::DetectionContext
//! [`Entities`]: nvisy_ontology::entity::Entities
//! [`DetectionEngine`]: crate::DetectionEngine

mod language_model;
mod named_entity;
mod pattern;

use async_trait::async_trait;
use nvisy_ontology::entity::{Entities, EntityKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub use self::language_model::{LlmDetection, LlmRecognizer};
pub use self::named_entity::{NerDetection, NerRecognizer};
pub use self::pattern::{PatternDetection, PatternRecognizer};
use crate::DetectionContext;
use crate::error::Result;

/// Cross-recognizer hints applied to every detection call.
///
/// `entity_kinds` and `confidence_threshold` are honored by every
/// built-in recognizer (NER, pattern, LLM). They live here — not
/// on any per-recognizer config — because they aren't specific to
/// any one backend: the workflow says "I want these kinds, above
/// this confidence" and every recognizer in the engine applies the
/// constraint.
#[derive(Debug, Clone, Default, PartialEq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DetectionParams {
    /// Entity kinds to detect. An empty list means all known kinds.
    #[serde(default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence threshold for detections (0.0 to 1.0).
    /// When `None`, confidence filtering is disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub confidence_threshold: Option<f64>,
}

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
    /// Detect entities in `ctx.text`. Offsets in returned entities
    /// are relative to `ctx.text` — the caller rebases when
    /// integrating into a larger document.
    async fn run(&self, ctx: &DetectionContext) -> Result<Entities>;

    /// Reset per-document state, called by the orchestrator at
    /// document boundaries. The default is a no-op — stateless
    /// recognizers don't need to override it.
    ///
    /// LLM-backed recognizers override this to clear coreference
    /// state between documents so per-document entity references
    /// don't bleed across runs.
    async fn reset(&self) {}
}
