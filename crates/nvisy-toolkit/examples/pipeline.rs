//! End-to-end toolkit pipeline against a synthetic text input:
//! **detection → deduplication → redaction**, all in one process,
//! no external services required.
//!
//! Mirrors the producer side of what the document crate's
//! per-modality phases run for every text block. Each section below
//! corresponds to one phase the document crate orchestrates.
//!
//! Run:
//!
//! ```text
//! cargo run -p nvisy-toolkit --example pipeline
//! ```

use nvisy_core::Result;
use nvisy_core::entity::EntityKind;
use nvisy_core::extraction::RedactAt;
use nvisy_core::modality::Text;
use nvisy_core::recognition::RecognizerInput;
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::deduplication::{
    DeduplicationParams, FilterParams, LayerContext, LayerPipeline,
};
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::ingestion::MemoryBuffer;
use nvisy_toolkit::redaction::RedactionRegistry;
use nvisy_toolkit::redaction::builtin::{Mask, Redact, Replace};

const SAMPLE: &str = "Email alice@example.test or call +1 415 555 0100. \
                      Card 4111111111111111 expires 12/27.";

#[tokio::main]
async fn main() -> Result<()> {
    // ── Phase 0: ingestion ────────────────────────────────────────
    // Wrap the source bytes in a MemoryBuffer. The same buffer
    // satisfies TextAt/DataAt/RedactAt for the later phases and owns
    // the payload the detection recognizers consume.
    let mut source = MemoryBuffer::<Text>::from_text(SAMPLE);
    println!("source = {}\n", source.as_str());

    // ── Phase 1: detection ────────────────────────────────────────
    // Pattern-only registry so the example needs no external
    // services. Add NER / LLM recognizers with extra
    // `.add_text_recognizer(...)` calls.
    let pattern = PatternRecognizer::builder()
        .with_registry(PatternRegistry::builtin())
        .build()?;
    let detection = RecognizerRegistry::new().add_text_recognizer(pattern);

    let input = RecognizerInput::new(source.data().clone());
    let entities = detection.run_text(input).await?;

    println!(
        "detection: {} entit{}",
        entities.len(),
        plural(entities.len())
    );
    for entity in &entities {
        println!(
            "  - {:?} at {}..{} (confidence {:.2})",
            entity.entity_kind,
            entity.location.start,
            entity.location.end,
            entity.confidence.get(),
        );
    }

    // ── Phase 2: deduplication ────────────────────────────────────
    // Canonical four-layer pipeline: calibrate → filter → fuse →
    // resolve. Drops overlapping / low-confidence detections so the
    // redaction phase sees a conflict-free entity set.
    let ctx = LayerContext::<Text, MemoryBuffer<_>>::new(&source);
    let dedup = LayerPipeline::<Text, MemoryBuffer<Text>>::from_params(
        &DeduplicationParams::default(),
        FilterParams::default(),
    );

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
    // back into the buffer.
    let redaction = RedactionRegistry::<Text>::new()
        .insert_kind(EntityKind::EmailAddress, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PhoneNumber, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PaymentCard, Mask::new('#', Some(12)))
        .with_fallback(Redact);

    let redactions = redaction.apply_all(&entities, source.data()).await?;
    println!("\nredaction: produced {} replacement(s)", redactions.len());
    source.redact_at(redactions).await?;

    println!("\nredacted = {}", source.as_str());
    Ok(())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "y" } else { "ies" }
}
