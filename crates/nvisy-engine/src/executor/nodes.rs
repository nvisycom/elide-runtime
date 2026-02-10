use std::any::Any;
use tokio::sync::mpsc;
use nvisy_core::data::DataValue;
use nvisy_core::errors::NvisyError;
use nvisy_core::registry::Registry;
use crate::schema::GraphNode;

/// Execute a source node: read from external system into output channel.
pub async fn execute_source(
    node: &GraphNode,
    output: mpsc::Sender<DataValue>,
    registry: &Registry,
    client: Box<dyn Any + Send>,
) -> Result<u64, NvisyError> {
    match node {
        GraphNode::Source { provider, stream, params, .. } => {
            let source_key = format!("{}/{}", provider, stream);
            let source = registry.get_source(&source_key).ok_or_else(|| {
                NvisyError::runtime(format!("Source not found: {}", source_key), "executor", false)
            })?;
            source.read(output, params.clone(), client).await
        }
        _ => Err(NvisyError::runtime("Expected source node", "executor", false)),
    }
}

/// Execute an action node: consume from input, produce to output.
pub async fn execute_action(
    node: &GraphNode,
    input: mpsc::Receiver<DataValue>,
    output: mpsc::Sender<DataValue>,
    registry: &Registry,
    client: Option<Box<dyn Any + Send>>,
) -> Result<u64, NvisyError> {
    match node {
        GraphNode::Action { action, params, .. } => {
            let act = registry.get_action(action).ok_or_else(|| {
                NvisyError::runtime(format!("Action not found: {}", action), "executor", false)
            })?;
            act.execute(input, output, params.clone(), client).await
        }
        _ => Err(NvisyError::runtime("Expected action node", "executor", false)),
    }
}

/// Execute a target node: consume from input, write to external system.
pub async fn execute_target(
    node: &GraphNode,
    input: mpsc::Receiver<DataValue>,
    registry: &Registry,
    client: Box<dyn Any + Send>,
) -> Result<u64, NvisyError> {
    match node {
        GraphNode::Target { provider, stream, params, .. } => {
            let target_key = format!("{}/{}", provider, stream);
            let target = registry.get_target(&target_key).ok_or_else(|| {
                NvisyError::runtime(format!("Target not found: {}", target_key), "executor", false)
            })?;
            target.write(input, params.clone(), client).await
        }
        _ => Err(NvisyError::runtime("Expected target node", "executor", false)),
    }
}
