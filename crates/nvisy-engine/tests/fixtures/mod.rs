//! Shared test helpers and factories for integration tests.

use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
use nvisy_engine::pipeline::{Engine, EngineInput};
use nvisy_ontology::workflow::{
    ExportFile, Graph, GraphEdge, GraphNode, GraphNodeKind, ImportFile,
};
use uuid::Uuid;

/// Creates a temporary [`Engine`] for testing.
pub fn engine() -> (Engine, tempfile::TempDir) {
    Engine::temp()
}

/// Returns a fixed actor UUID.
pub fn actor() -> Uuid {
    Uuid::parse_str("00000000-0000-7000-8000-000000000001").unwrap()
}

/// Returns a second fixed actor UUID for isolation tests.
pub fn other_actor() -> Uuid {
    Uuid::parse_str("00000000-0000-7000-8000-000000000002").unwrap()
}

/// Uploads text content and returns its content ID.
pub async fn upload_text(engine: &Engine, actor_id: Uuid, text: &str) -> Uuid {
    let data = ContentData::from_text(ContentSource::new(), text);
    let meta = ContentMetadata::new().with_content_type("text/plain");
    let content = Content::with_metadata(data, meta);
    let content = engine.registry().register_content(actor_id, content).await;
    content.unwrap().content_source().as_uuid()
}

/// Builds a simple import→export graph for the given content ID.
pub fn import_export_graph(content_id: Uuid) -> Graph {
    let import_id = Uuid::new_v4();
    let export_id = Uuid::new_v4();

    Graph {
        nodes: vec![
            GraphNode::new(
                import_id,
                GraphNodeKind::ImportFile(ImportFile {
                    content_ids: vec![content_id],
                    ..Default::default()
                }),
            ),
            GraphNode::new(
                export_id,
                GraphNodeKind::ExportFile(ExportFile {
                    content_ids: vec![Uuid::new_v4()],
                    ..Default::default()
                }),
            ),
        ],
        edges: vec![GraphEdge {
            source: import_id,
            target: export_id,
        }],
        concurrency: None,
    }
}

/// Builds an [`EngineInput`] with the given graph and no policies.
pub fn engine_input(actor_id: Uuid, graph: Graph) -> EngineInput {
    EngineInput {
        actor_id,
        policy_ids: Vec::new(),
        graph,
        config: None,
        dry_run: false,
    }
}

/// Builds a dry-run [`EngineInput`] with the given graph and no policies.
pub fn dry_run_input(actor_id: Uuid, graph: Graph) -> EngineInput {
    EngineInput {
        actor_id,
        policy_ids: Vec::new(),
        graph,
        config: None,
        dry_run: true,
    }
}
