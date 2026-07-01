//! Startup reap: [`Engine::reap_orphan_active_refs`] drops
//! active-file-reference rows whose run is either missing or
//! terminal.
//!
//! Boot-path callers (`ServiceRuntime::new`) run this before
//! starting the sweeper so the gate reflects real active runs,
//! not ghosts left behind by a crash.

use std::path::PathBuf;

use bytes::Bytes;
use hipstr::HipStr;
use nvisy_schema::plan::{AnalyzerParams, PatternRecognizerParams, ScopeParams};
use nvisy_engine::keyspace::FileDescriptor;
use nvisy_engine::runs::{DocumentInput, StartBatch};
use nvisy_engine::{Engine, FileRegistry};
use tempfile::TempDir;
use uuid::Uuid;

fn engine() -> (Engine, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = Engine::open(&PathBuf::from(dir.path())).expect("engine opens");
    (engine, dir)
}

fn analyzer_spec() -> AnalyzerParams {
    AnalyzerParams {
        recognizers: nvisy_schema::plan::RecognizerParams {
            pattern: Some(PatternRecognizerParams {
                builtins: true,
                context_enhanced: true,
            }),
            ner: Vec::new(),
            llm: false,
        },
        enrichers: nvisy_schema::plan::EnricherParams::default(),
        deduplication: Default::default(),
        scope: ScopeParams::default(),
    }
}

async fn upload_txt(engine: &Engine, actor_id: Uuid) -> Uuid {
    let descriptor = FileDescriptor {
        filename: Some(HipStr::from("sample.txt")),
        content_type: Some(HipStr::from("text/plain")),
        extension: HipStr::from("txt"),
        lineage: None,
        descriptor_labels: Vec::new(),
        descriptor_metadata: Default::default(),
    };
    let metadata = engine
        .registry()
        .put_file(
            actor_id,
            descriptor,
            Bytes::from_static(b"alice@example.com"),
        )
        .await
        .expect("upload succeeds");
    metadata.id
}

async fn start(engine: &Engine, actor_id: Uuid, file_id: Uuid) -> Uuid {
    engine
        .start_run(
            actor_id,
            StartBatch {
                policy_refs: Vec::new(),
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .expect("start succeeds")
}

#[tokio::test]
async fn reap_leaves_active_run_refs_in_place() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();
    let file = upload_txt(&engine, actor).await;
    let _run = start(&engine, actor, file).await;
    assert!(engine.has_active_refs(actor, file).await.unwrap());

    let reaped = engine.reap_orphan_active_refs().await.unwrap();
    assert_eq!(reaped, 0, "active run's ref must survive reap");
    assert!(
        engine.has_active_refs(actor, file).await.unwrap(),
        "gate must still trip after the reap",
    );
}
