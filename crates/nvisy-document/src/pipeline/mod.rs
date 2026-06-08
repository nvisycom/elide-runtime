//! Pipeline engine: configuration, execution, and run tracking.
//!
//! The pipeline executes a user-submitted [`EngineInput`] — a flat,
//! fixed-order plan of phases (extraction → detection → dedup →
//! redaction → validation). The [`Engine`] is a thin facade that
//! delegates actual execution to per-run state in `run::Pipeline`.
//!
//! # Submodules
//!
//! - `config`: [`RuntimeConfig`] and per-subsystem sections.
//! - `document_pipeline`: per-document `DocumentPipeline` struct
//!   holding one concrete instance of each phase.
//! - `run`: per-run lifecycle (`run::Pipeline`).
//! - `orchestrator`: concurrent per-document fan-out.
//! - `runs`: in-memory run lifecycle tracking.

mod config;
pub mod detection;
mod document_pipeline;
mod engine;
mod orchestrator;
pub mod redaction;
mod run;
mod runs;

#[cfg(feature = "image")]
pub use self::config::OcrExtractorConfig;
#[cfg(feature = "audio")]
pub use self::config::SttExtractorConfig;
pub use self::config::{
    AudioPlan, DeduplicationParams, Detection, DetectionConfig, EngineConfig, Extraction,
    ExtractionConfig, ImagePlan, NerBackend, NerDetection, PatternDetection, Redaction,
    RedactionConfig, ResourceLimits, RuntimeConfig, TabularPlan, TextPlan,
};
pub use self::detection::{
    DetectionEntry, DetectionFilter, DetectionInput, DetectionResult, DetectionSnapshot,
    DetectionStatus,
};
pub use self::engine::{Engine, EngineInput, EngineOutput};
pub use self::redaction::{
    RedactionAddEntity, RedactionDecision, RedactionEntry, RedactionFilter, RedactionInput,
    RedactionOverride, RedactionResult, RedactionSnapshot, RedactionStatus, validate_overrides,
};
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
// Re-export the plan struct since several phase modules read
// `input.plan.X` directly. The `Phase` / `PhaseTarget` / `PhaseInfo`
// trio is gone — phases are concrete structs now.
pub use crate::core::Plan;
pub use crate::phases::deduplication::DeduplicationPhase;
pub use crate::phases::detection::DetectionPhase;
pub use crate::phases::extraction::ExtractionPhase;
pub use crate::phases::redaction::phase::RedactionPhase;
pub use crate::phases::validation::ValidationPhase;
