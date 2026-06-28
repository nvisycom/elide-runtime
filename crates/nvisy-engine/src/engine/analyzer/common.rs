//! Modality-generic compile helpers shared by every per-modality
//! analyzer compile function.
//!
//! `PatternRecognizer` and `NerRecognizer` are themselves
//! modality-generic (both impl `Recognizer<M>` for any
//! `M: TextRecognizable`), so this module hosts the pattern/NER
//! attach + dedup + scope helpers once and lets the per-modality
//! files specialise only LLM (which is generic over a different
//! `LlmModality` trait) and the per-modality rejection of unwired
//! spec variants.

use std::sync::OnceLock;

use elide::detection::Analyzer;
use elide::detection::calibrate::{CalibrateLayer, CalibrationMap as ElideCalibrationMap};
use elide::detection::filter::FilterLayer;
use elide::detection::reconcile::scoring::{Max, NoisyOr};
use elide::detection::reconcile::tiebreaker::{HighestConfidence, LongestSpan};
use elide::detection::reconcile::{Merging, ReconcileLayer, Structural};
use elide::recognition::context::Enhanced;
use elide::recognition::ner::NerRecognizer;
use elide::recognition::pattern::PatternRecognizer;
use elide_bento::BentoNer;
use elide_core::entity::{LabelCatalog, LabelRef};
use elide_core::modality::{Modality, TextRecognizable};
use elide_core::primitive::ConfidenceThreshold;
use elide_core::recognition::Recognizer;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::{
    AnalyzerParams, DeduplicationParams, MergingStrategyParams, NerBackendParams,
    NerRecognizerParams, PatternRecognizerParams, TiebreakerParams,
};

/// The full builtin label catalog from `elide-core`, built once
/// and reused for every request. [`LabelCatalog::with_builtins`]
/// walks `BUILT_INS` and clones every label — cheap once, wasteful
/// per-request.
fn builtin_catalog() -> &'static LabelCatalog {
    static BUILTINS: OnceLock<LabelCatalog> = OnceLock::new();
    BUILTINS.get_or_init(LabelCatalog::with_builtins)
}

/// Build the per-request label catalog from `spec`.
///
/// Engine does not pre-seed builtins; the caller picks. Two sources
/// union into one catalog:
///
/// - [`builtins`](nvisy_core::plan::LabelCatalogParams::builtins) —
///   each name is looked up against the cached full builtin
///   catalog; unknown names warn and are skipped (typos shouldn't
///   fail the request).
/// - [`custom`](nvisy_core::plan::LabelCatalogParams::custom) —
///   inserted as-is; names that collide with a builtin replace it
///   (matches [`LabelCatalog::insert`] semantics: last write wins).
///
/// [`LabelCatalog::with_builtins`]: elide_core::entity::LabelCatalog::with_builtins
/// [`LabelCatalog::insert`]: elide_core::entity::LabelCatalog::insert
pub(crate) fn build_catalog(spec: &AnalyzerParams) -> LabelCatalog {
    let mut catalog = LabelCatalog::new();
    let builtins = builtin_catalog();
    for name in &spec.label_catalog.builtins {
        let label_ref = LabelRef::new(name.clone());
        match builtins.get(&label_ref) {
            Some(label) => {
                catalog.insert(label.clone());
            }
            None => {
                tracing::warn!(
                    target: "engine::analyzer",
                    label = %name,
                    "unknown builtin label name in catalog request; skipping",
                );
            }
        }
    }
    for label in &spec.label_catalog.custom {
        catalog.insert(label.clone().into());
    }
    catalog
}

/// Reject the language enricher: `elide-ner/lingua` wiring isn't
/// exposed through the compile surface yet. Per-modality compile
/// fns call this when they see `params.enrichers.language` set
/// (every modality supports language detection in principle, but
/// no modality currently has the backend wired).
pub(super) fn reject_language_enricher<M>() -> Result<Analyzer<M>, Error>
where
    M: Modality,
{
    Err(Error::new(
        ErrorKind::Validation,
        "analyzer compile: language enricher needs elide-ner/lingua wiring; \
         not exposed through the compile surface yet",
    ))
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
        let mut map = ElideCalibrationMap::new();
        for (recognizer, weight) in &spec.calibration.0 {
            map.insert(recognizer.clone(), *weight);
        }
        analyzer = analyzer.with_layer(CalibrateLayer::new(map));
    }

    analyzer = match spec.merging {
        MergingStrategyParams::Max => {
            analyzer.with_layer(ReconcileLayer::same_label(Merging::new(Max)))
        }
        MergingStrategyParams::NoisyOr => {
            analyzer.with_layer(ReconcileLayer::same_label(Merging::new(NoisyOr)))
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

    let threshold = match spec.min_confidence {
        Some(v) => ConfidenceThreshold::clamped(v),
        None => ConfidenceThreshold::BASELINE,
    };
    analyzer.with_layer(FilterLayer::new().with_threshold(threshold))
}
