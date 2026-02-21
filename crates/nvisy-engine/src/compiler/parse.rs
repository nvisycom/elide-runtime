//! JSON parsing and validation for pipeline graph definitions.
//!
//! Deserializes a [`serde_json::Value`] into a [`Graph`] and validates
//! structural invariants (non-empty, unique IDs, valid edge references).

use crate::compiler::graph::Graph;
use nvisy_core::Error;

/// Parses and validates a [`Graph`] from a JSON value.
///
/// Performs the following validations:
/// - The graph must contain at least one node.
/// - All node IDs must be unique.
/// - All edge endpoints must reference existing node IDs.
pub fn parse_graph(value: &serde_json::Value) -> Result<Graph, Error> {
    let graph: Graph = serde_json::from_value(value.clone()).map_err(|e| {
        Error::validation(format!("Invalid graph definition: {}", e), "compiler")
    })?;

    // Validate: must have at least one node
    if graph.nodes.is_empty() {
        return Err(Error::validation("Graph must have at least one node", "compiler"));
    }

    // Validate: no duplicate node IDs
    let mut seen = std::collections::HashSet::new();
    for node in &graph.nodes {
        if !seen.insert(node.id()) {
            return Err(Error::validation(
                format!("Duplicate node ID: {}", node.id()),
                "compiler",
            ));
        }
    }

    // Validate: all edge endpoints reference existing nodes
    let node_ids: std::collections::HashSet<&str> = graph.nodes.iter().map(|n| n.id()).collect();
    for edge in &graph.edges {
        if !node_ids.contains(edge.from.as_str()) {
            return Err(Error::validation(
                format!("Edge references unknown source node: {}", edge.from),
                "compiler",
            ));
        }
        if !node_ids.contains(edge.to.as_str()) {
            return Err(Error::validation(
                format!("Edge references unknown target node: {}", edge.to),
                "compiler",
            ));
        }
    }

    Ok(graph)
}
