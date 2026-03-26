//! Graph compilation and validation tests.

mod fixtures;

use nvisy_engine::graph::*;
use uuid::Uuid;

fn import_node(content_id: Uuid) -> GraphNode {
    GraphNode::new(
        Uuid::new_v4(),
        GraphNodeKind::ImportFile(ImportFile {
            content_ids: vec![content_id],
            decompression: None,
            decryption: None,
        }),
    )
}

fn export_node() -> GraphNode {
    GraphNode::new(
        Uuid::new_v4(),
        GraphNodeKind::ExportFile(ExportFile {
            content_ids: vec![],
            encryption: None,
            compression: None,
        }),
    )
}

#[tokio::test]
async fn valid_import_export_graph_accepted() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "compile test").await;

    let graph = fixtures::import_export_graph(content_id);
    let input = fixtures::engine_input(actor, graph);

    // run() succeeding means compilation passed (the pipeline may still
    // fail at execution, but that's a different concern).
    let _output = engine.run(input).await;
    Ok(())
}

#[tokio::test]
async fn empty_graph_rejected() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let graph = Graph::new(vec![], vec![]);
    let input = fixtures::engine_input(actor, graph);
    let result = engine.run(input).await;

    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn duplicate_node_ids_rejected() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "dup test").await;

    let shared_id = Uuid::new_v4();
    let node_a = GraphNode::new(
        shared_id,
        GraphNodeKind::ImportFile(ImportFile {
            content_ids: vec![content_id],
            decompression: None,
            decryption: None,
        }),
    );
    let node_b = GraphNode::new(
        shared_id,
        GraphNodeKind::ExportFile(ExportFile {
            content_ids: vec![],
            encryption: None,
            compression: None,
        }),
    );

    let graph = Graph::new(
        vec![node_a, node_b],
        vec![GraphEdge {
            source: shared_id,
            target: shared_id,
        }],
    );
    let input = fixtures::engine_input(actor, graph);
    assert!(engine.run(input).await.is_err());
    Ok(())
}

#[tokio::test]
async fn dangling_edge_rejected() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "dangle test").await;

    let node = import_node(content_id);
    let node_id = node.id;
    let phantom_id = Uuid::new_v4();

    let graph = Graph::new(
        vec![node],
        vec![GraphEdge {
            source: node_id,
            target: phantom_id,
        }],
    );
    let input = fixtures::engine_input(actor, graph);
    assert!(engine.run(input).await.is_err());
    Ok(())
}

#[tokio::test]
async fn self_loop_rejected() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "loop test").await;

    let node = import_node(content_id);
    let node_id = node.id;

    let graph = Graph::new(
        vec![node],
        vec![GraphEdge {
            source: node_id,
            target: node_id,
        }],
    );
    let input = fixtures::engine_input(actor, graph);
    assert!(engine.run(input).await.is_err());
    Ok(())
}

#[tokio::test]
async fn backward_edge_rejected() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "backward test").await;

    let import = import_node(content_id);
    let export = export_node();
    let import_id = import.id;
    let export_id = export.id;

    // Export (phase 6) → Import (phase 0) violates phase ordering.
    let graph = Graph::new(
        vec![import, export],
        vec![GraphEdge {
            source: export_id,
            target: import_id,
        }],
    );
    let input = fixtures::engine_input(actor, graph);
    assert!(engine.run(input).await.is_err());
    Ok(())
}

#[test]
fn concurrency_max_nodes_zero_rejected() {
    let policy = ConcurrencyPolicy { max_nodes: 0 };
    assert!(policy.validate().is_err());
}

#[test]
fn concurrency_max_nodes_one_accepted() {
    let policy = ConcurrencyPolicy { max_nodes: 1 };
    assert!(policy.validate().is_ok());
}
