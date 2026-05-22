//! Structured output types for NER candidate detection.

use nvisy_ontology::entity::{EntityCategory, EntityKind};
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
/// Carries everything the LLM saw plus enough grounding for a
/// downstream [`NerVerifyAgent`] to localize the surface form back
/// into the source text. The candidate does **not** carry offsets
/// — the LLM is not trusted to count bytes, and the verifier
/// re-derives the location from [`context`] via search.
///
/// Multiple candidates may share `entity_id` when the LLM marks
/// them as coreferent (different surface mentions of the same
/// real-world entity). When `entity_id` is `None` the candidate
/// participates in detection but not in cross-call coreference
/// tracking.
///
/// [`NerVerifyAgent`]: crate::agent::NerVerifyAgent
/// [`context`]: Self::context
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct NerCandidate {
    /// LLM-assigned identifier for the underlying real-world entity.
    /// Stable across coreferent mentions within one detection call,
    /// not stable across calls. `None` when the LLM declined to
    /// link this mention to a previously seen entity.
    #[serde(default)]
    pub entity_id: Option<String>,
    /// Broad classification (may be absent for coreferent mentions like pronouns).
    pub category: Option<EntityCategory>,
    /// Specific entity type (may be absent for coreferent mentions like pronouns).
    pub entity_type: Option<EntityKind>,
    /// The matched text value — the literal surface form the LLM
    /// wants flagged. Verifier uses this as the string to locate.
    pub value: String,
    /// LLM-asserted confidence in `[0.0, 1.0]`. Not guaranteed in
    /// range; verifier clamps. Missing means the model declined to
    /// score.
    pub confidence: Option<f64>,
    /// Short text window around the value, intended to uniquely
    /// locate it within the source. Typically the LLM is prompted
    /// to include 3-10 words on each side of `value`.
    ///
    /// The verifier searches for this string in the source text;
    /// when it appears exactly once, the value's offset within it
    /// gives the candidate's location. If `context` is absent or
    /// matches zero/many times, the candidate is unresolvable.
    pub context: Option<String>,
    /// Brief description of the real-world entity (e.g. "CEO of
    /// Acme Corp, mentioned as the signatory"). Carried forward
    /// via [`KnownNerEntity`] so the LLM can disambiguate entities
    /// across chunks.
    pub description: Option<String>,
}

/// A previously identified entity carried as context between
/// detection calls.
///
/// Lighter than [`NerCandidate`] — holds only the information the
/// LLM needs to recognise and reuse an existing `entity_id`.
/// Created via [`NerContext::merge`].
///
/// [`NerContext::merge`]: super::NerContext::merge
#[derive(Debug, Clone, PartialEq)]
pub struct KnownNerEntity {
    /// Stable identifier (e.g. `"person_1"`).
    pub entity_id: String,
    /// Entity type, if known.
    pub entity_type: Option<EntityKind>,
    /// All surface forms seen so far (e.g. `["John Smith", "John", "Mr. Smith"]`).
    pub values: Vec<String>,
    /// Accumulated descriptions from successive detection calls.
    pub descriptions: Vec<String>,
}
