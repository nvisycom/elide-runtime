#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod compiler;
pub mod operation;
pub mod pipeline;
pub mod provenance;

// Re-export graph data model for pipeline definitions.
pub use compiler::{
    ActionKind, ActionNode, Graph, GraphEdge, GraphNode, GraphNodeKind, SourceNode, TargetNode,
};
// Re-export retry and timeout policies for pipeline nodes.
pub use compiler::{BackoffStrategy, RetryPolicy, TimeoutBehavior, TimeoutPolicy};
pub use pipeline::{
    DefaultEngine, EngineSection, LlmSection, NvisyConfig, OcrSection, SttSection, TtsSection,
};
