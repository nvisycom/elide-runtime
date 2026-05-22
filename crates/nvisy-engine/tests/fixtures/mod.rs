//! Shared test helpers and factories for integration tests.
//!
//! Each integration test compiles this module separately, so some helpers
//! may appear unused from any given test file's perspective.
#![allow(dead_code)]

use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
use nvisy_engine::ingestion::{ExportFile, ImportFile};
use nvisy_engine::pipeline::{Engine, EngineInput};
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

/// Builds an `EngineInput` with one import + one export for the given content.
pub fn engine_input(actor_id: Uuid, content_id: Uuid) -> EngineInput {
    base_input(actor_id, content_id, false)
}

/// Same as [`engine_input`] but with `dry_run = true`.
pub fn dry_run_input(actor_id: Uuid, content_id: Uuid) -> EngineInput {
    base_input(actor_id, content_id, true)
}

/// `EngineInput` that imports a non-existent content ID. The run
/// will fail at import time — useful for testing failure paths
/// (failed run is still recorded, etc.).
pub fn failing_input(actor_id: Uuid) -> EngineInput {
    base_input(actor_id, Uuid::nil(), false)
}

fn base_input(actor_id: Uuid, content_id: Uuid, dry_run: bool) -> EngineInput {
    EngineInput {
        actor_id,
        policies: Vec::new(),
        config: None,
        dry_run,
        imports: vec![ImportFile {
            content_ids: vec![content_id],
            ..Default::default()
        }],
        context_ids: Vec::new(),
        extraction: Default::default(),
        detection: Default::default(),
        deduplication: Default::default(),
        redaction: Default::default(),
        validation: Default::default(),
        exports: vec![ExportFile {
            content_ids: vec![Uuid::new_v4()],
            ..Default::default()
        }],
    }
}
