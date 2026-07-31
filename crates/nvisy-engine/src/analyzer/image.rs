//! Compile the image-applicable parts of [`AnalyzerParams`] into
//! an [`elide::detection::Analyzer<Image>`].
//!
//! Image is the fullest non-text modality: Pattern and NER run
//! over the OCR'd text (the OCR enricher stamps a `Layout` onto
//! the recognizer artifacts upstream), and LLM is available
//! image-natively for vision-language models. NER and LLM are
//! selected out of the deployment's lineup via
//! `spec.recognizers.{ner,llm}`; the deployment's [`NerConfig`]
//! and [`LlmConfig`] provide the actual recognizer lineups.
//!
//! Modality-foreign enrichers (`language`, `stt`) on `spec` are
//! silently ignored; those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`NerConfig`]: crate::provider::ner::NerConfig
//! [`LlmConfig`]: crate::provider::llm::LlmConfig

use elide::detection::Analyzer;
use elide_core::Error;
use elide_core::modality::image::Image;
use nvisy_schema::plan::AnalyzerParams;

use super::PatternGuardrails;
use super::enricher::attach_ocr;
use super::layer::attach_dedup;
use super::recognizer::{attach_llm_lineup, attach_ner_lineup, attach_pattern};
use crate::provider::llm::{AttachTo, LlmConfig};
use crate::provider::ner::NerConfig;

/// Compile `spec` into an image-modality [`Analyzer`].
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    llm: &LlmConfig,
    guardrails: &PatternGuardrails,
) -> Result<Analyzer<Image>, Error> {
    let mut analyzer = Analyzer::<Image>::new();

    if let Some(ocr) = &spec.enrichers.ocr {
        analyzer = attach_ocr(analyzer, ocr)?;
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern, guardrails)?;
    }
    analyzer = attach_ner_lineup(analyzer, ner, spec.recognizers.ner.as_ref())?;
    analyzer = attach_llm_lineup(analyzer, llm, AttachTo::Image, spec.recognizers.llm.as_ref())?;

    Ok(attach_dedup(analyzer, &spec.deduplication))
}
