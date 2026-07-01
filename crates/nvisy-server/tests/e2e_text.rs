//! Server-side end-to-end over a plain `.txt`: upload, detect,
//! override one entity, apply, download. Asserts the targeted
//! literal disappeared from the redacted output.

mod fixtures;

use std::time::Duration;

use axum_test::TestServer;
use axum_test::http::HeaderName;
use nvisy_core::plan::AnalyzerParams;
use nvisy_server::{ServiceRuntime, routes};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

use self::fixtures::write_artefact;

const ACTOR_HEADER: &str = "x-actor-id";
const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

/// Spin up a fresh server over a temp data dir. Returns the
/// running `TestServer`, the actor id every request will assert,
/// and the temp dir guard (drop it last to keep the dir alive).
async fn server() -> (TestServer, Uuid, ServiceRuntime, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let runtime =
        ServiceRuntime::new(dir.path().to_path_buf(), AnalyzerParams::default(), None)
            .await
            .expect("service runtime");
    let router = routes().with_state(runtime.state());
    let server = TestServer::new(router.into_make_service()).expect("test server");
    let actor_id = Uuid::now_v7();
    (server, actor_id, runtime, dir)
}

async fn await_review(server: &TestServer, actor_id: Uuid, run_id: Uuid) -> Value {
    let actor = HeaderName::from_static(ACTOR_HEADER);
    let mut last = Value::Null;
    for _ in 0..100 {
        let resp = server
            .get(&format!("/api/v1/detections/{run_id}"))
            .add_header(actor.clone(), actor_id.to_string())
            .await;
        resp.assert_status_ok();
        last = resp.json();
        match last["state"].as_str() {
            Some("awaitingReview") => return last,
            Some("failed") => panic!("run failed: {last:#}"),
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("run {run_id} did not reach awaitingReview within budget; last state: {last:#}");
}

#[tokio::test]
async fn upload_detect_apply_download_round_trips_text_file() {
    let (server, actor_id, runtime, _dir) = server().await;
    let actor = HeaderName::from_static(ACTOR_HEADER);
    let source = std::str::from_utf8(SAMPLE_TXT).expect("fixture is UTF-8");

    let resp = server
        .post("/api/v1/files")
        .add_header(actor.clone(), actor_id.to_string())
        .add_header(axum_test::http::header::CONTENT_TYPE, "text/plain")
        .add_header(
            axum_test::http::header::CONTENT_DISPOSITION,
            r#"attachment; filename="sample.txt""#,
        )
        .bytes(SAMPLE_TXT.into())
        .await;
    resp.assert_status(axum_test::http::StatusCode::CREATED);
    let file_id_raw = resp.json::<Value>()["id"].as_str().unwrap().to_owned();
    let file_id: Uuid = file_id_raw.parse().expect("file id is a uuid");

    let resp = server
        .post("/api/v1/detections")
        .add_header(actor.clone(), actor_id.to_string())
        .json(&json!({
            "documents": [file_id],
            "analyzer": {
                "recognizers": {
                    "pattern": {
                        "mode": "replace",
                        "value": { "builtins": true, "contextEnhanced": true },
                    },
                },
            },
        }))
        .await;
    resp.assert_status(axum_test::http::StatusCode::ACCEPTED);
    let run_id_raw = resp.json::<Value>()["id"].as_str().unwrap().to_owned();
    let run_id: Uuid = run_id_raw.parse().expect("run id is a uuid");

    let detection = await_review(&server, actor_id, run_id).await;
    assert_eq!(detection["state"], json!("awaitingReview"));

    let docs = detection["documents"].as_array().expect("documents array");
    assert_eq!(docs.len(), 1);
    let doc = &docs[0];
    let doc_id_raw = doc["id"].as_str().unwrap().to_owned();
    let doc_id: Uuid = doc_id_raw.parse().expect("doc id is a uuid");

    let body_group = &doc["body"]["body"];
    assert_eq!(body_group["modality"], json!("text"));
    let parts = &doc["body"]["parts"];
    assert!(
        parts.is_null() || parts.as_object().is_some_and(|p| p.is_empty()),
        "plain .txt has no container parts; got {parts}",
    );

    let entities = body_group["entities"]
        .as_array()
        .expect("body has an entities array");
    assert!(
        !entities.is_empty(),
        "fixture should carry at least one entity",
    );
    let target = &entities[0];
    let entity_id_raw = target["entity"]["id"]
        .as_str()
        .expect("entity id is a string");
    let entity_id: Uuid = entity_id_raw.parse().expect("entity id is a uuid");
    let start = target["entity"]["location"]["start"]
        .as_u64()
        .expect("entity location.start") as usize;
    let end = target["entity"]["location"]["end"]
        .as_u64()
        .expect("entity location.end") as usize;
    let literal = &source[start..end];
    assert!(!literal.is_empty(), "entity span should be non-empty");

    let resp = server
        .post("/api/v1/redactions")
        .add_header(actor.clone(), actor_id.to_string())
        .json(&json!({
            "detectionId": run_id,
            "overrides": [{
                "docId": doc_id,
                "entityId": entity_id,
                "action": {
                    "kind": "redact",
                    "text": { "kind": "erase" },
                },
            }],
        }))
        .await;
    resp.assert_status(axum_test::http::StatusCode::ACCEPTED);
    let outputs = resp.json::<Value>()["outputs"]
        .as_array()
        .expect("outputs array")
        .clone();
    assert_eq!(outputs.len(), 1);
    let output = &outputs[0];
    assert_eq!(output["state"], json!("applied"));
    let output_file_id = output["outputFileId"]
        .as_str()
        .expect("apply produced an output file id")
        .to_owned();

    let resp = server
        .get(&format!("/api/v1/files/{output_file_id}/content"))
        .add_header(actor, actor_id.to_string())
        .await;
    resp.assert_status_ok();
    let bytes = resp.as_bytes();
    write_artefact("sample", "txt", bytes);
    let redacted = String::from_utf8(bytes.to_vec()).expect("redacted .txt is UTF-8");
    assert!(!redacted.is_empty(), "redacted output should not be empty");
    assert_ne!(redacted, source, "Erase override must change the output");
    assert!(
        !redacted.contains(literal),
        "Erase override must remove the targeted literal `{literal}`",
    );
    runtime.stop().await;
}
