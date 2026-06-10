//! End-to-end toolkit pipeline against a synthetic text input,
//! using the real codec read/write surface
//! ([`CodecRegistry`] → [`DecodedBuffer`]): **detection →
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
//! [`DecodedBuffer`]: nvisy_codec::document::DecodedBuffer

use nvisy_codec::CodecRegistry;
use nvisy_codec::document::DecodedBuffer;
use nvisy_core::Result;
use nvisy_core::entity::EntityKind;
use nvisy_core::modality::{Text, TextData};
use nvisy_core::recognition::RecognizerInput;
use nvisy_core::redaction::RedactAt;
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::deduplication::{LayerContext, LayerParams, LayerPipeline};
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::redaction::RedactionRegistry;
use nvisy_toolkit::redaction::builtin::{Mask, Redact, Replace};

const SAMPLE: &str = "Email alice@example.test or call +1 415 555 0100. \
                      Card 4111111111111111 expires 12/27.";

#[tokio::main]
async fn main() -> Result<()> {
    // ── Phase 0: ingestion ────────────────────────────────────────
    // Decode the sample through the codec registry — the same front
    // door the engine uses — by resolving the `.txt` extension to a
    // loader and pulling the resulting handle back into a typed
    // text DecodedBuffer.
    let registry = CodecRegistry::with_builtin();
    let untyped = registry.decode_from_memory(SAMPLE, "txt").await?;
    let handle = untyped
        .into_text()
        .expect("txt extension resolves to text modality");
    let mut source = DecodedBuffer::new(handle);
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
    let ctx = LayerContext::<Text, DecodedBuffer<Text>>::new(&source);
    let dedup = LayerPipeline::<Text, DecodedBuffer<Text>>::from_params(&LayerParams::default());

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
        .insert_kind(EntityKind::PaymentCard, Mask::new('#', Some(12)))
        .with_fallback(Redact);

    // `source` is the codec-backed buffer; it satisfies `DataAt<Text>`,
    // so apply_all pulls only the per-entity byte range and hands
    // *that* (not the whole document) to each anonymizer.
    let redactions = redaction.apply_all(&entities, &source).await?;
    println!("\nredaction: produced {} replacement(s)", redactions.len());
    source.redact_at(redactions).await?;

    // Dump the post-redaction buffer back to a string by walking
    // every chunk the handler yields. (TxtHandler::read is
    // single-line-bound, so a "full range" read would return None;
    // chunk iteration is the canonical full-text path.)
    let mut redacted = String::new();
    while let Some(chunk) = source.handle_mut().handler_mut().next_chunk().await? {
        if !redacted.is_empty() {
            redacted.push('\n');
        }
        redacted.push_str(chunk.data.as_str());
    }
    println!("\nredacted = {redacted}");
    Ok(())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "y" } else { "ies" }
}
