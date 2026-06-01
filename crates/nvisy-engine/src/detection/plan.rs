//! Detection node plan.
//!
//! [`Detection`] runs at **phase 3**, after extraction has produced
//! per-modality blocks and before deduplication. It carries the
//! per-run filters the engine applies during dispatch:
//!
//! - [`kinds`] is the per-plan recognizer allowlist by name. Empty =
//!   run every recognizer registered on the engine. Names that don't
//!   match any registered recognizer are warn-logged at dispatch
//!   time and silently skipped (Presidio-style lenient filter).
//!   Built-in names live in [`names`].
//! - [`entity_kinds`] is the entity-kind allowlist honored by every
//!   dispatched recognizer. Empty = no filter.
//!
//! Recognizer construction lives in [`DetectionEngine`], built once
//! at engine startup. This plan node never builds anything; it just
//! tells the pre-built engine which slots to dispatch this run.
//!
//! Confidence-based filtering is centralised in the deduplication
//! phase, applied once after per-recognizer calibration. There is no
//! per-plan confidence threshold — operators tune trust via the
//! dedup calibration map plus the single dedup threshold.
//!
//! [`DetectionEngine`]: super::DetectionEngine
//! [`names`]: super::recognizer::names
//! [`kinds`]: Detection::kinds
//! [`entity_kinds`]: Detection::entity_kinds

use nvisy_ontology::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Per-plan detection knobs.
#[derive(Debug, Clone, Default, PartialEq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// Per-plan recognizer allowlist by name. Empty = run every
    /// recognizer registered on the engine.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<String>,
    /// Entity-kind allowlist applied to every dispatched recognizer.
    /// Empty = all kinds permitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_kinds: Vec<EntityKind>,
}

impl Detection {
    /// Validate the configuration.
    pub fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        Validate::validate(self)
    }
}
