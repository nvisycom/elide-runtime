//! Attach the deployment's LLM lineup to a per-modality
//! [`Analyzer`]. Walks [`LlmConfig::recognizers`], filters by
//! declared modality, and builds one elide `LlmRecognizer<M>`
//! per matching entry via elide's `RigBackend` (or `MockBackend`
//! under the `test-utils` feature).
//!
//! Every configured recognizer whose `modalities` list contains
//! the analyzer's modality attaches to every request. Recognizers
//! scoped to a different modality are silently skipped for this
//! analyzer.
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`LlmConfig::recognizers`]: crate::provider::llm::LlmConfig::recognizers

use elide::detection::Analyzer;
#[cfg(feature = "test-utils")]
use elide::recognition::llm::backend::MockBackend as MockLlmBackend;
use elide::recognition::llm::backend::{LlmBackend, LlmModality, RigBackend};
use elide::recognition::llm::prompt::{DefaultPrompt, Jinja2Prompt, Prompt};
use elide::recognition::llm::provider::Provider;
use elide::recognition::llm::{LlmRecognizer, LlmRecognizerBuilder};
use elide_core::{Error, ErrorKind, Result};

use crate::provider::llm::{AttachTo, LlmConfig, LlmPrompt, LlmRecognizerConfig, LlmSource};

/// Attach every LLM recognizer whose `modalities` list contains
/// `modality`.
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
) -> Result<Analyzer<M>>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
    Jinja2Prompt<M>: Prompt<M>,
{
    for recognizer in &llm.recognizers {
        if recognizer.modalities.is_empty() {
            return Err(Error::new(
                ErrorKind::Configuration,
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
        analyzer = attach_one(analyzer, recognizer)?;
    }
    Ok(analyzer)
}

fn attach_one<M>(analyzer: Analyzer<M>, spec: &LlmRecognizerConfig) -> Result<Analyzer<M>>
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
    spec: &LlmRecognizerConfig,
) -> Result<LlmRecognizerBuilder<M>>
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
