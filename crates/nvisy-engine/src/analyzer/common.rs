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

use elide::Analyzer;
use elide::deduplication::calibrate::{CalibrateLayer, CalibrationMap as ElideCalibrationMap};
use elide::deduplication::filter::FilterLayer;
use elide::deduplication::fuse::{FuseLayer, MaxConfidence, Mean, NoisyOr};
use elide::deduplication::resolve::{HighestConfidence, LongestSpan, ResolveLayer};
use elide::recognition::ner::NerRecognizer;
use elide::recognition::pattern::PatternRecognizer;
use elide_bento::BentoNer;
use elide_core::entity::LabelCatalog;
use elide_core::modality::TextRecognizable;
use elide_core::primitive::ConfidenceThreshold;
use elide_core::recognition::Recognizer;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::{
    AnalyzerSpec, DeduplicationSpec, EnricherSpec, FusionStrategySpec, NerBackendSpec,
    NerRecognizerSpec, PatternRecognizerSpec, ResolutionStrategySpec,
};

/// Build the per-request label catalog from `spec`. Engine does not
/// pre-seed builtins; the caller picks.
pub(super) fn build_catalog(spec: &AnalyzerSpec) -> LabelCatalog {
    let mut catalog = LabelCatalog::new();
    for label in &spec.label_catalog {
        catalog.insert(label.clone().into());
    }
    catalog
}

/// Modality-generic enricher attach: handles the variants that
/// apply to every modality. Variants that are modality-specific
/// (e.g. `Ocr` on Image) are rejected here; the per-modality
/// compile function intercepts them before delegating.
pub(super) fn attach_enricher<M>(
    _analyzer: Analyzer<M>,
    spec: &EnricherSpec,
) -> Result<Analyzer<M>, Error>
where
    M: elide_core::modality::Modality,
{
    match spec {
        EnricherSpec::Language(_) => Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: language enricher needs elide-ner/lingua wiring; \
             not exposed through the compile surface yet",
        )),
        EnricherSpec::Ocr(_) => Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: OCR enricher is only valid on the image modality",
        )),
    }
}

/// Attach a [`PatternRecognizer`] built from `spec`. The same
/// recognizer instance — bare or wrapped in elide's `Enhanced`
/// layer — serves any `M: TextRecognizable`, so the helper is
/// uniform across modalities.
pub(super) fn attach_pattern<M>(
    analyzer: Analyzer<M>,
    spec: &PatternRecognizerSpec,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
    elide::recognition::context::Enhanced<PatternRecognizer>: Recognizer<M> + 'static,
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
    spec: &NerRecognizerSpec,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    let mut builder = NerRecognizer::builder().with_name(spec.name.clone());
    match &spec.backend {
        NerBackendSpec::Mock => {
            builder = builder.with_mock_backend();
        }
        NerBackendSpec::Bento { base_url, model } => {
            builder = builder.with_backend(BentoNer::new(base_url.clone(), model.clone())?);
        }
    }
    Ok(analyzer.with_recognizer(builder.build()?))
}

/// Append the deduplication layers: calibrate → fuse → resolve →
/// filter. Calibrate is skipped when the calibration map is empty.
pub(super) fn attach_dedup<M>(
    mut analyzer: Analyzer<M>,
    spec: &DeduplicationSpec,
) -> Analyzer<M>
where
    M: elide_core::modality::Modality,
{
    if !spec.calibration.is_empty() {
        let mut map = ElideCalibrationMap::new();
        for (recognizer, weight) in &spec.calibration.0 {
            map.insert(recognizer.clone(), *weight);
        }
        analyzer = analyzer.with_layer(CalibrateLayer::new(map));
    }

    analyzer = match spec.fusion {
        FusionStrategySpec::MaxConfidence => analyzer.with_layer(FuseLayer::new(MaxConfidence)),
        FusionStrategySpec::Mean => analyzer.with_layer(FuseLayer::new(Mean)),
        FusionStrategySpec::NoisyOr => analyzer.with_layer(FuseLayer::new(NoisyOr)),
    };

    analyzer = match spec.resolution {
        ResolutionStrategySpec::HighestConfidence => {
            analyzer.with_layer(ResolveLayer::new(HighestConfidence))
        }
        ResolutionStrategySpec::LongestSpan => analyzer.with_layer(ResolveLayer::new(LongestSpan)),
    };

    let threshold = match spec.min_confidence {
        Some(v) => ConfidenceThreshold::clamped(v),
        None => ConfidenceThreshold::BASELINE,
    };
    analyzer.with_layer(FilterLayer::new().with_threshold(threshold))
}
