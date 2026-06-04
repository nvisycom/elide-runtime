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

use async_trait::async_trait;
use nvisy_core::modality::{Text, TextData, TextLocation};
use nvisy_core::recognition::RecognizerInput;
use nvisy_core::{Result, ValueAt};
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::deduplication::{
    DeduplicationParams, FilterParams, LayerContext, LayerPipeline,
};
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::redaction::Anonymizer;
use nvisy_toolkit::redaction::builtin::{Mask, Redact, Replace};

/// Trivial [`ValueAt`] resolver — slices the source string by byte
/// offsets. The document crate ships a codec-backed implementation;
/// for a standalone example a string slice is enough.
struct SourceSlice<'a>(&'a str);

#[async_trait]
impl ValueAt<Text> for SourceSlice<'_> {
    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        self.0.get(location.start..location.end).map(str::to_owned)
    }
}

const SAMPLE: &str = "Email alice@example.test or call +1 415 555 0100. \
                      Card 4111111111111111 expires 12/27.";

#[tokio::main]
async fn main() -> Result<()> {
    println!("source = {SAMPLE}\n");

    // ── Phase 1: detection ────────────────────────────────────────
    // Pattern-only registry so the example needs no external
    // services. Add NER / LLM recognizers with extra
    // `.add_text_recognizer(...)` calls.
    let pattern = PatternRecognizer::builder()
        .with_registry(PatternRegistry::builtin())
        .build()?;
    let detection = RecognizerRegistry::new().add_text_recognizer(pattern);

    let input = RecognizerInput::<Text>::new(TextData::new(SAMPLE));
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
    let resolver = SourceSlice(SAMPLE);
    let ctx = LayerContext::<Text, SourceSlice<'_>>::new(&resolver);
    let dedup = LayerPipeline::<Text, SourceSlice<'_>>::from_params(
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
    // Pick an operator per entity kind. The document crate makes this
    // policy-driven via `TextRedaction`; here we hand-route to show
    // the operator dispatch surface directly.
    let replace = Replace::new("[{entity_kind}]");
    let mask = Mask::new('#', Some(12));
    let redact = Redact;
    let source = TextData::new(SAMPLE);

    println!("\nredaction:");
    for entity in &entities {
        let replacement = match entity.entity_kind {
            k if k.is_financial() => mask.apply(entity, &source).await?,
            k if k.is_contact_info() => replace.apply(entity, &source).await?,
            _ => redact.apply(entity, &source).await?,
        };
        println!(
            "  - {:?} {}..{} -> {:?}",
            entity.entity_kind, entity.location.start, entity.location.end, replacement,
        );
    }

    Ok(())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "y" } else { "ies" }
}
