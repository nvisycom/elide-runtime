//! Compile the image-applicable parts of [`AnalyzerParams`] into
//! an [`elide::detection::Analyzer<Image>`].
//!
//! Image is the fullest non-text modality: Pattern and NER run
//! over the OCR'd text (the OCR enricher stamps a `Layout` onto
//! the recognizer artifacts upstream), and LLM is available
//! image-natively for vision-language models.
//!
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide::recognition::llm::LlmRecognizer;
use elide_bento::BentoOcr;
use elide_core::modality::image::Image;
use elide_core::{Error, ErrorKind};
use elide_ocr::{MockBackend as MockOcrBackend, OcrEnricher};
use nvisy_core::plan::{
    AnalyzerParams, LlmBackendParams, LlmRecognizerParams, OcrBackendParams, OcrEnricherParams,
};

use super::common::{attach_dedup, attach_ner, attach_pattern, reject_language_enricher};

/// Compile `spec` into an image-modality [`Analyzer`].
pub(crate) fn compile_image(spec: &AnalyzerParams) -> Result<Analyzer<Image>, Error> {
    let mut analyzer = Analyzer::<Image>::new();

    if spec.enrichers.language.is_some() {
        analyzer = reject_language_enricher::<Image>("image")?;
    }
    if let Some(ocr) = &spec.enrichers.ocr {
        analyzer = attach_ocr(analyzer, ocr)?;
    }
    if spec.enrichers.stt.is_some() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: STT enricher is only valid on the audio modality",
        ));
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    for ner in &spec.recognizers.ner {
        analyzer = attach_ner(analyzer, ner)?;
    }
    for llm in &spec.recognizers.llm {
        analyzer = attach_llm(analyzer, llm)?;
    }

    Ok(attach_dedup(analyzer, &spec.deduplication))
}

fn attach_ocr(
    analyzer: Analyzer<Image>,
    spec: &OcrEnricherParams,
) -> Result<Analyzer<Image>, Error> {
    let enricher = match &spec.backend {
        OcrBackendParams::Mock => OcrEnricher::new(MockOcrBackend),
        OcrBackendParams::Bento { base_url, model } => {
            OcrEnricher::new(BentoOcr::new(base_url.clone(), model.clone())?)
        }
    };
    Ok(analyzer.with_enricher(enricher))
}

fn attach_llm(
    analyzer: Analyzer<Image>,
    spec: &LlmRecognizerParams,
) -> Result<Analyzer<Image>, Error> {
    let mut builder = LlmRecognizer::<Image>::builder().with_name(spec.name.clone());
    match &spec.backend {
        LlmBackendParams::Mock => {
            builder = builder.with_mock_backend();
        }
        LlmBackendParams::Openai { .. }
        | LlmBackendParams::Anthropic { .. }
        | LlmBackendParams::Google { .. } => {
            return Err(Error::new(
                ErrorKind::Validation,
                "analyzer compile: real LLM providers need engine-side credential + \
                 rate-limit wiring; not exposed through the compile surface yet",
            ));
        }
    }
    if spec.prompt.is_some() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: custom LLM prompts need the elide Prompt trait wiring; \
             pass `prompt: null` to use the default prompt for now",
        ));
    }
    builder = builder.with_default_prompt();
    Ok(analyzer.with_recognizer(builder.build()?))
}
