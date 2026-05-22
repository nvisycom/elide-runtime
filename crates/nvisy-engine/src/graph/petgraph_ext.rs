//! Petgraph conversion for [`Graph`].

use std::collections::HashMap;

use petgraph::graph::DiGraph;

use crate::workflow::{Graph, GraphEdge, GraphNode};

/// Extension trait adding petgraph conversion to [`Graph`].
pub(crate) trait GraphExt {
    /// Build a directed petgraph from the graph's nodes and edges.
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
