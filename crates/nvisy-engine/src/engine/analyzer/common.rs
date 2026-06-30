//! Modality-generic compile helpers shared by every per-modality
//! analyzer compile function.
//!
//! `PatternRecognizer` and `NerRecognizer` are themselves
//! modality-generic (both impl `Recognizer<M>` for any
//! `M: TextRecognizable`), so this module hosts the pattern/NER
//! attach + dedup helpers once and lets the per-modality files
//! specialise only LLM (which is generic over a different
//! `LlmModality` trait) and the modality-specific enricher
//! attachments (`language` for text, `ocr` for image, `stt` for
//! audio).

use elide::detection::Analyzer;
use elide::detection::calibrate::CalibrateLayer;
use elide::detection::filter::FilterLayer;
use elide::detection::reconcile::scoring::{MaxConfidence, NoisyOrConfidence};
use elide::detection::reconcile::tiebreaker::{HighestConfidence, LongestSpan};
use elide::detection::reconcile::{Merging, ReconcileLayer, Structural};
use elide::enrichment::lingua::LinguaEnricher;
use elide::recognition::context::Enhanced;
use elide::recognition::ner::NerRecognizer;
use elide::recognition::pattern::PatternRecognizer;
use elide_bento::BentoNer;
use elide_core::modality::text::Text;
use elide_core::modality::{Modality, TextRecognizable};
use elide_core::primitive::ConfidenceThreshold;
use elide_core::recognition::Recognizer;
use elide_core::Error;
use nvisy_core::plan::{
    DeduplicationParams, LanguageEnricherParams, MergingStrategyParams, NerBackendParams,
    NerRecognizerParams, PatternRecognizerParams, TiebreakerParams,
};

/// Attach the lingua language-detection [`Enricher<Text>`] built
/// from `spec`. An empty `candidates` list yields the
/// unrestricted detector (every language lingua was compiled
/// with); a non-empty list scopes detection to that pool.
///
/// [`Enricher<Text>`]: elide_core::recognition::Enricher
pub(super) fn attach_language(
    analyzer: Analyzer<Text>,
    spec: &LanguageEnricherParams,
) -> Analyzer<Text> {
    let enricher = if spec.candidates.is_empty() {
        LinguaEnricher::unrestricted()
    } else {
        LinguaEnricher::with_candidates(spec.candidates.iter().cloned())
    };
    analyzer.with_enricher(enricher)
}

/// Attach a [`PatternRecognizer`] built from `spec`. The same
/// recognizer instance — bare or wrapped in elide's `Enhanced`
/// layer — serves any `M: TextRecognizable`, so the helper is
/// uniform across modalities.
pub(super) fn attach_pattern<M>(
    analyzer: Analyzer<M>,
    spec: &PatternRecognizerParams,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
    Enhanced<PatternRecognizer>: Recognizer<M> + 'static,
{
    let mut builder = PatternRecognizer::builder();
    if spec.builtins {
        builder = builder.with_builtin_patterns().with_builtin_dictionaries();
    }
    if spec.context_enhanced {
        Ok(analyzer.with_recognizer(builder.build_context_enhanced()?))
    } else {
        Ok(analyzer.with_recognizer(builder.build()?))
    }
}

/// Attach a [`NerRecognizer`] built from `spec`. Like pattern,
/// modality-generic for any `M: TextRecognizable`.
pub(super) fn attach_ner<M>(
    analyzer: Analyzer<M>,
    spec: &NerRecognizerParams,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    let mut builder = NerRecognizer::builder().with_name(spec.name.clone());
    match &spec.backend {
        NerBackendParams::Mock => {
            builder = builder.with_mock_backend();
        }
        NerBackendParams::Bento { base_url, model } => {
            builder = builder.with_backend(BentoNer::new(base_url.clone(), model.clone())?);
        }
    }
    Ok(analyzer.with_recognizer(builder.build()?))
}

/// Append the deduplication layers: calibrate → reconcile
/// (merging same-label overlaps) → reconcile (tiebreaking
/// cross-label overlaps) → filter. Calibrate is skipped when the
/// calibration map is empty.
pub(super) fn attach_dedup<M>(mut analyzer: Analyzer<M>, spec: &DeduplicationParams) -> Analyzer<M>
where
    M: Modality,
{
    if !spec.calibration.is_empty() {
        analyzer = analyzer.with_layer(CalibrateLayer::new(spec.calibration.clone()));
    }

    analyzer = match spec.merging {
        MergingStrategyParams::Max => {
            analyzer.with_layer(ReconcileLayer::same_label(Merging::new(MaxConfidence)))
        }
        MergingStrategyParams::NoisyOr => {
            analyzer.with_layer(ReconcileLayer::same_label(Merging::new(NoisyOrConfidence)))
        }
    };

    analyzer = match spec.tiebreaker {
        TiebreakerParams::HighestConfidence => analyzer.with_layer(ReconcileLayer::cross_label(
            Structural::standard().with_tiebreaker(HighestConfidence),
        )),
        TiebreakerParams::LongestSpan => analyzer.with_layer(ReconcileLayer::cross_label(
            Structural::standard().with_tiebreaker(LongestSpan),
        )),
    };

    let threshold = spec.min_confidence.unwrap_or(ConfidenceThreshold::BASELINE);
    analyzer.with_layer(FilterLayer::new().with_threshold(threshold))
}
