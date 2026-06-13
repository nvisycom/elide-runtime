//! Detection subsystem: standalone recognise + evaluate pipeline.
//!
//! [`Engine::detect`] runs imports → extraction → detection →
//! deduplication → policy evaluation and stops. The result is an
//! immutable [`DetectionResult`] holding the per-document audits
//! (with `Execution::Pending` decisions) plus the original
//! `ImportFile` references so a follow-up [`Engine::redact`] call
//! can re-open the same content for byte rewriting.
//!
//! Detection results are first-class addressable artifacts: one
//! detection can feed multiple redaction passes (e.g. preview with
//! `Mask`, then commit with `Fake`), and the user can review +
//! override the decisions between detect and redact.
//!
//! [`Engine::detect`]: super::Engine::detect
//! [`Engine::redact`]: super::Engine::redact

mod document;
mod input;
mod orchestrator;
mod pipeline;
mod result;
mod state;
mod status;

pub use self::input::DetectionInput;
pub(crate) use self::pipeline::{DetectionEngineState, DetectionPipeline};
pub use self::result::{DetectionEntry, DetectionFilter, DetectionResult, DetectionSnapshot};
pub(crate) use self::state::DetectionState;
pub use self::status::DetectionStatus;
