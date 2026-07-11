//! Attach the deployment's LLM lineup to a per-modality
//! [`Analyzer`]. Walks [`LlmConfig::recognizers`], filters by
//! modality, and builds one `LlmRecognizer<M>` per matching
//! entry via elide's `RigBackend` (or `MockBackend` under the
//! `test-utils` feature).

use elide::detection::Analyzer;
#[cfg(feature = "test-utils")]
use elide::recognition::llm::backend::MockBackend as MockLlmBackend;
use elide::recognition::llm::backend::{LlmBackend, LlmModality, RigBackend};
use elide::recognition::llm::prompt::{DefaultPrompt, Jinja2Prompt, Prompt};
use elide::recognition::llm::provider::Provider;
use elide::recognition::llm::{LlmRecognizer, LlmRecognizerBuilder};
use elide_core::{Error, ErrorKind};

use crate::provider::llm::{
    AttachTo, LlmConfig, LlmPrompt, LlmRecognizer as ConfigRecognizer, LlmSource,
};

/// Attach every LLM recognizer in `llm.recognizers` whose
/// modality list includes `modality` to `analyzer`, dispatched on
/// the request's three-state toggle.
///
/// - `Some(true)`: explicit opt-in. Attaches every configured
///   recognizer whose declared modalities include `modality`;
///   errors when zero match.
/// - `Some(false)`: explicit opt-out. Returns the analyzer
///   unchanged.
/// - `None`: softly-on default. Attaches every matching
///   recognizer if any match; skips silently otherwise.
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
    toggle: Option<bool>,
) -> Result<Analyzer<M>, Error>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
    Jinja2Prompt<M>: Prompt<M>,
{
    if toggle == Some(false) {
        return Ok(analyzer);
    }
    let mut matched = 0usize;
    for recognizer in &llm.recognizers {
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
        matched += 1;
        analyzer = attach_one(analyzer, recognizer)?;
    }
    if matched == 0 && toggle == Some(true) {
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
