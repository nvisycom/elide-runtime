//! Channel primitives used to wire data flow between pipeline nodes.
//!
//! [`EdgeChannel`] carries [`ContentData`] items along a graph edge, while
//! [`NodeSignal`] broadcasts node completion.

use tokio::sync::{mpsc, watch};
use nvisy_core::io::ContentData;

/// Default buffer size for bounded inter-node MPSC channels.
pub const CHANNEL_BUFFER_SIZE: usize = 256;

/// A bounded MPSC channel pair used to transfer [`ContentData`] items along a
/// single graph edge from an upstream node to a downstream node.
pub struct EdgeChannel {
    /// Sending half, held by the upstream node.
    pub sender: mpsc::Sender<ContentData>,
    /// Receiving half, held by the downstream node.
    pub receiver: mpsc::Receiver<ContentData>,
}

impl Default for EdgeChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgeChannel {
    /// Creates a new edge channel with [`CHANNEL_BUFFER_SIZE`] capacity.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        Self { sender, receiver }
    }
}

/// A watch channel pair used to signal that a node has completed execution.
///
/// The sender broadcasts `true` when the node finishes, and downstream nodes
/// wait on the receiver before starting.
pub struct NodeSignal {
    /// Sending half; set to `true` when the node completes.
    pub sender: watch::Sender<bool>,
    /// Receiving half; downstream tasks call `wait_for(|&done| done)`.
    pub receiver: watch::Receiver<bool>,
}

impl Default for NodeSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeSignal {
    /// Creates a new node signal initialized to `false` (not completed).
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(false);
        Self { sender, receiver }
    }
}
