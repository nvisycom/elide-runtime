//! Compile the image-applicable parts of [`AnalyzerSpec`] into an
//! [`elide::Analyzer<Image>`].
//!
//! Image is the fullest non-text modality: Pattern and NER run over
//! the OCR'd text (an enricher must stamp a `Layout` onto the
//! recognizer artifacts upstream), and LLM is available
//! image-natively for vision-language models.
//!
//! [`AnalyzerSpec`]: nvisy_core::plan::AnalyzerSpec

use elide::Analyzer;
use elide::recognition::llm::LlmRecognizer;
use elide_bento::BentoOcr;
use elide_core::modality::image::Image;
use elide_core::recognition::Scope;
use elide_core::{Error, ErrorKind};
use elide_ocr::OcrEnricher;
use elide_ocr::backend::MockBackend as MockOcrBackend;
use nvisy_core::plan::{
    AnalyzerSpec, EnricherSpec, LlmBackendSpec, LlmRecognizerSpec, OcrBackendSpec,
    OcrEnricherSpec, RecognizerSpec,
};

use super::common::{attach_dedup, attach_enricher, attach_ner, attach_pattern, build_catalog};
use super::scope::compile_scope;

/// Compile `spec` into an image-modality analyzer + its compiled
/// [`Scope`].
pub fn compile_image(
    spec: &AnalyzerSpec,
) -> Result<(Analyzer<Image>, Scope<Image>), Error> {
    let scope = compile_scope::<Image>(&spec.scope)?;
    let catalog = build_catalog(spec);
    let mut analyzer = Analyzer::<Image>::new();

    for enricher in &spec.enrichers {
        analyzer = match enricher {
            EnricherSpec::Ocr(ocr) => attach_ocr(analyzer, ocr)?,
            other => attach_enricher(analyzer, other)?,
        };
    }

    for recognizer in &spec.recognizers {
        analyzer = match recognizer {
            RecognizerSpec::Pattern(p) => attach_pattern(analyzer, p)?,
            RecognizerSpec::Ner(n) => attach_ner(analyzer, n)?,
            RecognizerSpec::Llm(l) => attach_llm(analyzer, l)?,
        };
    }

    analyzer = attach_dedup(analyzer, &spec.deduplication);
    let _ = catalog;
    Ok((analyzer, scope))
}

fn attach_ocr(
    analyzer: Analyzer<Image>,
    spec: &OcrEnricherSpec,
) -> Result<Analyzer<Image>, Error> {
    let enricher = match &spec.backend {
        OcrBackendSpec::Mock => OcrEnricher::new(MockOcrBackend),
        OcrBackendSpec::Bento { base_url, model } => {
            OcrEnricher::new(BentoOcr::new(base_url.clone(), model.clone())?)
        }
    };
    Ok(analyzer.with_enricher(enricher))
}

fn attach_llm(
    analyzer: Analyzer<Image>,
    spec: &LlmRecognizerSpec,
) -> Result<Analyzer<Image>, Error> {
    let mut builder = LlmRecognizer::<Image>::builder().with_name(spec.name.clone());
    match &spec.backend {
        LlmBackendSpec::Mock => {
            builder = builder.with_mock_backend();
        }
        LlmBackendSpec::Openai { .. }
        | LlmBackendSpec::Anthropic { .. }
        | LlmBackendSpec::Google { .. } => {
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
