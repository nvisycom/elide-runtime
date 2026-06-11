//! End-to-end toolkit pipeline against a synthetic text input,
//! using the real codec read/write surface
//! ([`CodecRegistry`] → [`DocumentHandle`]): **detection →
//! deduplication → redaction**, all in one process, no external
//! services required.
//!
//! Mirrors the producer side of what the engine runs per text
//! block. Each section below corresponds to one phase the engine
//! orchestrates.
//!
//! Run:
//!
//! ```text
//! cargo run -p nvisy-toolkit --example pipeline
//! ```
//!
//! [`CodecRegistry`]: nvisy_codec::CodecRegistry
//! [`DocumentHandle`]: nvisy_codec::DocumentHandle

use std::str::from_utf8;

use nvisy_codec::{CodecRegistry, DocumentHandle};
use nvisy_core::Result;
use nvisy_core::entity::EntityKind;
use nvisy_core::modality::{Text, TextData};
use nvisy_core::primitive::ConfidenceThreshold;
use nvisy_core::recognition::RecognizerInput;
use nvisy_core::redaction::RedactAt;
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::deduplication::{LayerContext, LayerParams, LayerPipeline};
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::redaction::RedactionRegistry;
use nvisy_toolkit::redaction::anonymizer::{Mask, Redact, Replace};

const SAMPLE: &str = "Email alice@example.test or call +1 415 555 0100. \
                      Card 4111111111111111 expires 12/27.";

#[tokio::main]
async fn main() -> Result<()> {
    // ── Phase 0: ingestion ────────────────────────────────────────
    // Decode the sample through the codec registry — the same front
    // door the engine uses — by resolving the `.txt` extension to a
    // loader and pulling the resulting handle back into a typed
    // text DocumentHandle.
    let registry = CodecRegistry::with_builtin();
    let untyped = registry.decode_from_memory(SAMPLE, "txt").await?;
    let mut source = untyped
        .into_text()
        .expect("txt extension resolves to text modality");
    println!("source = {SAMPLE}\n");

    // ── Phase 1: detection ────────────────────────────────────────
    // Pattern-only registry so the example needs no external
    // services. Add NER / LLM recognizers with extra
    // `.with_recognizer(...)` calls.
    let pattern = PatternRecognizer::builder()
        .with_registry(PatternRegistry::builtin())
        .build()?;
    let detection = RecognizerRegistry::new().with_recognizer(pattern);

    let input = RecognizerInput::new(TextData::new(SAMPLE.to_owned()));
    let entities = detection.run::<Text>(input).await?;

    println!(
        "detection: {} entit{}",
        entities.len(),
        plural(entities.len())
    );
    for entity in &entities {
        let matched = &SAMPLE[entity.location.start..entity.location.end];
        println!(
            "  - {:?} {:?} at {}..{} (confidence {:.2})",
            entity.entity_kind,
            matched,
            entity.location.start,
            entity.location.end,
            entity.confidence.get(),
        );
    }

    // ── Phase 2: deduplication ────────────────────────────────────
    // Canonical four-layer pipeline: calibrate → filter → fuse →
    // resolve. Drops overlapping / low-confidence detections so the
    // redaction phase sees a conflict-free entity set.
    //
    // The 0.5 threshold filters out the ISO-639 short-code matches
    // (`or`, `am`, ...) from the languages dictionary, which load
    // at 0.30 because they collide with common English words. See
    // `assets/dictionaries/general/languages.toml`'s `column_scores`.
    let params = LayerParams {
        confidence_threshold: Some(ConfidenceThreshold::new(0.5).unwrap()),
        ..LayerParams::default()
    };
    let ctx = LayerContext::<Text, DocumentHandle<Text>>::new(&source);
    let dedup = LayerPipeline::<Text, DocumentHandle<Text>>::from_params(&params);

    let before = entities.len();
    let entities = dedup.run(entities, &ctx).await;
    println!(
        "\ndeduplication: kept {} of {} (dropped {})",
        entities.len(),
        before,
        before - entities.len(),
    );

    // ── Phase 3: redaction ────────────────────────────────────────
    // Register an operator per kind plus a catch-all fallback.
    // `apply_all` runs the per-kind resolver across every entity and
    // returns a `Redactions` batch; `redact_at` flushes the batch
    // back into the codec handler in place.
    let redaction = RedactionRegistry::<Text>::new()
        .insert_kind(EntityKind::EmailAddress, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PhoneNumber, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PaymentCard, Mask::stars())
        .with_fallback(Redact);

    // `source` is the codec-backed handle; it satisfies `DataAt<Text>`,
    // so apply_all pulls only the per-entity byte range and hands
    // *that* (not the whole document) to each anonymizer.
    let redactions = redaction.apply_all(&entities, &source).await?;
    println!("\nredaction: produced {} replacement(s)", redactions.len());
    source.redact_at(redactions).await?;

    // Serialize the post-redaction handler back through the codec.
    // `Handler::encode` is what the engine's export phase calls to
    // produce output bytes; it knows the format's reassembly rules
    // (line terminators for txt, JSON envelope for json, …) so the
    // example doesn't have to.
    let encoded = source.handler().encode()?;
    let redacted = from_utf8(encoded.as_bytes()).expect("txt encode produces UTF-8");
    println!("\nredacted = {redacted}");
    Ok(())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "y" } else { "ies" }
}
