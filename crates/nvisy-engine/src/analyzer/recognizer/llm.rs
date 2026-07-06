//! Attach the deployment's LLM lineup to a per-modality
//! [`Analyzer`]. Walks [`LlmConfig::recognizers`], filters by
//! modality, and builds one `LlmRecognizer<M>` per matching
//! entry via elide's `RigBackend` (or `MockBackend` under the
//! `test-utils` feature).

use elide::detection::Analyzer;
use elide::recognition::llm::{LlmRecognizer, LlmRecognizerBuilder};
use elide_core::{Error, ErrorKind};
#[cfg(feature = "test-utils")]
use elide_llm::backend::MockBackend as MockLlmBackend;
use elide_llm::backend::{LlmBackend, LlmModality, RigBackend};
use elide_llm::prompt::{DefaultPrompt, Jinja2Prompt, Prompt};
use elide_llm::provider::Provider;
use nvisy_core::llm::{
    LlmBackendConfig, LlmConfig, LlmPrompt, LlmRecognizer as ConfigRecognizer,
    LlmRecognizerModality,
};

/// Attach every LLM recognizer in `llm.recognizers` whose
/// modality list includes `modality` to `analyzer`. Errors when:
///
/// - The lineup has no recognizer for this modality (compile is
///   only invoked when the request toggled `llm = true`, so
///   "nothing configured for this modality" is user-visible).
/// - A recognizer's `modalities` list is empty.
/// - A Jinja2 prompt file / template fails to load or compile.
/// - A provider-client construction (rig) fails.
///
/// Bound explanation:
///
/// - `RigBackend: LlmBackend<M>` — rig implements both `Text` and
///   `Image`.
/// - `DefaultPrompt: Prompt<M>` — elide ships text + image
///   default prompts.
/// - `Jinja2Prompt<M>: Prompt<M>` — same coverage.
pub(crate) fn attach_llm_lineup<M>(
    mut analyzer: Analyzer<M>,
    llm: &LlmConfig,
    modality: LlmRecognizerModality,
) -> Result<Analyzer<M>, Error>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
    Jinja2Prompt<M>: Prompt<M>,
{
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
    if matched == 0 {
        return Err(Error::new(
            ErrorKind::Validation,
            format!(
                "AnalyzerParams.recognizers.llm = true but the deployment has no LLM \
                 recognizer configured for the {modality:?} modality; add one to \
                 `[[llm.recognizers]]` in the deployment config or leave `llm = false`",
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
