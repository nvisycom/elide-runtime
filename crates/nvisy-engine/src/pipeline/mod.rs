//! Pipeline engine: configuration, execution, and per-pass
//! tracking.
//!
//! The pipeline runs as two distinct subsystems, dispatched by
//! [`Engine::detect`] and [`Engine::redact`] respectively. See
//! `ARCHITECTURE.md` in this directory for the contract.
//!
//! # Submodules
//!
//! - `config`: [`RuntimeConfig`] and per-subsystem sections.
//! - `detection`: detection-pass types + orchestrator
//!   (extraction → detection → deduplication).
//! - `redaction`: redaction-pass types + override applicator +
//!   orchestrator (override-apply → redaction → validation →
//!   export).
//! - `engine`: the [`Engine`] facade that owns shared resources
//!   and routes calls into the per-subsystem pipelines.
//!
//! [`Engine::detect`]: Engine::detect
//! [`Engine::redact`]: Engine::redact

mod config;
pub mod detection;
mod engine;
pub mod redaction;

#[cfg(feature = "image")]
pub use self::config::OcrExtractorConfig;
#[cfg(feature = "audio")]
pub use self::config::SttExtractorConfig;
pub use self::config::{
    AudioPlan, DeduplicationParams, Detection, DetectionConfig, EngineConfig, Extraction,
    ExtractionConfig, ImagePlan, NerBackend, NerDetection, PatternDetection, Redaction,
    RedactionConfig, ResourceLimits, RuntimeConfig, TabularPlan, TextPlan, Validation,
};
pub use self::detection::{
    DetectionEntry, DetectionFilter, DetectionInput, DetectionResult, DetectionSnapshot,
    DetectionStatus,
};
pub use self::engine::Engine;
pub use self::redaction::{
    RedactionAddEntity, RedactionDecision, RedactionEntry, RedactionFilter, RedactionInput,
    RedactionOverride, RedactionResult, RedactionSnapshot, RedactionStatus, validate_overrides,
};
pub use crate::core::Plan;
pub use crate::phases::deduplication::DeduplicationPhase;
pub use crate::phases::detection::DetectionPhase;
pub use crate::phases::extraction::ExtractionPhase;
pub use crate::phases::redaction::phase::RedactionPhase;
pub use crate::phases::validation::ValidationPhase;
