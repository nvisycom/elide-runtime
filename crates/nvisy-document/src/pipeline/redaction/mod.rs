//! Redaction subsystem: standalone apply + validate + export
//! pipeline.
//!
//! [`Engine::redact`] takes a [`RedactionInput`] referencing a
//! prior [`DetectionResult`] plus optional [`RedactionOverride`]s
//! and exports. It re-opens the original imports, applies the
//! (overridden) decisions, runs validation, and writes to the
//! configured exports.
//!
//! [`DetectionResult`]: super::detection::DetectionResult
//! [`Engine::redact`]: super::Engine::redact

mod applicator;
mod document;
mod input;
mod orchestrator;
mod pipeline;
mod override_;
mod result;
mod state;
mod status;

// `apply_overrides` is used inside `pipeline.rs` via
// `super::applicator::apply_overrides`; no public re-export.
pub(crate) use self::pipeline::{RedactionEngineState, RedactionPipeline};

pub use self::input::RedactionInput;
pub use self::override_::{
    RedactionAddEntity, RedactionDecision, RedactionOverride, validate_overrides,
};
pub use self::result::{RedactionEntry, RedactionFilter, RedactionResult, RedactionSnapshot};
pub(crate) use self::state::RedactionState;
pub use self::status::RedactionStatus;
