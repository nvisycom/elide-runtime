//! Server-side end-to-end over a multimodal DOCX (text body +
//! embedded PNG): upload, detect, override one entity, apply,
//! download. Asserts the body changed and the image part
//! round-tripped unchanged.

mod fixtures;

use std::io::Read;
use std::time::Duration;

use axum_test::TestServer;
use axum_test::http::HeaderName;
use nvisy_core::plan::AnalyzerParams;
use nvisy_server::{ServiceState, routes};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

use self::fixtures::write_artefact;

const ACTOR_HEADER: &str = "x-actor-id";
const IMAGE_PART_ID: &str = "word/media/image1.png";
const SAMPLE_DOCX: &[u8] = include_bytes!("testdata/sample.docx");

async fn server() -> (TestServer, Uuid, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let state = ServiceState::new(dir.path().to_path_buf(), AnalyzerParams::default())
        .await
        .expect("service state");
    let router = routes().with_state(state);
    let server = TestServer::new(router.into_make_service()).expect("test server");
    let actor_id = Uuid::now_v7();
    (server, actor_id, dir)
}

fn read_zip_entry(buf: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(buf.to_vec())).ok()?;
    let mut entry = zip.by_name(name).ok()?;
    let mut out = Vec::new();
    entry.read_to_end(&mut out).ok()?;
    Some(out)
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
async fn upload_detect_apply_download_round_trips_multimodal_docx() {
    let (server, actor_id, _dir) = server().await;
    let actor = HeaderName::from_static(ACTOR_HEADER);

    let resp = server
        .post("/api/v1/files")
        .add_header(actor.clone(), actor_id.to_string())
        .add_header(
            axum_test::http::header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        )
        .add_header(
            axum_test::http::header::CONTENT_DISPOSITION,
            r#"attachment; filename="sample.docx""#,
        )
        .bytes(SAMPLE_DOCX.into())
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
                "enrichers": {
                    "ocr": {
                        "mode": "replace",
                        "value": { "backend": { "kind": "mock" } },
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
    let body_entities = body_group["entities"].as_array().expect("body entities");
    assert!(
        !body_entities.is_empty(),
        "fixture should carry at least one body entity",
    );

    let parts = &doc["body"]["parts"];
    assert!(
        parts[IMAGE_PART_ID].is_object(),
        "expected parts[{IMAGE_PART_ID}]; got {parts}",
    );
    assert_eq!(parts[IMAGE_PART_ID]["modality"], json!("image"));

    let entity_id_raw = body_entities[0]["entity"]["id"]
        .as_str()
        .expect("entity id is a string");
    let entity_id: Uuid = entity_id_raw.parse().expect("entity id is a uuid");

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
    let result: Value = resp.json();
    assert_eq!(result["id"], json!(run_id.to_string()));
    let outputs = result["outputs"].as_array().expect("outputs array");
    assert_eq!(outputs.len(), 1);
    let output = &outputs[0];
    assert_eq!(output["docId"], json!(doc_id.to_string()));
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
    write_artefact("sample", "docx", bytes);
    let original_body =
        read_zip_entry(SAMPLE_DOCX, "word/document.xml").expect("fixture has word/document.xml");
    let redacted_body =
        read_zip_entry(bytes, "word/document.xml").expect("redacted docx still has the body part");
    assert_ne!(
        redacted_body, original_body,
        "Erase override must change the body XML",
    );
    let image_bytes = read_zip_entry(bytes, IMAGE_PART_ID).expect("image part survives apply");
    let original_image =
        read_zip_entry(SAMPLE_DOCX, IMAGE_PART_ID).expect("fixture has the image part");
    assert_eq!(
        image_bytes, original_image,
        "image part must round-trip unchanged",
    );
}
