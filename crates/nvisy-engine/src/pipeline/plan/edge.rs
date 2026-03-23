//! Edge types for the compiled execution plan.

use uuid::Uuid;

/// Default buffer size for bounded MPSC channels between nodes.
pub(crate) const DEFAULT_CHANNEL_BUFFER: usize = 256;

/// Channel configuration for a resolved edge.
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// Buffer size for the bounded MPSC channel on this edge.
    pub channel_buffer: usize,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            channel_buffer: DEFAULT_CHANNEL_BUFFER,
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
