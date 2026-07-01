//! End-to-end at the engine layer: a run that references a
//! policy with retention rules stamps the expected rows in the
//! retention schedule.
//!
//! - `Engine::start_run` pins one `OriginalContent` row per
//!   input file with the strictest resolved retention across
//!   the run's policies.
//! - `Engine::apply_run` pins one `RedactedOutput` row per
//!   output file after the redacted bytes land.
//! - Policies whose retention is `Indefinite` (or absent) write
//!   no row; the sweeper has nothing to scan for
//!   never-deleted artifacts.

use std::path::PathBuf;

use bytes::Bytes;
use hipstr::HipStr;
use jiff::{SignedDuration, Timestamp};
use nvisy_schema::plan::{AnalyzerParams, PatternRecognizerParams, ScopeParams};
use nvisy_schema::policy::redaction::{ModalityRedactions, TextRedaction};
use nvisy_schema::policy::{
    Policy, Predicate, Retention, RetentionPolicy, RetentionScope, Rule, RuleAction,
};
use nvisy_engine::keyspace::FileDescriptor;
use nvisy_engine::retention::RetentionRecord;
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

/// Policy carrying a text-body erase rule plus two retention
/// rules: bounded original content, zero redacted output.
fn erase_email_with_retention() -> Policy {
    Policy {
        id: Uuid::now_v7(),
        name: HipStr::from("erase-with-retention"),
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
        retention: vec![
            RetentionPolicy {
                scope: RetentionScope::OriginalContent,
                retention: Retention::Duration { days: 30 },
            },
            RetentionPolicy {
                scope: RetentionScope::RedactedOutput,
                retention: Retention::ZeroRetention,
            },
        ],
    }
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

/// Upload one small text file. Returns the file id.
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

fn find_scope(rows: &[RetentionRecord], scope: RetentionScope) -> Option<&RetentionRecord> {
    rows.iter().find(|r| r.scope == scope)
}

#[tokio::test]
async fn start_pins_original_content_retention_per_input_file() {
    let (engine, _dir) = engine();
    let actor_id = Uuid::now_v7();

    let policy = erase_email_with_retention();
    let policy_ref = ResourceRef {
        id: policy.id,
        version: policy.version.clone(),
    };
    engine
        .registry()
        .put_policy(actor_id, &policy)
        .await
        .expect("policy upload");

    let file_id = upload_txt(&engine, actor_id, b"Contact: alice@example.com\n").await;

    let before = Timestamp::now();
    let run_id = engine
        .start_run(
            actor_id,
            StartBatch {
                policy_refs: vec![policy_ref],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .expect("run starts");
    let after = Timestamp::now();

    let rows = engine
        .list_retention_for_file(actor_id, file_id)
        .await
        .expect("read retention rows");
    let original = find_scope(&rows, RetentionScope::OriginalContent)
        .expect("OriginalContent row pinned at start");
    assert_eq!(original.source_run_id, run_id);
    // The policy set Duration { days: 30 }. Deadline should be
    // ~30 days from the pinned_at instant. Allow the window
    // between `before` and `after` as slack.
    let thirty_days = SignedDuration::from_secs(30 * 24 * 60 * 60);
    assert!(original.deadline >= before.checked_add(thirty_days).unwrap());
    assert!(original.deadline <= after.checked_add(thirty_days).unwrap());
    // No RedactedOutput row exists on the input file — that
    // scope pins the *output* artifact, not the input.
    assert!(find_scope(&rows, RetentionScope::RedactedOutput).is_none());
}

#[tokio::test]
async fn apply_pins_redacted_output_retention_per_output_file() {
    let (engine, _dir) = engine();
    let actor_id = Uuid::now_v7();

    let policy = erase_email_with_retention();
    let policy_ref = ResourceRef {
        id: policy.id,
        version: policy.version.clone(),
    };
    engine
        .registry()
        .put_policy(actor_id, &policy)
        .await
        .unwrap();

    let file_id = upload_txt(&engine, actor_id, b"Contact: alice@example.com\n").await;

    let run_id = engine
        .start_run(
            actor_id,
            StartBatch {
                policy_refs: vec![policy_ref],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .unwrap();

    let before = Timestamp::now();
    engine
        .apply_run(actor_id, run_id)
        .await
        .expect("apply succeeds");
    let after = Timestamp::now();

    // Find the output file id from the per-doc row.
    let doc_id = engine.get_run(actor_id, run_id).await.unwrap().document_ids[0];
    let doc = engine.get_run_doc(actor_id, run_id, doc_id).await.unwrap();
    let output_file_id = doc.output_file_id.expect("apply produced an output file");

    let rows = engine
        .list_retention_for_file(actor_id, output_file_id)
        .await
        .unwrap();
    let redacted = find_scope(&rows, RetentionScope::RedactedOutput)
        .expect("RedactedOutput row pinned after apply");
    assert_eq!(redacted.source_run_id, run_id);
    // ZeroRetention → deadline == pinned_at (i.e. within the
    // window of the apply call).
    assert!(redacted.deadline >= before);
    assert!(redacted.deadline <= after);
    // OriginalContent from the original policy governs the
    // *input*, not the output — no row here.
    assert!(find_scope(&rows, RetentionScope::OriginalContent).is_none());
}

#[tokio::test]
async fn indefinite_retention_writes_no_row() {
    let (engine, _dir) = engine();
    let actor_id = Uuid::now_v7();

    // Same shape but all-indefinite retention → no rows expected.
    let policy = Policy {
        retention: vec![
            RetentionPolicy {
                scope: RetentionScope::OriginalContent,
                retention: Retention::Indefinite,
            },
            RetentionPolicy {
                scope: RetentionScope::RedactedOutput,
                retention: Retention::Indefinite,
            },
        ],
        ..erase_email_with_retention()
    };
    let policy_ref = ResourceRef {
        id: policy.id,
        version: policy.version.clone(),
    };
    engine
        .registry()
        .put_policy(actor_id, &policy)
        .await
        .unwrap();

    let file_id = upload_txt(&engine, actor_id, b"Contact: alice@example.com\n").await;

    let run_id = engine
        .start_run(
            actor_id,
            StartBatch {
                policy_refs: vec![policy_ref],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .unwrap();
    engine.apply_run(actor_id, run_id).await.unwrap();

    let doc_id = engine.get_run(actor_id, run_id).await.unwrap().document_ids[0];
    let doc = engine.get_run_doc(actor_id, run_id, doc_id).await.unwrap();
    let output_file_id = doc.output_file_id.unwrap();

    let input_rows = engine
        .list_retention_for_file(actor_id, file_id)
        .await
        .unwrap();
    let output_rows = engine
        .list_retention_for_file(actor_id, output_file_id)
        .await
        .unwrap();
    assert!(
        input_rows.is_empty(),
        "Indefinite OriginalContent must not write a row; got {input_rows:?}",
    );
    assert!(
        output_rows.is_empty(),
        "Indefinite RedactedOutput must not write a row; got {output_rows:?}",
    );
}

#[tokio::test]
async fn strictest_wins_across_multiple_policies() {
    let (engine, _dir) = engine();
    let actor_id = Uuid::now_v7();

    // Two policies both cover OriginalContent — one asks for
    // 90 days, one asks for 7 days. The pinned row must reflect
    // the stricter of the two (7 days).
    let lax = Policy {
        retention: vec![RetentionPolicy {
            scope: RetentionScope::OriginalContent,
            retention: Retention::Duration { days: 90 },
        }],
        ..erase_email_with_retention()
    };
    let strict = Policy {
        id: Uuid::now_v7(),
        retention: vec![RetentionPolicy {
            scope: RetentionScope::OriginalContent,
            retention: Retention::Duration { days: 7 },
        }],
        ..erase_email_with_retention()
    };
    engine.registry().put_policy(actor_id, &lax).await.unwrap();
    engine
        .registry()
        .put_policy(actor_id, &strict)
        .await
        .unwrap();

    let file_id = upload_txt(&engine, actor_id, b"Contact: alice@example.com\n").await;

    let before = Timestamp::now();
    let _run_id = engine
        .start_run(
            actor_id,
            StartBatch {
                policy_refs: vec![
                    ResourceRef {
                        id: lax.id,
                        version: lax.version.clone(),
                    },
                    ResourceRef {
                        id: strict.id,
                        version: strict.version.clone(),
                    },
                ],
                context_refs: Vec::new(),
                documents: vec![DocumentInput { file_id }],
                metadata: Default::default(),
                analyzer: analyzer_spec(),
                concurrency: Some(1),
            },
        )
        .await
        .unwrap();
    let after = Timestamp::now();

    let rows = engine
        .list_retention_for_file(actor_id, file_id)
        .await
        .unwrap();
    let original =
        find_scope(&rows, RetentionScope::OriginalContent).expect("OriginalContent row present");
    let seven_days = SignedDuration::from_secs(7 * 24 * 60 * 60);
    assert!(original.deadline >= before.checked_add(seven_days).unwrap());
    assert!(original.deadline <= after.checked_add(seven_days).unwrap());
}
