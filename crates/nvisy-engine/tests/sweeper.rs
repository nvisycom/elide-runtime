//! Retention sweeper: per-tick behaviour + background loop.
//!
//! The sweeper's contract:
//! - Deletes files whose retention deadline has passed.
//! - Defers `OriginalContent` rows while an active run still
//!   references the file.
//! - Deletes `RedactedOutput` rows unconditionally.
//! - Skips `AuditLogs` rows until phase 5 wires the audit
//!   keyspace.
//! - Per-row errors don't abort the tick.

use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use hipstr::HipStr;
use jiff::{SignedDuration, Timestamp};
use nvisy_core::plan::{AnalyzerParams, PatternRecognizerParams, ScopeParams};
use nvisy_core::policy::redaction::{ModalityRedactions, TextRedaction};
use nvisy_core::policy::{
    Policy, Predicate, Retention, RetentionPolicy, RetentionScope, Rule, RuleAction,
};
use nvisy_engine::keyspace::FileDescriptor;
use nvisy_engine::runs::{DocumentInput, ResourceRef, StartBatch};
use nvisy_engine::{Engine, FileRegistry, PolicyRegistry};
use semver::Version;
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

/// Policy carrying a single retention rule on one scope. Rules
/// list holds one text-erase rule so `apply_run` has something
/// to do; without it apply would still succeed but be a no-op.
fn policy_with_retention(scope: RetentionScope, retention: Retention) -> Policy {
    Policy {
        id: Uuid::now_v7(),
        name: HipStr::from("test-policy"),
        version: Version::new(1, 0, 0),
        description: None,
        applies_when: None,
        labels: Vec::new(),
        rules: vec![Rule {
            id: Uuid::now_v7(),
            name: HipStr::from("erase email"),
            description: None,
            predicate: Predicate::LabelOneOf {
                labels: vec!["email_address".to_owned()],
            },
            action: RuleAction::Redact(ModalityRedactions {
                text: Some(TextRedaction::Erase),
                tabular: None,
                image: None,
                audio: None,
            }),
        }],
        fallback: None,
        retention: vec![RetentionPolicy { scope, retention }],
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
        .expect("file upload succeeds");
    metadata.id
}

async fn file_exists(engine: &Engine, actor_id: Uuid, file_id: Uuid) -> bool {
    engine.registry().get_file(actor_id, file_id).await.is_ok()
}

#[tokio::test]
async fn sweep_does_nothing_when_no_rows_are_due() {
    let (engine, _dir) = engine();
    let report = engine
        .sweep_once(Timestamp::now())
        .await
        .expect("sweep succeeds");
    assert_eq!(report.swept, 0);
    assert_eq!(report.deferred, 0);
    assert_eq!(report.errored, 0);
}

#[tokio::test]
async fn sweep_defers_original_content_while_run_is_active() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();

    // ZeroRetention → row's deadline == pinned_at, so it's
    // instantly due. But the run is in Analyzing/AwaitingReview
    // and its active-ref row must gate the sweep.
    let policy = policy_with_retention(RetentionScope::OriginalContent, Retention::ZeroRetention);
    engine.registry().put_policy(actor, &policy).await.unwrap();
    let file = upload_txt(&engine, actor).await;

    engine
        .start_run(
            actor,
            StartBatch {
                policy_refs: vec![ResourceRef {
                    id: policy.id,
                    version: policy.version.clone(),
                }],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id: file }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .expect("start succeeds");

    // Run is in AwaitingReview now; active-ref row exists.
    let report = engine.sweep_once(Timestamp::now()).await.unwrap();
    assert_eq!(report.swept, 0);
    assert_eq!(report.deferred, 1, "row must defer while gate is on");
    assert!(
        file_exists(&engine, actor, file).await,
        "file must survive while gate defers",
    );
    assert!(
        !engine
            .list_retention_for_file(actor, file)
            .await
            .unwrap()
            .is_empty(),
        "retention row must survive the deferred tick",
    );
}

#[tokio::test]
async fn sweep_deletes_original_content_after_run_terminates() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();

    let policy = policy_with_retention(RetentionScope::OriginalContent, Retention::ZeroRetention);
    engine.registry().put_policy(actor, &policy).await.unwrap();
    let file = upload_txt(&engine, actor).await;
    let run = engine
        .start_run(
            actor,
            StartBatch {
                policy_refs: vec![ResourceRef {
                    id: policy.id,
                    version: policy.version.clone(),
                }],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id: file }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .unwrap();

    // Terminal transition drops the active-ref row.
    engine.apply_run(actor, run).await.unwrap();

    let report = engine.sweep_once(Timestamp::now()).await.unwrap();
    assert_eq!(report.swept, 1, "row must sweep once the gate clears");
    assert!(
        !file_exists(&engine, actor, file).await,
        "file must be deleted after the sweeper acts",
    );
    assert!(
        engine
            .list_retention_for_file(actor, file)
            .await
            .unwrap()
            .is_empty(),
        "retention row must be gone after sweep",
    );
}

#[tokio::test]
async fn sweep_deletes_redacted_output_without_gate_check() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();

    // Set only RedactedOutput retention on the policy — no gate
    // applies to output files (no active run ever reads them).
    let policy = policy_with_retention(RetentionScope::RedactedOutput, Retention::ZeroRetention);
    engine.registry().put_policy(actor, &policy).await.unwrap();
    let file = upload_txt(&engine, actor).await;
    let run = engine
        .start_run(
            actor,
            StartBatch {
                policy_refs: vec![ResourceRef {
                    id: policy.id,
                    version: policy.version.clone(),
                }],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id: file }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .unwrap();
    engine.apply_run(actor, run).await.unwrap();

    // Grab the output file id from the per-doc row so we can
    // assert its deletion.
    let doc_id = engine.get_run(actor, run).await.unwrap().document_ids[0];
    let output_file = engine
        .get_run_doc(actor, run, doc_id)
        .await
        .unwrap()
        .output_file_id
        .expect("apply produced an output file");

    let report = engine.sweep_once(Timestamp::now()).await.unwrap();
    assert_eq!(report.swept, 1);
    assert!(
        !file_exists(&engine, actor, output_file).await,
        "output file must be deleted",
    );
    // The input file remains — no retention on OriginalContent
    // in this policy.
    assert!(
        file_exists(&engine, actor, file).await,
        "input file must survive when no OriginalContent rule pinned it",
    );
}

#[tokio::test]
async fn sweep_ignores_rows_whose_deadline_has_not_arrived() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();

    let policy = policy_with_retention(
        RetentionScope::RedactedOutput,
        Retention::Duration { days: 30 },
    );
    engine.registry().put_policy(actor, &policy).await.unwrap();
    let file = upload_txt(&engine, actor).await;
    let run = engine
        .start_run(
            actor,
            StartBatch {
                policy_refs: vec![ResourceRef {
                    id: policy.id,
                    version: policy.version.clone(),
                }],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id: file }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .unwrap();
    engine.apply_run(actor, run).await.unwrap();

    // Sweep at "now" — the 30-day deadline is far off, nothing
    // is due.
    let report = engine.sweep_once(Timestamp::now()).await.unwrap();
    assert_eq!(report.swept, 0);
    assert_eq!(report.deferred, 0);

    // Skip ahead 31 days — now the row is due.
    let future = Timestamp::now()
        .checked_add(SignedDuration::from_secs(31 * 24 * 60 * 60))
        .unwrap();
    let report = engine.sweep_once(future).await.unwrap();
    assert_eq!(report.swept, 1);
}

#[tokio::test]
async fn background_loop_sweeps_and_stops_cleanly() {
    let (engine, _dir) = engine();
    let actor = Uuid::now_v7();

    let policy = policy_with_retention(RetentionScope::RedactedOutput, Retention::ZeroRetention);
    engine.registry().put_policy(actor, &policy).await.unwrap();
    let file = upload_txt(&engine, actor).await;
    let run = engine
        .start_run(
            actor,
            StartBatch {
                policy_refs: vec![ResourceRef {
                    id: policy.id,
                    version: policy.version.clone(),
                }],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id: file }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .unwrap();
    engine.apply_run(actor, run).await.unwrap();

    let doc_id = engine.get_run(actor, run).await.unwrap().document_ids[0];
    let output_file = engine
        .get_run_doc(actor, run, doc_id)
        .await
        .unwrap()
        .output_file_id
        .unwrap();

    // Start the background loop with a tight tick; wait long
    // enough that at least one tick fires and processes the row.
    let handle = engine.start_sweeper(Duration::from_millis(50));
    let mut swept = false;
    for _ in 0..40 {
        if !file_exists(&engine, actor, output_file).await {
            swept = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    handle.stop().await;
    assert!(swept, "background loop must have swept the output file");
}
