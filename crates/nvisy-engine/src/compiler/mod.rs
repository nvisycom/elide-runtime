//! Pipeline compilation: graph construction, validation, and execution planning.
//!
//! The [`Compiler`] is the entry-point for turning a [`Graph`] into an
//! [`ExecutionPlan`]. It carries optional default retry and timeout policies
//! that are applied to nodes which don't specify their own.

mod graph;
mod plan;
mod policy;

use std::collections::HashMap;

use derive_builder::Builder;
use nvisy_core::Error;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::DiGraph;

pub use self::graph::{Graph, GraphEdge, GraphNode, GraphNodeKind};
pub(crate) use self::plan::{ExecutionPlan, ResolvedNode};
pub use self::policy::{BackoffStrategy, RetryPolicy, TimeoutBehavior, TimeoutPolicy};

/// Pipeline compiler with optional default policies.
///
/// Nodes that don't carry their own retry or timeout policy will inherit
/// the compiler-level defaults (if set) at compile time.
#[derive(Debug, Clone, Default, Builder)]
#[builder(
    name = "CompilerBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner")
)]
pub(crate) struct Compiler {
    /// Default retry policy applied to nodes without one.
    #[builder(default)]
    pub retry: Option<RetryPolicy>,
    /// Default timeout policy applied to nodes without one.
    #[builder(default)]
    pub timeout: Option<TimeoutPolicy>,
}

impl CompilerBuilder {
    /// Build the compiler.
    pub fn build(self) -> Result<Compiler, Error> {
        self.build_inner()
            .map_err(|e| Error::validation(e.to_string(), "compiler"))
    }
}

impl Compiler {
    /// Creates a compiler with no default policies.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a builder for configuring compiler defaults.
    pub fn builder() -> CompilerBuilder {
        CompilerBuilder::default()
    }

    /// Compiles a [`Graph`] into an [`ExecutionPlan`].
    ///
    /// Validates the graph, applies compiler-level default policies to nodes
    /// that don't specify their own, builds a petgraph representation,
    /// checks for cycles, and produces a topologically-sorted plan.
    pub fn compile(&self, graph: &Graph) -> Result<ExecutionPlan, Error> {
        let mut graph = graph.clone();

        for node in &mut graph.nodes {
            if node.retry.is_none() {
                node.retry.clone_from(&self.retry);
            }
            if node.timeout.is_none() {
                node.timeout.clone_from(&self.timeout);
            }
        }

        graph.validate()?;

        let pg = Self::build_petgraph(&graph)?;

        let topo = toposort(&pg, None)
            .map_err(|_| Error::validation("graph contains a cycle", "compiler"))?;

        let resolved = topo
            .iter()
            .map(|&idx| {
                let upstream_ids = pg
                    .neighbors_directed(idx, petgraph::Direction::Incoming)
                    .map(|n| pg[n].id)
                    .collect();
                let downstream_ids = pg
                    .neighbors_directed(idx, petgraph::Direction::Outgoing)
                    .map(|n| pg[n].id)
                    .collect();
                ResolvedNode {
                    node: pg[idx].clone(),
                    upstream_ids,
                    downstream_ids,
                }
            })
            .collect();

        Ok(ExecutionPlan { nodes: resolved })
    }

    /// Builds a petgraph `DiGraph` from a validated [`Graph`] and checks
    /// for cycles.
    fn build_petgraph(graph: &Graph) -> Result<DiGraph<GraphNode, GraphEdge>, Error> {
        let mut pg = DiGraph::with_capacity(graph.nodes.len(), graph.edges.len());
        let mut index_map = HashMap::with_capacity(graph.nodes.len());

        for node in &graph.nodes {
            let idx = pg.add_node(node.clone());
            index_map.insert(node.id, idx);
        }

        for edge in &graph.edges {
            let from = index_map[&edge.source];
            let to = index_map[&edge.target];
            pg.add_edge(from, to, edge.clone());
        }

        if is_cyclic_directed(&pg) {
            return Err(Error::validation("graph contains a cycle", "compiler"));
        }

        Ok(pg)
    }
}
