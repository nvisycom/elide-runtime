//! Top-level engine types and the [`Engine`] implementation.
//!
//! [`Engine`] orchestrates the detect -> evaluate -> redact pipeline
//! and drives the DAG execution graph.

mod analytics;
mod config;
mod default;
mod executor;
mod orchestrator;
mod plan;
mod runs;

pub use self::analytics::AnalyticsSnapshot;
pub use self::config::{
    EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection,
};
pub use self::default::{Engine, EngineInput, EngineOutput};
pub use self::runs::{NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary};
