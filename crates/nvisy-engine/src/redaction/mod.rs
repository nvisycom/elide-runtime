//! Redaction subsystem: standalone apply + validate + export
//! pipeline.
//!
//! [`RedactionEngine::redact`] takes a [`RedactionInput`]
//! referencing a prior [`DetectionResult`] plus optional
//! [`RedactionOverride`]s and exports. It re-opens the original
//! imports, applies the (overridden) decisions, runs validation,
//! and writes to the configured exports.
//!
//! [`DetectionResult`]: crate::detection::DetectionResult

mod applicator;
mod config;
mod document;
mod engine;
mod input;
mod orchestrator;
mod override_;
pub mod phases;
mod pipeline;
mod plan;
mod result;
mod state;
mod status;

pub use self::config::RedactionConfig;
pub use self::engine::RedactionEngine;
pub use self::input::RedactionInput;
pub use self::override_::{
    RedactionAddEntity, RedactionDecision, RedactionOverride, validate_overrides,
};
pub use self::plan::{Redaction, RedactionPlan, Validation};
pub use self::result::{RedactionEntry, RedactionFilter, RedactionResult, RedactionSnapshot};
pub use self::status::RedactionStatus;
