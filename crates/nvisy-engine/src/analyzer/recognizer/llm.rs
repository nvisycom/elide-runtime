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
use nvisy_core::llm::{
    LlmBackendConfig, LlmConfig, LlmPrompt, LlmRecognizer as ConfigRecognizer,
    LlmRecognizerModality,
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
    modality: LlmRecognizerModality,
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
    builder = attach_backend(builder, spec)?;
    builder = match &spec.prompt {
        None => builder.with_default_prompt(),
        Some(LlmPrompt::Inline { template }) => {
            builder.with_prompt(Jinja2Prompt::<M>::from_template(template.clone())?)
        }
        Some(LlmPrompt::File { path }) => builder.with_prompt(Jinja2Prompt::<M>::from_file(path)?),
        // `LlmPrompt` is `#[non_exhaustive]`. A future variant
        // reaching this arm in an older binary should surface as
        // a compile error rather than silently falling back to
        // the default prompt.
        Some(_) => {
            return Err(Error::new(
                ErrorKind::Validation,
                format!(
                    "LLM recognizer `{}` uses a prompt shape this engine binary \
                     doesn't understand; upgrade the engine or downgrade the config",
                    spec.name,
                ),
            ));
        }
    };
    Ok(analyzer.with_recognizer(builder.build()?))
}

fn attach_backend<M>(
    builder: LlmRecognizerBuilder<M>,
    spec: &ConfigRecognizer,
) -> Result<LlmRecognizerBuilder<M>, Error>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
{
    let provider = match &spec.backend {
        LlmBackendConfig::OpenAi(p) => Provider::OpenAi(p.clone()),
        LlmBackendConfig::Anthropic(p) => Provider::Anthropic(p.clone()),
        LlmBackendConfig::Gemini(p) => Provider::Gemini(p.clone()),
        LlmBackendConfig::Ollama(p) => Provider::Ollama(p.clone()),
        #[cfg(feature = "test-utils")]
        LlmBackendConfig::Mock => {
            return Ok(builder.with_backend(MockLlmBackend));
        }
        // `LlmBackendConfig` is `#[non_exhaustive]`. Unknown
        // variants surface as Validation.
        _ => {
            return Err(Error::new(
                ErrorKind::Validation,
                format!(
                    "LLM recognizer `{}` uses a backend kind this engine binary \
                     doesn't understand; upgrade the engine or downgrade the config",
                    spec.name,
                ),
            ));
        }
    };
    Ok(builder.with_backend(RigBackend::new(provider)?))
}
