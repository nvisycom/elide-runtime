//! Compile the text-applicable parts of [`AnalyzerParams`] into an
//! [`elide::detection::Analyzer<Text>`].
//!
//! Text supports the full recognizer set: Pattern, NER, and LLM.
//! Real LLM providers + custom prompts return a `Validation`
//! error today (their credential / rate-limit wiring is not
//! exposed through the compile surface yet).
//!
//! Modality-foreign enrichers (`ocr`, `stt`) on `spec` are
//! silently ignored — those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide::recognition::llm::LlmRecognizer;
use elide_core::modality::text::Text;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::{AnalyzerParams, LlmBackendParams, LlmRecognizerParams};

use super::common::{attach_dedup, attach_language, attach_ner, attach_pattern};

/// Compile `spec` into a text-modality [`Analyzer`]. Scope is
/// built separately and lives on the orchestrator.
pub(super) fn compile(spec: &AnalyzerParams) -> Result<Analyzer<Text>, Error> {
    let mut analyzer = Analyzer::<Text>::new();

    if let Some(language) = &spec.enrichers.language {
        analyzer = attach_language(analyzer, language);
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
