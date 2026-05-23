//! End-to-end test for the `onnx-community/gliner_small-v2.1` preset.
//!
//! Marked `#[ignore]` because it downloads the GLiNER ONNX model on
//! first run (cached afterwards in `dirs::cache_dir()/nvisy/models/`),
//! which is too heavy for normal CI. Run explicitly with:
//!
//! ```sh
//! # Tell ort where to find the ONNX Runtime shared library. On macOS
//! # with Homebrew: brew install onnxruntime, then export this var.
//! export ORT_DYLIB_PATH=/opt/homebrew/lib/libonnxruntime.dylib
//!
//! cargo test -p nvisy-nlp --features hf,gliner \
//!     --test preset_gliner -- --ignored --nocapture
//! ```
//!
//! Verifies the manifest at
//! `crates/nvisy-nlp/presets/gliner-small-v2.1.json` drives a real
//! download + ONNX load + zero-shot recognition that produces both a
//! `PersonName` and an `OrganizationName` from a sentence containing
//! both.

#![cfg(all(feature = "hf", feature = "gliner"))]

use std::path::PathBuf;

use nvisy_nlp::NlpPreset;
use nvisy_ontology::entity::EntityKind;

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets/gliner-small-v2.1.json")
}

#[tokio::test]
#[ignore = "downloads a GLiNER ONNX model from HuggingFace; run with --ignored"]
async fn gliner_small_detects_person_and_org() {
    let preset = NlpPreset::Manifest {
        manifest_path: manifest_path(),
        model_path: None,
        tokenizer_path: None,
    };
    let engine = preset
        .build()
        .await
        .expect("preset build should succeed: download + load + label_map validation");

    // The reference preset declares `person`, `organization`,
    // `location`, … as labels. By passing every kind in
    // `context.entities`, we exercise the full label set the model
    // is asked about.
    let ctx = nvisy_nlp::Context::builder()
        .with_text("Alice Johnson works at Acme Corp in Berlin.")
        .with_entities(vec![
            EntityKind::PersonName,
            EntityKind::OrganizationName,
            EntityKind::GeolocationMetadata,
        ])
        .build()
        .unwrap();
    let artifacts = engine.analyze(ctx).await.expect("analyze succeeds");

    let kinds: Vec<EntityKind> = artifacts.entities.iter().map(|e| e.entity_kind).collect();
    assert!(
        kinds.contains(&EntityKind::PersonName),
        "expected at least one PersonName entity, got {kinds:?}",
    );
    assert!(
        kinds.contains(&EntityKind::OrganizationName),
        "expected at least one OrganizationName entity, got {kinds:?}",
    );
}
