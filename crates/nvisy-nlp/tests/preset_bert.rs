//! End-to-end test for the `dslim/bert-base-NER` preset.
//!
//! Marked `#[ignore]` because it downloads the ~411 MiB ONNX model on
//! first run (cached afterwards in `dirs::cache_dir()/nvisy/models/`),
//! which is too heavy for normal CI. Run explicitly with:
//!
//! ```sh
//! # Tell ort where to find the ONNX Runtime shared library. On macOS
//! # with Homebrew: brew install onnxruntime, then export this var.
//! export ORT_DYLIB_PATH=/opt/homebrew/lib/libonnxruntime.dylib
//!
//! cargo test -p nvisy-nlp --features hf,onnx \
//!     --test preset_bert -- --ignored --nocapture
//! ```
//!
//! Verifies the manifest at `crates/nvisy-nlp/presets/dslim-bert-base-NER.json`
//! drives a real download + ONNX load + recognition that produces a
//! plausible person-name entity from a fixed sentence.

#![cfg(all(feature = "hf", feature = "onnx"))]

use std::path::PathBuf;

use nvisy_nlp::preset::NlpPreset;
use nvisy_ontology::entity::{EntityKind, Location};

/// Workspace-root-relative path to the reference manifest.
fn manifest_path() -> PathBuf {
    // CARGO_MANIFEST_DIR points at `crates/nvisy-nlp` when the test is
    // built — the manifest lives next to `src/`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets/dslim-bert-base-NER.json")
}

const TEXT: &str = "Alice Johnson works at Acme Corp in Berlin.";

#[tokio::test]
#[ignore = "downloads ~411 MiB model from HuggingFace; run with --ignored"]
async fn bert_base_ner_detects_a_person_name() {
    let preset = NlpPreset::Manifest {
        manifest_path: manifest_path(),
        model_path: None,
        tokenizer_path: None,
    };
    let engine = preset.build().await.expect("preset build");

    let ctx = nvisy_nlp::NlpContext::default();
    let artifacts = engine.analyze(TEXT, &ctx).await.expect("analyze");

    // The model is CoNLL-2003 English: PER (PersonName), ORG
    // (OrganizationName), LOC (GeolocationMetadata), MISC. We check
    // the strong claim (a PersonName appears at the right span) and
    // leave ORG/LOC as best-effort so the test stays robust to
    // marginal model disagreement.
    assert_span(&artifacts.entities, EntityKind::PersonName, "Alice Johnson");
}

/// Find an entity of `kind` whose text span resolves to `expected`
/// against `TEXT`; panic otherwise.
fn assert_span(entities: &[nvisy_ontology::entity::Entity], kind: EntityKind, expected: &str) {
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
