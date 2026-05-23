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
//! cargo test -p nvisy-nlp --features hf \
//!     --test preset_bert -- --ignored --nocapture
//! ```
//!
//! Verifies the manifest at `crates/nvisy-nlp/presets/dslim-bert-base-NER.json`
//! drives a real download + ONNX load + recognition that produces a
//! plausible person-name entity from a fixed sentence.

#![cfg(feature = "hf")]

use std::path::PathBuf;

use nvisy_nlp::NlpPreset;
use nvisy_ontology::entity::EntityKind;

/// Workspace-root-relative path to the reference manifest.
fn manifest_path() -> PathBuf {
    // CARGO_MANIFEST_DIR points at `crates/nvisy-nlp` when the test is
    // built — the manifest lives next to `src/`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets/dslim-bert-base-NER.json")
}

#[tokio::test]
#[ignore = "downloads ~411 MiB model from HuggingFace; run with --ignored"]
async fn bert_base_ner_detects_a_person_name() {
    let preset = NlpPreset::Manifest {
        manifest_path: manifest_path(),
        model_path: None,
        tokenizer_path: None,
    };
    let engine = preset
        .build()
        .await
        .expect("preset build should succeed: download + load + label_map validation");

    let artifacts = engine
        .analyze("My name is Wolfgang and I live in Berlin.")
        .await
        .expect("analyze succeeds");

    // The model is CoNLL-2003 English: it should pick `Wolfgang` as
    // PER (PersonName) and `Berlin` as LOC (GeolocationMetadata).
    // We check the strong claim (a PersonName appears) rather than
    // asserting on every kind, so test brittleness stays bounded if
    // the model marginally disagrees on `Berlin`.
    let kinds: Vec<EntityKind> = artifacts.entities.iter().map(|e| e.entity_kind).collect();
    assert!(
        kinds.contains(&EntityKind::PersonName),
        "expected at least one PersonName entity, got {kinds:?}",
    );
}
