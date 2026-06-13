//! Detection node plan.
//!
//! [`Detection`] runs at **phase 3**, after extraction has produced
//! per-modality blocks and before deduplication. It carries the
//! per-run filters the engine applies during dispatch:
//!
//! - [`entity_kinds`] is the entity-kind allowlist honored by every
//!   dispatched recognizer. Empty = no filter.
//!
//! Every recognizer registered on the engine runs on every request;
//! there is no per-plan recognizer-name allowlist. Operators shape
//! the active set by tuning what gets registered at engine startup
//! through `[detection.*]` config sections (and any custom
//! recognizers added programmatically), not per request.
//!
//! Recognizer construction lives in
//! [`RecognizerRegistry`],
//! built once at engine startup. This plan node never builds
//! anything; it just carries the post-detection filters the registry
//! applies during dispatch.
//!
//! Confidence-based filtering is centralised in the deduplication
//! phase, applied once after per-recognizer calibration. There is no
//! per-plan confidence threshold — operators tune trust via the
//! dedup calibration map plus the single dedup threshold.
//!
//! [`entity_kinds`]: Detection::entity_kinds
//! [`RecognizerRegistry`]: crate::detection::RecognizerRegistry

use nvisy_core::entity::EntityLabelRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Per-plan detection knobs.
#[derive(Debug, Clone, Default, PartialEq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// Entity-label allowlist applied to every dispatched recognizer.
    /// Empty = all labels permitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<EntityLabelRef>,
}

impl Detection {
    /// Validate the configuration.
    pub fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        Validate::validate(self)
    }
}
