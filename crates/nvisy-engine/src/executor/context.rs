use tokio::sync::{mpsc, watch};
use nvisy_core::data::DataValue;

/// Buffer size for inter-node channels.
pub const CHANNEL_BUFFER_SIZE: usize = 256;

/// Wiring for a single edge: sender + receiver pair.
pub struct EdgeChannel {
    pub sender: mpsc::Sender<DataValue>,
    pub receiver: mpsc::Receiver<DataValue>,
}

impl Default for EdgeChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgeChannel {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        Self { sender, receiver }
    }
}

/// Signals that a node has completed.
pub struct NodeSignal {
    pub sender: watch::Sender<bool>,
    pub receiver: watch::Receiver<bool>,
}

impl Default for NodeSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeSignal {
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(false);
        Self { sender, receiver }
    }
}
