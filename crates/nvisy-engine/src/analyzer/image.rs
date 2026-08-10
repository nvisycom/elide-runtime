//! Compile the image-applicable parts of an
//! [`AnalyzerParams`] into an [`elide::detection::Analyzer<Image>`].
//!
//! Image is the fullest non-text modality: Pattern and NER run
//! over the OCR'd text (the OCR enricher stamps a `Layout` onto
//! the recognizer artifacts upstream), and LLM is available
//! image-natively for vision-language models. The OCR enricher
//! attaches when the deployment wired one via
//! [`Engine::with_ocr`].
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`Engine::with_ocr`]: crate::Engine::with_ocr

use elide::detection::Analyzer;
use elide_core::Result;
use elide_core::modality::image::Image;
use nvisy_schema::plan::AnalyzerParams;

use super::enricher::attach_ocr;
use super::layer::attach_dedup;
use super::recognizer::{attach_llm_lineup, attach_ner_lineup, attach_pattern};
use crate::provider::llm::{AttachTo, LlmConfig};
use crate::provider::ner::NerConfig;
use crate::provider::ocr::OcrBackend;

/// Compile `spec` into an image-modality [`Analyzer`].
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    llm: &LlmConfig,
    ocr: Option<&OcrBackend>,
) -> Result<Analyzer<Image>> {
    let mut analyzer = Analyzer::<Image>::new();

    if let Some(ocr) = ocr {
        analyzer = attach_ocr(analyzer, ocr)?;
    }

    analyzer = attach_pattern(analyzer, &spec.recognizers)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;
    analyzer = attach_llm_lineup(analyzer, llm, AttachTo::Image)?;

    Ok(attach_dedup(analyzer))
}
