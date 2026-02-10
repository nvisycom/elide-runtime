use crate::schema::Graph;
use nvisy_core::errors::NvisyError;

/// Parse a graph from a JSON value.
pub fn parse_graph(value: &serde_json::Value) -> Result<Graph, NvisyError> {
    let graph: Graph = serde_json::from_value(value.clone()).map_err(|e| {
        NvisyError::validation(format!("Invalid graph definition: {}", e), "compiler")
    })?;

    // Validate: must have at least one node
    if graph.nodes.is_empty() {
        return Err(NvisyError::validation("Graph must have at least one node", "compiler"));
    }

    // Validate: no duplicate node IDs
    let mut seen = std::collections::HashSet::new();
    for node in &graph.nodes {
        if !seen.insert(node.id()) {
            return Err(NvisyError::validation(
                format!("Duplicate node ID: {}", node.id()),
                "compiler",
            ));
        }
    }

    // Validate: all edge endpoints reference existing nodes
    let node_ids: std::collections::HashSet<&str> = graph.nodes.iter().map(|n| n.id()).collect();
    for edge in &graph.edges {
        if !node_ids.contains(edge.from.as_str()) {
            return Err(NvisyError::validation(
                format!("Edge references unknown source node: {}", edge.from),
                "compiler",
            ));
        }
        if !node_ids.contains(edge.to.as_str()) {
            return Err(NvisyError::validation(
                format!("Edge references unknown target node: {}", edge.to),
                "compiler",
            ));
        }
    }

    Ok(graph)
}
