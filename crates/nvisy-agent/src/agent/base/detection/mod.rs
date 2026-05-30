//! Shared LLM-driven detection configuration.
//!
//! [`LlmNerContext`] is the detect-pass config consumed by
//! [`NerAgent`] and (legacy) by [`CvAgent`]. [`LlmNerVerification`]
//! is the verify-pass config consumed by [`NerVerifyAgent`]. The
//! two types intentionally split so each method can grow
//! method-specific fields without leaking them into the others'
//! prompts.
//!
//! [`NerAgent`]: crate::agent::ner::NerAgent
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
//! [`CvAgent`]: crate::agent::cv::CvAgent

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use nvisy_ontology::primitive::ConfidenceThreshold;
use uuid::Uuid;

/// Fallback hint used in prompts when no specific entity types are requested.
pub(crate) const ALL_TYPES_HINT: &str = "all entity types";

/// One user-supplied hint — a `Hint`-strength inclusion annotation
/// lifted from the document, lifted into the detect prompt so the
/// LLM has both a discovery task and a per-hint adjudication task
/// in the same call.
///
/// The LLM either emits an entity carrying `hint_id = Some(this
/// hint's index)` — confirming or relocating the hint — or omits
/// any reference to it, which the engine treats as an implicit
/// rejection.
#[derive(Debug, Clone)]
pub struct NerHint {
    /// Uploader-supplied name (optional). Forwarded into the
    /// resulting entity's [`RecognitionMethod::Annotation`] when
    /// the LLM confirms or adjusts this hint.
    ///
    /// [`RecognitionMethod::Annotation`]: nvisy_ontology::entity::RecognitionMethod::Annotation
    pub name: Option<String>,
    /// Uploader-claimed category (optional).
    pub category: Option<EntityCategory>,
    /// Uploader-claimed entity kind (optional).
    pub entity_kind: Option<EntityKind>,
    /// Byte range start in the source text.
    pub start: usize,
    /// Byte range end (exclusive) in the source text.
    pub end: usize,
}

/// Configuration for the NER detect pass: which entity kinds to
/// look for, at what confidence threshold, what document-level
/// signals to surface in the prompt, and any user-supplied hints
/// to fold in.
///
/// [`hints`](Self::hints) are user-supplied [`Hint`]-strength
/// inclusion annotations the engine lifts into the prompt so the
/// LLM can confirm, adjust, or implicitly reject each one in the
/// same pass as open-ended discovery. Exclusions don't flow through
/// the prompt — they're always assertions and enforced by a
/// post-detection filter regardless of detector.
///
/// [`Hint`]: nvisy_ontology::entity::AnnotationStrength::Hint
#[derive(Debug, Clone, Default)]
pub struct LlmNerContext {
    /// Entity kinds to detect (empty = all).
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence score to include a detection. When `None`,
    /// no confidence filtering is applied.
    pub confidence_threshold: Option<ConfidenceThreshold>,
    /// System prompt override (if set, replaces the agent's default).
    pub system_prompt: Option<String>,
    /// User-supplied hint regions to fold into the prompt for
    /// per-hint adjudication alongside open-ended discovery.
    /// Empty when the document has no Hint inclusions.
    pub hints: Vec<NerHint>,
    /// Document-level classification labels (e.g. `"medical"`,
    /// `"gdpr-request"`). Rendered into the prompt as document
    /// context so the LLM can adjust its sensitivity to
    /// domain-specific terms.
    pub labels: Vec<String>,
    /// Correlation UUID propagated through the tracing span. Not
    /// used for detection.
    pub correlation_id: Option<Uuid>,
}

/// Configuration for the NER verify pass — whole-audit LLM filter
/// over already-built entities.
///
/// The entity payload (what to verify) is passed through the
/// [`verify`] method signature itself, not this config; this type
/// carries only the document-wide context that biases the LLM's
/// confirm/reject/adjust verdicts.
///
/// [`verify`]: crate::pipeline::LlmNerPipeline
#[derive(Debug, Clone, Default)]
pub struct LlmNerVerification {
    /// System prompt override (if set, replaces the agent's default).
    pub system_prompt: Option<String>,
    /// Document-level classification labels. Rendered into the
    /// prompt so the LLM can adjust its verdicts based on
    /// domain-specific context.
    pub labels: Vec<String>,
    /// Correlation UUID propagated through the tracing span.
    pub correlation_id: Option<Uuid>,
}
