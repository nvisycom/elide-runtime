//! Shared test helpers and factories for integration tests.

use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
use nvisy_engine::graph::{ExportFile, Graph, GraphEdge, GraphNode, GraphNodeKind, ImportFile};
use nvisy_engine::pipeline::{Engine, EngineInput};
use nvisy_ontology::policy::Policies;
use uuid::Uuid;

/// Creates a temporary engine and returns it with the temp directory guard.
///
/// The directory is cleaned up when the guard is dropped.
pub fn engine() -> (Engine, tempfile::TempDir) {
    Engine::temp()
}

/// Returns a random actor ID.
pub fn actor() -> Uuid {
    Uuid::new_v4()
}

/// Creates a text content entry and uploads it to the engine.
///
/// Returns the content ID.
pub async fn upload_text(engine: &Engine, actor_id: Uuid, text: &str) -> Uuid {
    let data = ContentData::from_text(ContentSource::new(), text);
    let metadata = ContentMetadata::new().with_content_type("text/plain");
    let content = Content::with_metadata(data, metadata);
    engine
        .upload_content(actor_id, content)
        .await
        .expect("upload_text failed")
}

/// Builds a minimal import → export graph for the given content ID.
pub fn import_export_graph(content_id: Uuid) -> Graph {
    let import_id = Uuid::new_v4();
    let export_id = Uuid::new_v4();

    Graph::new(
        vec![
            GraphNode::new(
                import_id,
                GraphNodeKind::ImportFile(ImportFile {
                    content_ids: vec![content_id],
                    decompression: None,
                    decryption: None,
                }),
            ),
            GraphNode::new(
                export_id,
                GraphNodeKind::ExportFile(ExportFile {
                    content_ids: vec![],
                    encryption: None,
                    compression: None,
                }),
            ),
        ],
        vec![GraphEdge {
            source: import_id,
            target: export_id,
        }],
    )
}

/// Builds an [`EngineInput`] with the given graph and empty policies.
pub fn engine_input(actor_id: Uuid, graph: Graph) -> EngineInput {
    EngineInput {
        actor_id,
        policies: Policies::default(),
        graph,
        config: None,
        dry_run: false,
    }
}

/// Builds a dry-run [`EngineInput`] with the given graph and empty policies.
pub fn dry_run_input(actor_id: Uuid, graph: Graph) -> EngineInput {
    EngineInput {
        actor_id,
        policies: Policies::default(),
        graph,
        config: None,
        dry_run: true,
    }
}
