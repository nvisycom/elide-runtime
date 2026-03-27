//! Graph data model and execution policy implementations.
//!
//! The graph data types (nodes, edges, kinds, policies) are defined in
//! [`nvisy_ontology::graph`] and re-exported here. This module adds
//! async execution behavior for [`RetryPolicy`] and [`TimeoutPolicy`]
//! via extension traits, plus a petgraph conversion helper for the
//! plan compiler.

mod policy_ext;

pub use nvisy_ontology::graph::*;
pub(crate) use self::policy_ext::{RetryExt, TimeoutExt};

use std::collections::HashMap;

use petgraph::graph::DiGraph;

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
