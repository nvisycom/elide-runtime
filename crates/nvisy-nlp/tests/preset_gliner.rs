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

use nvisy_nlp::preset::NlpPreset;
use nvisy_ontology::entity::{Entity, EntityKind, Location};

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets/gliner-small-v2.1.json")
}

const TEXT: &str = "Alice Johnson works at Acme Corp in Berlin.";

#[tokio::test]
#[ignore = "downloads a GLiNER ONNX model from HuggingFace; run with --ignored"]
async fn gliner_small_detects_person_and_org() {
    let preset = NlpPreset::Manifest {
        manifest_path: manifest_path(),
        model_path: None,
        tokenizer_path: None,
    };
    let engine = preset.build().await.expect("preset build");

    // The reference preset declares `person`, `organization`,
    // `location`, … as labels. By passing every kind in
    // `context.entities`, we exercise the full label set the model
    // is asked about.
    let ctx = nvisy_nlp::NlpContext::builder()
        .with_entities(vec![
            EntityKind::PersonName,
            EntityKind::OrganizationName,
            EntityKind::GeolocationMetadata,
        ])
        .build()
        .expect("context build");
    let artifacts = engine.analyze(TEXT, &ctx).await.expect("analyze");

    assert_span(&artifacts.entities, EntityKind::PersonName, "Alice Johnson");
    assert_span(
        &artifacts.entities,
        EntityKind::OrganizationName,
        "Acme Corp",
    );
}

/// Find an entity of `kind` whose text span resolves to `expected`
/// against `TEXT`; panic otherwise.
fn assert_span(entities: &[Entity], kind: EntityKind, expected: &str) {
    let hit = entities.iter().find(|e| {
        e.entity_kind == kind
            && matches!(
                &e.location,
                Location::Text(loc)
                    if TEXT.get(loc.start_offset..loc.end_offset) == Some(expected),
            )
    });
    assert!(
        hit.is_some(),
        "expected {kind:?} entity covering {expected:?}; got {:?}",
        entities
            .iter()
            .map(|e| (
                e.entity_kind,
                match &e.location {
                    Location::Text(l) => TEXT.get(l.start_offset..l.end_offset).unwrap_or("?"),
                    _ => "(non-text)",
                }
            ))
            .collect::<Vec<_>>(),
    );
}
