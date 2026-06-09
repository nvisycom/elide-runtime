//! Integration test: `RecognizerRegistry` dispatching pattern + NER
//! + LLM recognizers in one pass.
//!
//! Wires every text-modality recognizer the toolkit ships with against
//! one input string and asserts each one fires. The NER and LLM legs
//! call live services (Bento ner-bento + Ollama) so the test is
//! `#[ignore]`d by default; run locally with:
//!
//! ```text
//! cargo test -p nvisy-toolkit --features bento -- --ignored
//! ```
//!
//! Required services:
//!
//! - `ner-bento` listening at `$NVISY_BENTO_URL` (default
//!   `http://localhost:3000`). See nvisycom/inference.
//! - Ollama listening at `$NVISY_OLLAMA_URL` (default
//!   `http://localhost:11434`) with `$NVISY_OLLAMA_MODEL` (default
//!   `llama3.1:8b`) pulled.

#![cfg(feature = "bento")]

use std::env;

use nvisy_core::entity::EntityKind;
use nvisy_core::modality::{Text, TextData};
use nvisy_core::recognition::RecognizerInput;
use nvisy_llm::backend::rig::RigBackend;
use nvisy_llm::provider::LlmProvider;
use nvisy_llm::{DefaultPrompt, LlmRecognizer};
use nvisy_ner::NerRecognizer;
use nvisy_ner::backend::{BentoBackend, BentoParams};
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::detection::RecognizerRegistry;

/// Sample text that triggers all three recognizers:
/// - `pattern` catches the email (built-in email regex).
/// - `ner` (zero-shot Bento) catches the person name + organisation.
/// - `llm` (Ollama) is asked the same question and picks up at least
///   one of the above.
const SAMPLE: &str = "Alice Carter (alice.carter@acme.test) joined Acme Corp last week.";

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn build_registry() -> RecognizerRegistry {
    let pattern = PatternRecognizer::builder()
        .with_registry(PatternRegistry::builtin())
        .build()
        .expect("pattern recognizer builds from builtin registry");

    let bento_url = env_or("NVISY_BENTO_URL", "http://localhost:3000");
    let bento_backend = BentoBackend::new(BentoParams::new(bento_url)).expect("bento backend init");

    let ner = NerRecognizer::builder()
        .with_name("ner")
        .with_engine(bento_backend)
        .with_supported_kinds(EntityKind::all().collect::<Vec<_>>())
        .build()
        .expect("ner recognizer builds");

    let ollama_model = env_or("NVISY_OLLAMA_MODEL", "llama3.1:8b");
    let ollama_url = env_or("NVISY_OLLAMA_URL", "http://localhost:11434");
    let rig = RigBackend::builder()
        .with_provider(LlmProvider::ollama_with_url(&ollama_model, &ollama_url))
        .build()
        .expect("rig backend builds for Ollama");
    let llm = LlmRecognizer::builder()
        .with_name("llm")
        .with_backend(rig)
        .with_prompt(DefaultPrompt)
        .build()
        .expect("llm recognizer builds");

    RecognizerRegistry::new()
        .with_recognizer::<Text>(pattern)
        .with_recognizer::<Text>(ner)
        .with_recognizer::<Text>(llm)
}

fn fired(
    entities: &[nvisy_core::entity::Entity<nvisy_core::modality::Text>],
    source: &str,
) -> bool {
    entities
        .iter()
        .any(|e| e.trail.iter().any(|s| s.source == source))
}

#[tokio::test]
#[ignore = "requires live ner-bento + ollama; run with `--ignored`"]
async fn registry_dispatches_pattern_ner_and_llm_against_live_services() {
    let registry = build_registry();
    let input = RecognizerInput::new(TextData::new(SAMPLE.to_owned()));

    let entities = registry
        .run::<Text>(input)
        .await
        .expect("dispatch over pattern + ner + llm succeeds");

    assert!(
        fired(&entities, "pattern"),
        "pattern recognizer did not fire on `{SAMPLE}`; got {entities:?}"
    );
    assert!(
        fired(&entities, "ner"),
        "ner (bento) recognizer did not fire on `{SAMPLE}`; got {entities:?}"
    );
    assert!(
        fired(&entities, "llm-ner"),
        "llm (ollama) recognizer did not fire on `{SAMPLE}`; got {entities:?}"
    );
}
