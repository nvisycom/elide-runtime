//! Phase grouping for the compiled execution plan.

/// A group of node indices that share the same pipeline phase.
#[derive(Debug, Clone)]
pub struct PhaseGroup {
    /// The pipeline phase number (0–5).
    pub phase: u8,
    /// Indices into `ExecutionPlan::nodes()` for nodes in this phase.
    pub node_indices: Vec<usize>,
}
