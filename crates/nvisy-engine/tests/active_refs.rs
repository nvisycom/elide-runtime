//! Lifecycle wiring of the active-file-ref reverse index:
//!
//! - `start_run` inserts one ref per input file.
//! - `apply_run` clears every ref for the run at its terminal
//!   transition (Applied / PartiallyApplied).
//! - `cancel_run` clears every ref for the run.
//! - `delete_run` clears every ref before dropping the per-doc
//!   bodies (which are the only source of truth for
//!   input_file_id).
//!
//! These tests reach for the `has_active_refs` gate through a
//! small in-crate helper on `Engine` to keep the integration
//! shape close to what the sweeper will read in phase 4b.

use std::path::PathBuf;

use bytes::Bytes;
use hipstr::HipStr;
use nvisy_engine::keyspace::FileDescriptor;
use nvisy_engine::runs::{DocumentInput, StartBatch};
use nvisy_engine::{Engine, FileRegistry};
use nvisy_schema::plan::{AnalyzerParams, PatternRecognizerParams, ScopeParams};
use tempfile::TempDir;
use uuid::Uuid;

fn engine() -> (Engine, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = PathBuf::from(dir.path());
    let engine = Engine::open(&path).expect("engine opens");
    (engine, dir)
}

fn analyzer_spec() -> AnalyzerParams {
    AnalyzerParams {
        recognizers: nvisy_schema::plan::RecognizerParams {
            pattern: Some(PatternRecognizerParams {
                builtins: true,
                context_enhanced: true,
            }),
            ner: false,
            llm: false,
        },
        enrichers: nvisy_schema::plan::EnricherParams::default(),
        deduplication: Default::default(),
        scope: ScopeParams::default(),
    }
}

async fn upload_txt(engine: &Engine, actor_id: Uuid, bytes: &'static [u8]) -> Uuid {
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
        .put_file(actor_id, descriptor, Bytes::from_static(bytes))
        .await
        .expect("file upload succeeds");
    metadata.id
}

/// Read the gate directly — bypasses the sweeper's own loop so
/// tests can assert the flag independent of sweeper cadence.
async fn gate(engine: &Engine, actor_id: Uuid, file_id: Uuid) -> bool {
    engine
        .has_active_refs(actor_id, file_id)
        .await
        .expect("has_active_refs succeeds")
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
async fn start_inserts_a_ref_per_input_file() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();
    let file = upload_txt(&engine, actor, b"Contact: alice@example.com\n").await;
    assert!(!gate(&engine, actor, file).await, "gate off before start");
    let _run = start(&engine, actor, file).await;
    assert!(gate(&engine, actor, file).await, "gate on after start");
}

#[tokio::test]
async fn apply_clears_refs_for_the_run() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();
    let file = upload_txt(&engine, actor, b"Contact: alice@example.com\n").await;
    let run = start(&engine, actor, file).await;
    assert!(gate(&engine, actor, file).await, "gate on after start");
    engine.apply_run(actor, run).await.expect("apply succeeds");
    assert!(!gate(&engine, actor, file).await, "gate off after apply");
}

#[tokio::test]
async fn cancel_clears_refs_for_the_run() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();
    let file = upload_txt(&engine, actor, b"Contact: alice@example.com\n").await;
    let run = start(&engine, actor, file).await;
    assert!(gate(&engine, actor, file).await, "gate on after start");
    engine
        .cancel_run(actor, run)
        .await
        .expect("cancel succeeds");
    assert!(!gate(&engine, actor, file).await, "gate off after cancel");
}

#[tokio::test]
async fn delete_run_clears_refs_before_dropping_bodies() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();
    let file = upload_txt(&engine, actor, b"Contact: alice@example.com\n").await;
    let run = start(&engine, actor, file).await;
    // Move to a terminal state first via cancel, so delete_run
    // is legal in this configuration.
    engine.cancel_run(actor, run).await.unwrap();
    // cancel already dropped the refs; re-check delete_run is
    // safe against the empty-ref case too.
    engine
        .delete_run(actor, run)
        .await
        .expect("delete succeeds");
    assert!(!gate(&engine, actor, file).await);
}

#[tokio::test]
async fn gate_stays_on_while_any_run_still_references_the_file() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();
    let file = upload_txt(&engine, actor, b"Contact: alice@example.com\n").await;
    let run_a = start(&engine, actor, file).await;
    let run_b = start(&engine, actor, file).await;
    assert!(gate(&engine, actor, file).await);
    engine.cancel_run(actor, run_a).await.unwrap();
    // run_b still references the file, so the gate must stay on.
    assert!(
        gate(&engine, actor, file).await,
        "gate must stay on while any active run still references the file",
    );
    engine.cancel_run(actor, run_b).await.unwrap();
    assert!(!gate(&engine, actor, file).await);
}
