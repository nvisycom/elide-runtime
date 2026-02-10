use std::collections::HashMap;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use crate::schema::{Graph, GraphNode};
use nvisy_core::errors::NvisyError;
use nvisy_core::registry::Registry;

/// A node resolved against the registry.
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    pub node: GraphNode,
    pub topo_order: usize,
    pub upstream_ids: Vec<String>,
    pub downstream_ids: Vec<String>,
}

/// A compiled execution plan ready for the executor.
pub struct ExecutionPlan {
    pub nodes: Vec<ResolvedNode>,
    pub topo_order: Vec<String>,
}

/// Build an execution plan from a parsed graph and registry.
pub fn build_plan(graph: &Graph, registry: &Registry) -> Result<ExecutionPlan, NvisyError> {
    // Build petgraph
    let mut pg: DiGraph<&str, ()> = DiGraph::new();
    let mut index_map: HashMap<&str, NodeIndex> = HashMap::new();

    for node in &graph.nodes {
        let idx = pg.add_node(node.id());
        index_map.insert(node.id(), idx);
    }

    for edge in &graph.edges {
        let from = index_map.get(edge.from.as_str()).ok_or_else(|| {
            NvisyError::validation(format!("Unknown edge source: {}", edge.from), "compiler")
        })?;
        let to = index_map.get(edge.to.as_str()).ok_or_else(|| {
            NvisyError::validation(format!("Unknown edge target: {}", edge.to), "compiler")
        })?;
        pg.add_edge(*from, *to, ());
    }

    // Cycle detection
    if is_cyclic_directed(&pg) {
        return Err(NvisyError::validation("Graph contains a cycle", "compiler"));
    }

    // Topological sort
    let topo = toposort(&pg, None).map_err(|_| {
        NvisyError::validation("Graph contains a cycle", "compiler")
    })?;

    let topo_order: Vec<String> = topo.iter().map(|idx| pg[*idx].to_string()).collect();

    // Resolve nodes against registry
    for node in &graph.nodes {
        match node {
            GraphNode::Action { action, params, .. } => {
                let _a = registry.get_action(action).ok_or_else(|| {
                    NvisyError::validation(format!("Unknown action: {}", action), "compiler")
                })?;
                _a.validate_params(params)?;
            }
            GraphNode::Source { provider, stream, .. } => {
                let source_key = format!("{}/{}", provider, stream);
                let _s = registry.get_source(&source_key).ok_or_else(|| {
                    NvisyError::validation(format!("Unknown source: {}", source_key), "compiler")
                })?;
            }
            GraphNode::Target { provider, stream, .. } => {
                let target_key = format!("{}/{}", provider, stream);
                let _t = registry.get_target(&target_key).ok_or_else(|| {
                    NvisyError::validation(format!("Unknown target: {}", target_key), "compiler")
                })?;
            }
        }
    }

    // Build resolved nodes with adjacency info
    let node_map: HashMap<&str, &GraphNode> = graph.nodes.iter().map(|n| (n.id(), n)).collect();
    let mut resolved = Vec::new();

    for (order, node_id) in topo_order.iter().enumerate() {
        let node = node_map[node_id.as_str()];
        let idx = index_map[node_id.as_str()];

        let upstream_ids: Vec<String> = pg
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|n| pg[n].to_string())
            .collect();

        let downstream_ids: Vec<String> = pg
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|n| pg[n].to_string())
            .collect();

        resolved.push(ResolvedNode {
            node: node.clone(),
            topo_order: order,
            upstream_ids,
            downstream_ids,
        });
    }

    Ok(ExecutionPlan {
        nodes: resolved,
        topo_order,
    })
}
