#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod compiler;
pub mod operation;
pub mod pipeline;
pub mod provenance;

// Re-export graph data model for pipeline definitions.
// Re-export retry and timeout policies for pipeline nodes.
pub use self::compiler::{BackoffStrategy, RetryPolicy, TimeoutBehavior, TimeoutPolicy};
pub use self::compiler::{Graph, GraphEdge, GraphNode, GraphNodeKind};
pub use self::pipeline::{
    DefaultEngine, EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection,
};
