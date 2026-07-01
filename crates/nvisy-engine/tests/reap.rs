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
use nvisy_core::plan::{AnalyzerParams, PatternRecognizerParams, ScopeParams};
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
        recognizers: nvisy_core::plan::RecognizerParams {
            pattern: Some(PatternRecognizerParams {
                builtins: true,
                context_enhanced: true,
            }),
            ner: Vec::new(),
            llm: Vec::new(),
        },
        enrichers: nvisy_core::plan::EnricherParams::default(),
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

#[tokio::test]
async fn reap_is_no_op_after_clean_lifecycle() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();
    let file = upload_txt(&engine, actor).await;
    let run = start(&engine, actor, file).await;
    engine.cancel_run(actor, run).await.unwrap();
    engine.delete_run(actor, run).await.unwrap();
    assert!(!engine.has_active_refs(actor, file).await.unwrap());
    let reaped = engine.reap_orphan_active_refs().await.unwrap();
    assert_eq!(reaped, 0, "no orphans after a clean lifecycle");
}

#[tokio::test]
async fn reap_survives_multi_actor_state() {
    let (engine, _dir) = engine();
    let actor_a = Uuid::now_v7();
    let actor_b = Uuid::now_v7();
    let file_a = upload_txt(&engine, actor_a).await;
    let file_b = upload_txt(&engine, actor_b).await;
    let _run_a = start(&engine, actor_a, file_a).await;
    let _run_b = start(&engine, actor_b, file_b).await;

    let reaped = engine.reap_orphan_active_refs().await.unwrap();
    assert_eq!(reaped, 0);
    assert!(engine.has_active_refs(actor_a, file_a).await.unwrap());
    assert!(engine.has_active_refs(actor_b, file_b).await.unwrap());
}
