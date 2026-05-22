//! Graph structural validation.

use std::collections::{HashMap, HashSet};

use nvisy_ontology::Error;
use uuid::Uuid;
use validator::Validate;

use super::{Graph, GraphNode, GraphNodeKind};

/// Maps node ID → edge count (in-degree or out-degree).
type DegreeMap = HashMap<Uuid, usize>;

impl Graph {
    /// Validates all structural invariants of the graph.
    #[must_use = "validation errors are silently ignored if the result is unused"]
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(ref concurrency) = self.concurrency {
            concurrency.validate()?;
        }
        let node_map = self.validate_nodes()?;
        let (in_degree, out_degree) = self.validate_edges(&node_map)?;
        self.validate_structure(&node_map, &in_degree, &out_degree)?;
        self.validate_dag(&node_map)?;
        Ok(())
    }

    /// Validates node-level invariants: non-empty graph, unique IDs,
    /// per-node retry/timeout policy validation, and action config validation.
    fn validate_nodes(&self) -> Result<HashMap<Uuid, &GraphNode>, Error> {
        if self.nodes.is_empty() {
            return Err(Error::new("graph must have at least one node"));
        }

        let mut node_map = HashMap::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if node_map.insert(node.id, node).is_some() {
                return Err(Error::new(format!("duplicate node id: {}", node.id)));
            }
        }

        for node in &self.nodes {
            if let Some(retry) = &node.retry {
                retry
                    .validate()
                    .map_err(|e| Error::new(format!("node {}: {e}", node.id)))?;
            }
            if let Some(timeout) = &node.timeout {
                timeout
                    .validate()
                    .map_err(|e| Error::new(format!("node {}: {e}", node.id)))?;
            }
            node.kind
                .validate()
                .map_err(|e| Error::new(format!("node {}: {}", node.id, e.message)))?;
        }

        Ok(node_map)
    }

    /// Validates edge invariants: no self-loops, no duplicates, all endpoints
    /// reference existing nodes, and edges respect pipeline phase ordering.
    fn validate_edges(
        &self,
        node_map: &HashMap<Uuid, &GraphNode>,
    ) -> Result<(DegreeMap, DegreeMap), Error> {
        let mut in_degree: DegreeMap = HashMap::new();
        let mut out_degree: DegreeMap = HashMap::new();
        let mut seen_edges = HashSet::with_capacity(self.edges.len());

        for edge in &self.edges {
            if edge.source == edge.target {
                return Err(Error::new(format!("self-loop on node {}", edge.source)));
            }

            if !seen_edges.insert((edge.source, edge.target)) {
                return Err(Error::new(format!(
                    "duplicate edge from {} to {}",
                    edge.source, edge.target
                )));
            }

            let source = node_map.get(&edge.source).ok_or_else(|| {
                Error::new(format!(
                    "edge references unknown source node: {}",
                    edge.source
                ))
            })?;
            let target = node_map.get(&edge.target).ok_or_else(|| {
                Error::new(format!(
                    "edge references unknown target node: {}",
                    edge.target
                ))
            })?;

            let source_phase = source.kind.phase();
            let target_phase = target.kind.phase();
            if source_phase > target_phase {
                return Err(Error::new(format!(
                    "edge from node {} (phase {source_phase}) to node {} \
                         (phase {target_phase}) violates pipeline ordering",
                    edge.source, edge.target,
                )));
            }

            *out_degree.entry(edge.source).or_default() += 1;
            *in_degree.entry(edge.target).or_default() += 1;
        }

        Ok((in_degree, out_degree))
    }

    /// Validates structural constraints: Import nodes have no incoming edges,
    /// Export nodes have no outgoing edges, and non-source/non-sink nodes
    /// are not isolated.
    fn validate_structure(
        &self,
        _node_map: &HashMap<Uuid, &GraphNode>,
        in_degree: &DegreeMap,
        out_degree: &DegreeMap,
    ) -> Result<(), Error> {
        for node in &self.nodes {
            let incoming = in_degree.get(&node.id).copied().unwrap_or(0);
            let outgoing = out_degree.get(&node.id).copied().unwrap_or(0);

            match &node.kind {
                GraphNodeKind::ImportFile(_) if incoming > 0 => {
                    return Err(Error::new(format!(
                        "import node {} must not have incoming edges",
                        node.id
                    )));
                }
                GraphNodeKind::ExportFile(_) if outgoing > 0 => {
                    return Err(Error::new(format!(
                        "export node {} must not have outgoing edges",
                        node.id
                    )));
                }
                _ => {}
            }

            let is_source = matches!(
                &node.kind,
                GraphNodeKind::ImportFile(_) | GraphNodeKind::LoadContext(_)
            );
            let is_sink = matches!(
                &node.kind,
                GraphNodeKind::ExportFile(_) | GraphNodeKind::SaveContext(_)
            );

            if !is_source && !is_sink && incoming == 0 && outgoing == 0 {
                return Err(Error::new(format!(
                    "node {} is isolated (no edges)",
                    node.id
                )));
            }
        }
        Ok(())
    }

    /// Validates that the graph is a proper DAG (no cycles).
    /// Uses DFS coloring: returns an error on the first cycle found.
    fn validate_dag(&self, node_map: &HashMap<Uuid, &GraphNode>) -> Result<(), Error> {
        let mut adjacency: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for edge in &self.edges {
            adjacency.entry(edge.source).or_default().push(edge.target);
        }

        #[derive(Clone, Copy, PartialEq)]
        enum State {
            Unvisited,
            InProgress,
            Done,
        }

        let mut state: HashMap<Uuid, State> =
            node_map.keys().map(|&id| (id, State::Unvisited)).collect();

        fn dfs(
            node: Uuid,
            adjacency: &HashMap<Uuid, Vec<Uuid>>,
            state: &mut HashMap<Uuid, State>,
        ) -> Result<(), Uuid> {
            state.insert(node, State::InProgress);
            if let Some(neighbors) = adjacency.get(&node) {
                for &next in neighbors {
                    match state.get(&next) {
                        Some(State::InProgress) => return Err(next),
                        Some(State::Unvisited) => dfs(next, adjacency, state)?,
                        _ => {}
                    }
                }
            }
            state.insert(node, State::Done);
            Ok(())
        }

        for &id in node_map.keys() {
            if state.get(&id) == Some(&State::Unvisited) {
                dfs(id, &adjacency, &mut state).map_err(|cycle_node| {
                    Error::new(format!(
                        "graph contains a cycle involving node {cycle_node}"
                    ))
                })?;
            }
        }

        Ok(())
    }
}
