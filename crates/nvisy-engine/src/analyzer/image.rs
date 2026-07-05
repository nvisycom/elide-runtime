//! Compile the image-applicable parts of [`AnalyzerParams`] into
//! an [`elide::detection::Analyzer<Image>`].
//!
//! Image is the fullest non-text modality: Pattern and NER run
//! over the OCR'd text (the OCR enricher stamps a `Layout` onto
//! the recognizer artifacts upstream), and LLM is available
//! image-natively for vision-language models. NER and LLM are
//! opt-in via `spec.recognizers.ner = true` /
//! `spec.recognizers.llm = true`; the deployment's [`NerConfig`]
//! and [`LlmConfig`] provide the actual recognizer lineups.
//!
//! Modality-foreign enrichers (`language`, `stt`) on `spec` are
//! silently ignored; those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`NerConfig`]: nvisy_core::ner::NerConfig
//! [`LlmConfig`]: nvisy_core::llm::LlmConfig

use elide::detection::Analyzer;
use elide_bento::BentoOcr;
use elide_core::Error;
use elide_core::modality::image::Image;
#[cfg(feature = "test-utils")]
use elide_ocr::MockBackend as MockOcrBackend;
use elide_ocr::OcrEnricher;
use nvisy_core::llm::{LlmConfig, LlmRecognizerModality};
use nvisy_core::ner::NerConfig;
use nvisy_schema::plan::{AnalyzerParams, OcrBackendParams, OcrEnricherParams};

use super::common::{attach_dedup, attach_pattern};
use super::llm::attach_llm_lineup;
use super::ner::attach_ner_lineup;

/// Compile `spec` into an image-modality [`Analyzer`].
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    llm: &LlmConfig,
) -> Result<Analyzer<Image>, Error> {
    let mut analyzer = Analyzer::<Image>::new();

    if let Some(ocr) = &spec.enrichers.ocr {
        analyzer = attach_ocr(analyzer, ocr)?;
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    if spec.recognizers.ner {
        analyzer = attach_ner_lineup(analyzer, ner)?;
    }
    if spec.recognizers.llm {
        analyzer = attach_llm_lineup(analyzer, llm, LlmRecognizerModality::Image)?;
    }

    Ok(attach_dedup(analyzer, &spec.deduplication))
}

fn attach_ocr(
    analyzer: Analyzer<Image>,
    spec: &OcrEnricherParams,
) -> Result<Analyzer<Image>, Error> {
    let enricher = match &spec.backend {
        OcrBackendParams::Bento { base_url, model } => {
            OcrEnricher::new(BentoOcr::new(base_url.clone(), model.clone())?)
        }
        #[cfg(feature = "test-utils")]
        OcrBackendParams::Mock => OcrEnricher::new(MockOcrBackend),
        // `OcrBackendParams` is `#[non_exhaustive]`. Unknown
        // variants surface as Validation instead of silently
        // skipping OCR.
        _ => {
            return Err(elide_core::Error::new(
                elide_core::ErrorKind::Validation,
                "OCR enricher uses a backend kind this engine binary doesn't \
                 understand; upgrade the engine or downgrade the config",
            ));
        }
    };
    Ok(analyzer.with_enricher(enricher))
}
