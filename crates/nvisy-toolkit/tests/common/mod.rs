//! Shared helpers for the toolkit pipeline E2E tests
//! (`pipeline_txt.rs`, `pipeline_csv.rs`, `pipeline_json.rs`).
//!
//! Each codec test loads a fixture through the codec registry,
//! drives detection per chunk, runs dedup + redaction, encodes the
//! redacted handler back to bytes, and asserts on the result.
//! The boilerplate of building the recognizer / redaction
//! registries lives here so the per-codec tests stay focused on
//! what's actually codec-specific.

#![allow(dead_code)] // Each integration-test file pulls a subset.

use std::str::from_utf8;

use nvisy_codec::CodecRegistry;
use nvisy_codec::document::DocumentHandle;
use nvisy_core::Result;
use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::extraction::DataAt;
use nvisy_core::modality::{Modality, Tabular, Text, TextData};
use nvisy_core::primitive::ConfidenceThreshold;
use nvisy_core::redaction::{RedactAt, Redactions};
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::deduplication::{LayerContext, LayerParams, LayerPipeline};
use nvisy_toolkit::detection::{RecognizerRegistry, RecognizerRegistryExt};
use nvisy_toolkit::redaction::RedactionRegistry;
use nvisy_toolkit::redaction::anonymizer::{Mask, Replace};

/// Build the shipped pattern recognizer once.
pub fn shipped_recognizer() -> PatternRecognizer {
    PatternRecognizer::builder()
        .with_registry(PatternRegistry::builtin())
        .build()
        .expect("shipped recognizer builds")
}

/// Build the redaction registry the E2E tests use: emails and
/// phones replaced with their kind tag, payment cards masked, no
/// fallback (unmatched kinds are skipped).
pub fn redaction_registry() -> RedactionRegistry<Text> {
    RedactionRegistry::<Text>::new()
        .insert_kind(EntityKind::EmailAddress, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PhoneNumber, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PaymentCard, Mask::stars())
}

/// Tabular sibling of [`redaction_registry`], with the same
/// kind-to-operator mapping. Built-in [`Replace`] and [`Mask`] now
/// impl [`Anonymizer<Tabular>`], so the registry construction is
/// identical apart from the modality parameter.
///
/// [`Anonymizer<Tabular>`]: nvisy_toolkit::redaction::Anonymizer
pub fn tabular_redaction_registry() -> RedactionRegistry<Tabular> {
    RedactionRegistry::<Tabular>::new()
        .insert_kind(EntityKind::EmailAddress, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PhoneNumber, Replace::new("[{entity_kind}]"))
        .insert_kind(EntityKind::PaymentCard, Mask::stars())
}

/// Standard dedup params for the tests: a `0.5` confidence
/// threshold so the low-confidence ISO-639 short-code matches
/// from the languages dictionary drop out (see
/// `assets/dictionaries/general/languages.toml`'s
/// `column_scores`).
pub fn dedup_params() -> LayerParams {
    LayerParams {
        confidence_threshold: Some(ConfidenceThreshold::new(0.5).unwrap()),
        ..LayerParams::default()
    }
}

/// Decode `bytes` through the codec registry by extension, returning
/// a typed text [`DocumentHandle`].
pub async fn decode_text_buffer(
    bytes: impl Into<bytes::Bytes>,
    extension: &str,
) -> Result<DocumentHandle<Text>> {
    let registry = CodecRegistry::with_builtin();
    let untyped = registry.decode_from_memory(bytes.into(), extension).await?;
    Ok(untyped
        .into_text()
        .expect("text-modality extension resolves to text handle"))
}

/// Tabular sibling of [`decode_text_buffer`].
pub async fn decode_tabular_buffer(
    bytes: impl Into<bytes::Bytes>,
    extension: &str,
) -> Result<DocumentHandle<Tabular>> {
    let registry = CodecRegistry::with_builtin();
    let untyped = registry.decode_from_memory(bytes.into(), extension).await?;
    Ok(untyped
        .into_tabular()
        .expect("tabular-modality extension resolves to tabular handle"))
}

/// Drive detection across every chunk the handler yields and
/// return the concatenated entity list, with locations already
/// lifted to source-byte coordinates. Wraps the recognizer in a
/// single-element [`RecognizerRegistry`] and delegates to
/// [`RecognizerRegistryExt::detect`].
pub async fn detect_per_chunk(
    recognizer: PatternRecognizer,
    buffer: &mut DocumentHandle<Text>,
) -> Result<Vec<Entity<Text>>> {
    let registry = RecognizerRegistry::new().with_recognizer(recognizer);
    registry.detect(buffer.handler_mut()).await
}

/// Run dedup over the entity list using the standard threshold.
pub async fn dedup<R>(entities: Vec<Entity<Text>>, resolver: &R) -> Vec<Entity<Text>>
where
    R: DataAt<Text> + nvisy_core::extraction::TextAt<Text> + ?Sized + Sync,
{
    let ctx = LayerContext::<Text, R>::new(resolver);
    let pipeline = LayerPipeline::<Text, R>::from_params(&dedup_params());
    pipeline.run(entities, &ctx).await
}

/// Apply the redaction registry against `entities`, flush through
/// `RedactAt`, encode the handler back to bytes, and return the
/// UTF-8 string.
pub async fn redact_and_encode(
    buffer: &mut DocumentHandle<Text>,
    entities: &[Entity<Text>],
) -> Result<String> {
    let redactions: Redactions<Text> = redaction_registry()
        .apply_all(entities.iter(), buffer)
        .await?;
    buffer.redact_at(redactions).await?;
    let encoded = buffer.handler().encode()?;
    Ok(from_utf8(encoded.as_bytes())
        .expect("text codec encode produces UTF-8")
        .to_owned())
}

/// Tabular sibling of [`detect_per_chunk`]: wraps the recognizer in
/// a single-element [`RecognizerRegistry`] registered for `Text`
/// (recognizers don't care that the strings are CSV cells) and
/// delegates to [`RecognizerRegistryExt::detect`]; the handler's
/// modality picks the [`Tabular`] reshape.
pub async fn detect_per_cell(
    recognizer: PatternRecognizer,
    buffer: &mut DocumentHandle<Tabular>,
) -> Result<Vec<Entity<Tabular>>> {
    let registry = RecognizerRegistry::new().with_recognizer(recognizer);
    registry.detect(buffer.handler_mut()).await
}

/// Tabular sibling of [`dedup`].
pub async fn dedup_tabular<R>(entities: Vec<Entity<Tabular>>, resolver: &R) -> Vec<Entity<Tabular>>
where
    R: DataAt<Tabular> + nvisy_core::extraction::TextAt<Tabular> + ?Sized + Sync,
{
    let ctx = LayerContext::<Tabular, R>::new(resolver);
    let pipeline = LayerPipeline::<Tabular, R>::from_params(&dedup_params());
    pipeline.run(entities, &ctx).await
}

/// Tabular sibling of [`redact_and_encode`].
pub async fn redact_and_encode_tabular(
    buffer: &mut DocumentHandle<Tabular>,
    entities: &[Entity<Tabular>],
) -> Result<String> {
    let redactions: Redactions<Tabular> = tabular_redaction_registry()
        .apply_all(entities.iter(), buffer)
        .await?;
    buffer.redact_at(redactions).await?;
    let encoded = buffer.handler().encode()?;
    Ok(from_utf8(encoded.as_bytes())
        .expect("tabular codec encode produces UTF-8")
        .to_owned())
}

/// Assert at least one entity of `kind` was detected and its
/// surfaced match equals `needle`. Substring-based so fixtures
/// can evolve without offset-counting churn.
pub fn assert_entity_matched(
    text: &str,
    entities: &[Entity<Text>],
    kind: EntityKind,
    needle: &str,
) {
    let hit = entities
        .iter()
        .any(|e| e.entity_kind == kind && &text[e.location.start..e.location.end] == needle);
    assert!(
        hit,
        "expected `{needle}` as {kind:?}; got: {:?}",
        entities
            .iter()
            .map(|e| (e.entity_kind, &text[e.location.start..e.location.end]))
            .collect::<Vec<_>>()
    );
}

/// Assert at least one [`Entity<Tabular>`] of `kind` lives in cell
/// `(row, col)` and its intra-cell range slices to `needle` when
/// applied to the cell's source string.
pub fn assert_cell_entity_matched(
    cell_value: &str,
    entities: &[Entity<Tabular>],
    kind: EntityKind,
    row: u32,
    col: u32,
    needle: &str,
) {
    let hit = entities.iter().any(|e| {
        if e.entity_kind != kind {
            return false;
        }
        if e.location.row_index != row || e.location.column_index != col {
            return false;
        }
        let start = e.location.start_offset.unwrap_or(0);
        let end = e.location.end_offset.unwrap_or(cell_value.len());
        cell_value.get(start..end) == Some(needle)
    });
    assert!(
        hit,
        "expected `{needle}` as {kind:?} at ({row},{col}); got: {:?}",
        entities
            .iter()
            .map(|e| (e.entity_kind, e.location.row_index, e.location.column_index))
            .collect::<Vec<_>>()
    );
}

/// Write the redacted output next to its fixture as
/// `{stem}.redacted.{ext}` for human inspection. The pipeline
/// tests run `cargo test`, so dropping the artifact in
/// `testdata/pipeline/` (gitignored under `*.redacted.*`) lets
/// you `diff` against the original fixture after a run.
pub fn write_redacted_artifact(fixture: &str, redacted: &str) {
    let path = std::path::Path::new(fixture);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("fixture has a UTF-8 stem");
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .expect("fixture has a UTF-8 extension");
    let parent = path.parent().expect("fixture has a parent");
    let out = parent.join(format!("{stem}.redacted.{ext}"));
    std::fs::write(&out, redacted).unwrap_or_else(|e| {
        panic!("write redacted artifact {}: {e}", out.display());
    });
}

/// Generic Modality marker import kept for cross-test type hygiene.
const _: fn() = || {
    fn assert_modality<M: Modality>() {}
    assert_modality::<Text>();
    fn assert_data<D: Sized>() {}
    assert_data::<TextData>();
};
