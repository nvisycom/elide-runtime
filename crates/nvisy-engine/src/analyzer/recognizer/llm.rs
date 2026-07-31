//! Attach the deployment's LLM lineup to a per-modality
//! [`Analyzer`]. Walks [`LlmConfig::recognizers`], filters by
//! [`ProviderSelection`] and then by modality, and builds one
//! `LlmRecognizer<M>` per matching entry via elide's `RigBackend`
//! (or `MockBackend` under the `test-utils` feature).
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`LlmConfig::recognizers`]: crate::provider::llm::LlmConfig::recognizers
//! [`ProviderSelection`]: nvisy_schema::plan::ProviderSelection

use elide::detection::Analyzer;
#[cfg(feature = "test-utils")]
use elide::recognition::llm::backend::MockBackend as MockLlmBackend;
use elide::recognition::llm::backend::{LlmBackend, LlmModality, RigBackend};
use elide::recognition::llm::prompt::{DefaultPrompt, Jinja2Prompt, Prompt};
use elide::recognition::llm::provider::Provider;
use elide::recognition::llm::{LlmRecognizer, LlmRecognizerBuilder};
use elide_core::{Error, ErrorKind};
use nvisy_schema::plan::ProviderSelection;

use super::selection::select;
use crate::provider::llm::{
    AttachTo, LlmConfig, LlmPrompt, LlmRecognizer as ConfigRecognizer, LlmSource,
};

/// Attach LLM recognizers selected by `selection` whose modality
/// list includes `modality`.
///
/// The name allowlist runs first; the modality filter then drops
/// any allowlisted recognizer that doesn't attach to this
/// analyzer's modality. Only `All(true)` requires at least one
/// modality-matching recognizer to remain — `Only(names)`
/// silently skips if every named recognizer is scoped to a
/// different modality.
///
/// Errors on: any recognizer whose `modalities` list is empty
/// (bad config), Jinja2 prompt load/compile failure, provider
/// client construction failure.
///
/// Bound explanation:
///
/// - `RigBackend: LlmBackend<M>` — rig implements both `Text` and
///   `Image`.
/// - `DefaultPrompt: Prompt<M>` — elide ships text + image
///   default prompts.
/// - `Jinja2Prompt<M>: Prompt<M>` — same coverage.
pub(in crate::analyzer) fn attach_llm_lineup<M>(
    mut analyzer: Analyzer<M>,
    llm: &LlmConfig,
    modality: AttachTo,
    selection: Option<&ProviderSelection>,
) -> Result<Analyzer<M>, Error>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
    Jinja2Prompt<M>: Prompt<M>,
{
    let selected = select(
        selection,
        &llm.recognizers,
        |r| r.name.as_str(),
        "llm",
        "[[llm.recognizers]]",
    )?;
    let mut modality_matched = 0usize;
    for recognizer in &selected {
        if recognizer.modalities.is_empty() {
            return Err(Error::new(
                ErrorKind::Validation,
                format!(
                    "LLM recognizer `{}` declares empty `modalities`; \
                     add at least one modality or remove the recognizer",
                    recognizer.name,
                ),
            ));
        }
        if !recognizer.modalities.contains(&modality) {
            continue;
        }
        modality_matched += 1;
        analyzer = attach_one(analyzer, recognizer)?;
    }
    if modality_matched == 0 && matches!(selection, Some(ProviderSelection::All(true))) {
        return Err(Error::new(
            ErrorKind::Validation,
            format!(
                "AnalyzerParams.recognizers.llm = true but the deployment has no LLM \
                 recognizer configured for the {modality:?} modality; add one to \
                 `[[llm.recognizers]]` in the deployment config or leave `llm` unset / false",
            ),
        ));
    }
    Ok(analyzer)
}

fn attach_one<M>(analyzer: Analyzer<M>, spec: &ConfigRecognizer) -> Result<Analyzer<M>, Error>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
    Jinja2Prompt<M>: Prompt<M>,
{
    let mut builder = LlmRecognizer::<M>::builder().with_name(spec.name.clone());
    builder = attach_source(builder, spec)?;
    builder = match &spec.prompt {
        None => builder.with_default_prompt(),
        Some(LlmPrompt::Inline { template }) => {
            builder.with_prompt(Jinja2Prompt::<M>::from_template(template.clone())?)
        }
        Some(LlmPrompt::File { path }) => builder.with_prompt(Jinja2Prompt::<M>::from_file(path)?),
    };
    Ok(analyzer.with_recognizer(builder.build()?))
}

fn attach_source<M>(
    builder: LlmRecognizerBuilder<M>,
    spec: &ConfigRecognizer,
) -> Result<LlmRecognizerBuilder<M>, Error>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
{
    let provider = match &spec.source {
        LlmSource::OpenAi(p) => Provider::OpenAi(p.clone()),
        LlmSource::Anthropic(p) => Provider::Anthropic(p.clone()),
        LlmSource::Gemini(p) => Provider::Gemini(p.clone()),
        LlmSource::Ollama(p) => Provider::Ollama(p.clone()),
        #[cfg(feature = "test-utils")]
        LlmSource::Mock => {
            return Ok(builder.with_backend(MockLlmBackend));
        }
    };
    Ok(builder.with_backend(RigBackend::new(provider)?))
}
