//! Detection subsystem: standalone recognise + evaluate pipeline.
//!
//! [`DetectionEngine::detect`] runs imports → extraction →
//! detection → deduplication → policy evaluation and stops. The
//! result is an immutable [`DetectionResult`] holding the
//! per-document audits (with `Execution::Pending` decisions) plus
//! the original `ImportFile` references so a follow-up
//! [`RedactionEngine::redact`][rr] call can re-open the same
//! content for byte rewriting.
//!
//! Detection results are first-class addressable artifacts: one
//! detection can feed multiple redaction passes (e.g. preview
//! with `Mask`, then commit with `Fake`), and the user can review
//! + override the decisions between detect and redact.
//!
//! [rr]: crate::redaction::RedactionEngine::redact

mod config;
mod document;
mod engine;
mod extraction;
mod input;
mod orchestrator;
pub mod phases;
mod pipeline;
mod plan;
mod result;
mod state;
mod status;

pub use self::config::{
    DetectionConfig, DetectionResources, NerBackend, NerDetection, PatternDetection,
};
pub use self::engine::DetectionEngine;
pub use self::extraction::ExtractionConfig;
#[cfg(feature = "image")]
pub use self::extraction::{OcrBackend, OcrExtractorConfig};
#[cfg(feature = "audio")]
pub use self::extraction::{SttBackend, SttExtractorConfig};
pub use self::input::DetectionInput;
pub use self::plan::{
    AudioPlan, DeduplicationParams, DetectionPlan, Extraction, ImagePlan, TabularPlan, TextPlan,
};
pub use self::result::{DetectionEntry, DetectionFilter, DetectionResult, DetectionSnapshot};
pub use self::state::DetectionState;
pub use self::status::DetectionStatus;
