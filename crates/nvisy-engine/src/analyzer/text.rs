//! Compile the text-applicable parts of [`AnalyzerParams`] into an
//! [`elide::detection::Analyzer<Text>`].
//!
//! Text supports the full recognizer set: Pattern, NER, and LLM.
//! Real LLM providers + custom prompts return a `Validation`
//! error today (their credential / rate-limit wiring is not
//! exposed through the compile surface yet).
//!
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide::recognition::llm::LlmRecognizer;
use elide_core::modality::text::Text;
use elide_core::recognition::Scope;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::{AnalyzerParams, LlmBackendParams, LlmRecognizerParams};

use super::common::{
    attach_dedup, attach_ner, attach_pattern, build_catalog, reject_language_enricher,
};
use super::scope::compile_scope;

/// Compile `spec` into a text-modality analyzer + its compiled
/// [`Scope`].
///
/// The returned scope is **not** attached to the analyzer
/// (elide's `Analyzer::analyze` takes `&Scope` per-call, not at
/// build time); the caller pairs them: `analyzer.analyze(data,
/// &scope)`.
pub fn compile_text(spec: &AnalyzerParams) -> Result<(Analyzer<Text>, Scope<Text>), Error> {
    let scope = compile_scope::<Text>(&spec.scope)?;
    let catalog = build_catalog(spec);
    let mut analyzer = Analyzer::<Text>::new();

    // Enrichers: language is the only text-applicable kind; OCR
    // / STT are modality-specific (image / audio).
    if spec.enrichers.language.is_some() {
        analyzer = reject_language_enricher::<Text>()?;
    }
    if spec.enrichers.ocr.is_some() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: OCR enricher is only valid on the image modality",
        ));
    }
    if spec.enrichers.stt.is_some() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: STT enricher is only valid on the audio modality",
        ));
    }

    // Recognizers: pattern (at-most-one), ner (list), llm (list).
    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    for ner in &spec.recognizers.ner {
        analyzer = attach_ner(analyzer, ner)?;
    }
    for llm in &spec.recognizers.llm {
        analyzer = attach_llm(analyzer, llm)?;
    }

    analyzer = attach_dedup(analyzer, &spec.deduplication);
    let _ = catalog; // catalog wiring lands when selectors learn to tag-match upstream
    Ok((analyzer, scope))
}

fn attach_llm(
    analyzer: Analyzer<Text>,
    spec: &LlmRecognizerParams,
) -> Result<Analyzer<Text>, Error> {
    let mut builder = LlmRecognizer::<Text>::builder().with_name(spec.name.clone());
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
