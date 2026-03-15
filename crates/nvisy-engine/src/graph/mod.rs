//! Graph data model for pipeline definitions.
//!
//! A pipeline is represented as a set of [`GraphNode`]s connected by
//! [`GraphEdge`]s, collected into a [`Graph`]. Each node carries shared
//! fields (`id`, `retry`, `timeout`) alongside a [`GraphNodeKind`] that
//! determines what the node does.

mod context;
mod extraction;
mod lifecycle;
pub mod policy;
mod recognition;
mod refinement;

use std::collections::{HashMap, HashSet};

use nvisy_core::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

pub use self::context::{GenerateContext, LoadContext, SaveContext};
pub use self::extraction::{AudialExtraction, VisualExtraction};
pub use self::lifecycle::{Export, Import};
use self::policy::{RetryPolicy, TimeoutPolicy};
pub use self::recognition::{NamedEntityRecognition, PatternRecognition};
pub use self::refinement::{Fusion, Redaction};

/// The set of strongly-typed actions a pipeline node can perform.
///
/// Each variant maps to one or more [`Operation`](crate::operation::Operation)
/// implementations. Variants carry a dedicated configuration struct.
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

    /// Imports content into the pipeline for processing.
    Import(Import),
    /// Exports processed content to a target destination.
    Export(Export),
}

impl GraphNodeKind {
    /// Returns the pipeline phase for this node kind.
    ///
    /// Phases enforce execution ordering: edges must flow from equal or
    /// lower phase to equal or higher phase.
    ///
    /// | Phase | Actions                                    |
    /// |-------|--------------------------------------------|
    /// | 0     | Import, LoadContext                         |
    /// | 1     | VisualExtraction, AudialExtraction          |
    /// | 2     | NamedEntityRecognition, PatternRecognition  |
    /// | 3     | Fusion                                     |
    /// | 4     | Redaction, GenerateContext                  |
    /// | 5     | Export, SaveContext                         |
    #[must_use]
    pub fn phase(&self) -> u8 {
        match self {
            Self::Import(_) | Self::LoadContext(_) => 0,
            Self::VisualExtraction(_) | Self::AudialExtraction(_) => 1,
            Self::NamedEntityRecognition(_) | Self::PatternRecognition(_) => 2,
            Self::Fusion(_) => 3,
            Self::Redaction(_) | Self::GenerateContext(_) => 4,
            Self::Export(_) | Self::SaveContext(_) => 5,
        }
    }

    /// Validates action-specific configuration.
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::LoadContext(action) => action.validate(),
            Self::SaveContext(action) => action.validate(),
            Self::NamedEntityRecognition(action) => action.validate(),
            _ => Ok(()),
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

    /// Validates structural invariants.
    ///
    /// Checks that the graph contains at least one node, all node IDs are
    /// unique, node-level policies and action configs are valid, edges have
    /// no self-loops or duplicates, all edge endpoints reference existing
    /// node IDs, and edges respect pipeline phase ordering.
    #[must_use = "validation errors are silently ignored if the result is unused"]
    pub fn validate(&self) -> Result<(), Error> {
        if self.nodes.is_empty() {
            return Err(Error::validation(
                "graph must have at least one node",
                "compiler",
            ));
        }

        let mut node_map = HashMap::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if node_map.insert(node.id, node).is_some() {
                return Err(Error::validation(
                    format!("duplicate node id: {}", node.id),
                    "compiler",
                ));
            }
        }

        for node in &self.nodes {
            if let Some(retry) = &node.retry {
                retry
                    .validate()
                    .map_err(|e| Error::validation(format!("node {}: {e}", node.id), "compiler"))?;
            }
            if let Some(timeout) = &node.timeout {
                timeout
                    .validate()
                    .map_err(|e| Error::validation(format!("node {}: {e}", node.id), "compiler"))?;
            }
            node.kind.validate().map_err(|e| {
                Error::validation(format!("node {}: {}", node.id, e.message), "compiler")
            })?;
        }

        // Collect in-degree and out-degree per node for structural checks
        let mut in_degree: HashMap<Uuid, usize> = HashMap::new();
        let mut out_degree: HashMap<Uuid, usize> = HashMap::new();

        let mut seen_edges = HashSet::with_capacity(self.edges.len());
        for edge in &self.edges {
            if edge.source == edge.target {
                return Err(Error::validation(
                    format!("self-loop on node {}", edge.source),
                    "compiler",
                ));
            }

            if !seen_edges.insert((edge.source, edge.target)) {
                return Err(Error::validation(
                    format!("duplicate edge from {} to {}", edge.source, edge.target,),
                    "compiler",
                ));
            }

            let source = node_map.get(&edge.source).ok_or_else(|| {
                Error::validation(
                    format!("edge references unknown source node: {}", edge.source),
                    "compiler",
                )
            })?;
            let target = node_map.get(&edge.target).ok_or_else(|| {
                Error::validation(
                    format!("edge references unknown target node: {}", edge.target),
                    "compiler",
                )
            })?;

            let source_phase = source.kind.phase();
            let target_phase = target.kind.phase();
            if source_phase > target_phase {
                return Err(Error::validation(
                    format!(
                        "edge from node {} (phase {source_phase}) to node {} \
                         (phase {target_phase}) violates pipeline ordering",
                        edge.source, edge.target,
                    ),
                    "compiler",
                ));
            }

            *out_degree.entry(edge.source).or_default() += 1;
            *in_degree.entry(edge.target).or_default() += 1;
        }

        for node in &self.nodes {
            let incoming = in_degree.get(&node.id).copied().unwrap_or(0);
            let outgoing = out_degree.get(&node.id).copied().unwrap_or(0);

            match &node.kind {
                GraphNodeKind::Import(_) if incoming > 0 => {
                    return Err(Error::validation(
                        format!("import node {} must not have incoming edges", node.id),
                        "compiler",
                    ));
                }
                GraphNodeKind::Export(_) if outgoing > 0 => {
                    return Err(Error::validation(
                        format!("export node {} must not have outgoing edges", node.id),
                        "compiler",
                    ));
                }
                _ => {}
            }
        }

        Ok(())
    }
}
