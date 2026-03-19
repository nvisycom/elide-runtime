//! Graph data model for pipeline definitions.
//!
//! A pipeline is represented as a set of [`GraphNode`]s connected by
//! [`GraphEdge`]s, collected into a [`Graph`]. Each node carries shared
//! fields (`id`, `retry`, `timeout`) alongside a [`GraphNodeKind`] that
//! determines what the node does.

mod context;
mod extraction;
mod lifecycle;
mod policy;
mod recognition;
mod refinement;
mod validate;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::Error;
use validator::Validate;

pub use self::context::{GenerateContext, LoadContext, SaveContext};
pub use self::extraction::{AudialExtraction, VisualExtraction};
pub use self::lifecycle::{CompressionFormat, EncryptionFormat, Export, Import};
pub use self::policy::{BackoffStrategy, RetryPolicy, TimeoutBehavior, TimeoutPolicy};
pub use self::recognition::{NamedEntityRecognition, PatternRecognition};
pub use self::refinement::{Fusion, FusionStrategy, Redaction, Validation};

/// The set of strongly-typed actions a pipeline node can perform.
///
/// Each variant maps to one or more [`Operation`] implementations.
/// Variants carry a dedicated configuration struct.
///
/// [`Operation`]: crate::operation::Operation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GraphNodeKind {
    /// Loads reference-data contexts required by downstream actions.
    LoadContext(LoadContext),
    /// Persists contexts produced during the pipeline run.
    SaveContext(SaveContext),
    /// Generates a new context from detection results and content data.
    GenerateContext(GenerateContext),

    /// Extracts text and entities from images and scanned documents.
    VisualExtraction(VisualExtraction),
    /// Extracts text from speech audio.
    AudialExtraction(AudialExtraction),

    /// Detects named entities via language model inference.
    NamedEntityRecognition(NamedEntityRecognition),
    /// Detects entities via regex, checksum, dictionary, and heuristic rules.
    PatternRecognition(PatternRecognition),

    /// Merges and scores entities from multiple detection sources.
    Fusion(Fusion),
    /// Applies redaction instructions to produce output content.
    Redaction(Redaction),
    /// Verifies that redacted content does not leak original values.
    Validation(Validation),

    /// Imports content into the pipeline for processing.
    Import(Import),
    /// Exports processed content to a target destination.
    Export(Export),
}

impl std::fmt::Display for GraphNodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadContext(_) => f.write_str("load_context"),
            Self::SaveContext(_) => f.write_str("save_context"),
            Self::GenerateContext(_) => f.write_str("generate_context"),
            Self::VisualExtraction(_) => f.write_str("visual_extraction"),
            Self::AudialExtraction(_) => f.write_str("audial_extraction"),
            Self::NamedEntityRecognition(_) => f.write_str("named_entity_recognition"),
            Self::PatternRecognition(_) => f.write_str("pattern_recognition"),
            Self::Fusion(_) => f.write_str("fusion"),
            Self::Redaction(_) => f.write_str("redaction"),
            Self::Validation(_) => f.write_str("validation"),
            Self::Import(_) => f.write_str("import"),
            Self::Export(_) => f.write_str("export"),
        }
    }
}

impl GraphNodeKind {
    /// Returns the pipeline phase for this node kind.
    ///
    /// Phases enforce execution ordering: edges must flow from equal or
    /// lower phase to equal or higher phase.
    ///
    /// | Phase | Actions                                           |
    /// |-------|---------------------------------------------------|
    /// | 0     | Import, LoadContext                                |
    /// | 1     | VisualExtraction, AudialExtraction                 |
    /// | 2     | NamedEntityRecognition, PatternRecognition         |
    /// | 3     | Fusion                                            |
    /// | 4     | Redaction, GenerateContext                         |
    /// | 5     | Validation                                        |
    /// | 6     | Export, SaveContext                                |
    #[must_use]
    pub fn phase(&self) -> u8 {
        match self {
            Self::Import(_) | Self::LoadContext(_) => 0,
            Self::VisualExtraction(_) | Self::AudialExtraction(_) => 1,
            Self::NamedEntityRecognition(_) | Self::PatternRecognition(_) => 2,
            Self::Fusion(_) => 3,
            Self::Redaction(_) | Self::GenerateContext(_) => 4,
            Self::Validation(_) => 5,
            Self::Export(_) | Self::SaveContext(_) => 6,
        }
    }

    /// Validates action-specific configuration.
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Import(cfg) => validate_struct(cfg),
            Self::LoadContext(cfg) => validate_struct(cfg),
            Self::SaveContext(cfg) => validate_struct(cfg),
            Self::NamedEntityRecognition(cfg) => cfg.validate(),
            Self::Export(_)
            | Self::VisualExtraction(_)
            | Self::AudialExtraction(_)
            | Self::PatternRecognition(_)
            | Self::Fusion(_)
            | Self::Redaction(_)
            | Self::Validation(_)
            | Self::GenerateContext(_) => Ok(()),
        }
    }
}

/// A node in the pipeline graph.
///
/// Common fields (`id`, `retry`, `timeout`) live on the struct directly.
/// The action-specific payload is carried in [`GraphNodeKind`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphNode {
    /// Unique identifier for this node within the graph.
    pub id: Uuid,
    /// Optional retry policy for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
    /// Optional timeout policy for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<TimeoutPolicy>,
    /// Action-specific payload.
    #[serde(flatten)]
    pub kind: GraphNodeKind,
}

impl GraphNode {
    /// Creates a new node with the given ID and action kind.
    pub fn new(id: Uuid, kind: GraphNodeKind) -> Self {
        Self {
            id,
            retry: None,
            timeout: None,
            kind,
        }
    }

    /// Returns the retry policy, if configured.
    #[must_use]
    pub fn retry(&self) -> Option<&RetryPolicy> {
        self.retry.as_ref()
    }

    /// Returns the timeout policy, if configured.
    #[must_use]
    pub fn timeout(&self) -> Option<&TimeoutPolicy> {
        self.timeout.as_ref()
    }
}

/// A directed edge connecting two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphEdge {
    /// ID of the upstream node.
    pub source: Uuid,
    /// ID of the downstream node.
    pub target: Uuid,
}

/// A complete pipeline graph: nodes and directed edges forming a DAG.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Graph {
    /// All nodes in the pipeline.
    pub nodes: Vec<GraphNode>,
    /// Directed edges describing data flow between nodes.
    pub edges: Vec<GraphEdge>,
}

impl Graph {
    /// Creates a new graph from nodes and edges.
    pub fn new(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> Self {
        Self { nodes, edges }
    }
}

/// Convert a `validator::Validate` result into our `Error` type.
fn validate_struct(v: &impl Validate) -> Result<(), Error> {
    v.validate()
        .map_err(|e| Error::validation(e.to_string(), "compiler"))
}
