//! Structured output types for NER candidate detection.

use nvisy_core::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Serde wrapper matching the LLM's `{"entities": [...]}` response.
/// Unwrapped by the agent into a plain `Vec<NerCandidate>` at the
/// public boundary.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) struct NerCandidates {
    /// Detected candidates.
    pub entities: Vec<NerCandidate>,
}

/// One entity candidate produced by an LLM NER pass.
///
/// Carries everything the LLM saw plus enough grounding for the
/// shared offset resolver to place the surface form back into a
/// byte range. The candidate does **not** carry offsets — the LLM
/// is not trusted to count bytes; resolution re-derives the
/// position from [`context`] via search.
///
/// `entity_id` is carried through verbatim from the LLM. The
/// pipeline does no cross-call coreference tracking, so the LLM's
/// id values are purely advisory metadata.
///
/// [`context`]: Self::context
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct NerCandidate {
    /// LLM-assigned identifier for the underlying real-world entity.
    /// Stable across coreferent mentions within one detection call,
    /// not stable across calls.
    #[serde(default)]
    pub entity_id: Option<String>,
    /// Specific entity type (may be absent for coreferent mentions like pronouns).
    pub entity_type: Option<EntityKind>,
    /// The matched text value — the literal surface form the LLM
    /// wants flagged. Used as the string to locate during
    /// localization.
    pub value: String,
    /// LLM-asserted confidence in `[0.0, 1.0]`. Not guaranteed in
    /// range; entity construction clamps. Missing means the model
    /// declined to score.
    pub confidence: Option<f64>,
    /// Short text window around the value, intended to uniquely
    /// locate it within the source. Typically the LLM is prompted
    /// to include 3-10 words on each side of `value`.
    ///
    /// Localization searches for this string in the source text;
    /// when it appears exactly once, the value's offset within it
    /// gives the candidate's location. If `context` is absent or
    /// matches zero/many times, the candidate is unresolvable.
    pub context: Option<String>,
    /// Brief description of the real-world entity (e.g. "CEO of
    /// Acme Corp, mentioned as the signatory"). Advisory metadata
    /// — not consumed by the engine, but preserved on the entity
    /// for downstream visibility.
    pub description: Option<String>,
    /// Index into [`RecognizerInput::hints`] when this candidate is
    /// the LLM's response to a user-supplied hint; `None` for
    /// fresh discoveries. Drives recognition-step stamping:
    /// `Some(i)` → annotation provenance with the hint's name;
    /// `None` → model provenance with the agent's model name.
    ///
    /// [`RecognizerInput::hints`]: nvisy_core::RecognizerInput::hints
    #[serde(default)]
    pub hint_id: Option<usize>,
}
