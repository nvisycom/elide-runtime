//! Execution extensions for ontology workflow types.
//!
//! The graph data types (nodes, edges, kinds, policies) are defined in
//! [`nvisy_ontology::workflow`]. This module adds:
//!
//! - [`GraphExt`]: petgraph conversion for topological sort / cycle detection.
//! - [`RetryExt`]: automatic retry with configurable backoff.
//! - [`TimeoutExt`]: wall-clock deadline enforcement for pipeline phases.

mod retry;
mod timeout;

use std::collections::HashMap;

use nvisy_ontology::workflow::{Graph, GraphEdge, GraphNode};
use petgraph::graph::DiGraph;

#[allow(unused_imports)] // wired when operations gain internal retry
pub(crate) use self::retry::RetryExt;
pub(crate) use self::timeout::TimeoutExt;

/// Extension trait adding petgraph conversion to [`Graph`].
pub(crate) trait GraphExt {
    fn to_petgraph(&self) -> DiGraph<GraphNode, GraphEdge>;
}

impl GraphExt for Graph {
    fn to_petgraph(&self) -> DiGraph<GraphNode, GraphEdge> {
        let mut pg = DiGraph::with_capacity(self.nodes.len(), self.edges.len());
        let mut index_map = HashMap::with_capacity(self.nodes.len());

        for node in &self.nodes {
            let idx = pg.add_node(node.clone());
            index_map.insert(node.id, idx);
        }

        for edge in &self.edges {
            let from = index_map[&edge.source];
            let to = index_map[&edge.target];
            pg.add_edge(from, to, edge.clone());
        }

        pg
    }
}
