//! Structured output for the VLM detect pass.

use nvisy_core::entity::EntityKind;
use nvisy_core::primitive::NormalizedBoundingBox;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One image entity discovered by the VLM.
///
/// The bounding box is normalised (`[0, 1]`); the agent converts
/// to pixel coordinates using the source image's [`Dimensions`]
/// before constructing the final [`Entity<Image>`].
///
/// [`Dimensions`]: nvisy_core::primitive::Dimensions
/// [`Entity<Image>`]: nvisy_core::entity::Entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VlmDetectedEntity {
    /// Specific entity kind.
    pub entity_kind: EntityKind,
    /// Normalised bounding box around the entity.
    #[serde(flatten)]
    pub bbox: NormalizedBoundingBox,
    /// VLM-asserted confidence in `[0, 1]`. Defaults to `0.5`
    /// when missing.
    #[serde(default)]
    pub confidence: Option<f64>,
    /// Short human-readable description of what the box contains
    /// (e.g. `"woman's face"`, `"driver's license number"`).
    /// Advisory metadata — surfaced on the entity for audit
    /// visibility but not consumed by the engine.
    #[serde(default)]
    pub description: Option<String>,
}

/// Serde wrapper matching the LLM's `{"entities": [...]}` response.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct VlmDetectedEntities {
    pub entities: Vec<VlmDetectedEntity>,
}
