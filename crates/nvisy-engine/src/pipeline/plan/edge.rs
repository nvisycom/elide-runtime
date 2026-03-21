//! Edge types for the compiled execution plan.

use uuid::Uuid;

/// Channel configuration for a resolved edge.
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// Buffer size for the bounded MPSC channel on this edge.
    pub channel_buffer: usize,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            channel_buffer: 256,
        }
    }
}

/// A directed edge with pre-computed channel configuration.
#[derive(Debug, Clone)]
pub struct ResolvedEdge {
    /// ID of the upstream node.
    pub source: Uuid,
    /// ID of the downstream node.
    pub target: Uuid,
    /// Channel configuration for this edge.
    pub config: EdgeConfig,
}
