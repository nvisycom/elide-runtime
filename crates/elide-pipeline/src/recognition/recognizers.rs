//! Per-recognizer compile helpers: pattern, NER, LLM.
//!
//! Symmetric with [`super::enrichers`]: each helper takes the
//! deployment-owned config for its lineup ([`NerConfig`] /
//! [`LlmConfig`] for the two inference lineups), and attaches
//! the compiled recognizer to a [`elide::detection::Analyzer<M>`].
//!
//! Pattern is modality-generic (`M: TextRecognizable`); NER and
//! LLM constrain over their upstream `Recognizer<M>` /
//! `LlmModality<M>` impls respectively: modalities that lack
//! the impl either fail the compile with a Validation error
//! (NER: cheap trait bound) or are silently skipped upstream
//! (LLM: no `LlmModality` impl for Tabular / Audio).
//!
//! [`NerConfig`]: crate::recognition::NerConfig
//! [`LlmConfig`]: crate::recognition::LlmConfig

use elide::detection::Analyzer;
use elide::modality::TextRecognizable;
use elide::recognition::Recognizer;
use elide::recognition::context::Enhanced;
#[cfg(feature = "test-utils")]
use elide::recognition::llm::backend::MockBackend as MockLlmBackend;
use elide::recognition::llm::backend::{LlmBackend, LlmModality, RigBackend};
use elide::recognition::llm::prompt::{DefaultPrompt, Jinja2Prompt, Prompt};
use elide::recognition::llm::provider::Provider;
use elide::recognition::llm::{LlmRecognizer, LlmRecognizerBuilder};
use elide::recognition::ner::NerRecognizer;
use elide::recognition::pattern::{PatternRecognizer, PatternRecognizerBuilder};
use elide::{Error, ErrorKind, Result};
use elide_bento::ner::BentoNer;

use crate::recognition::{
    AttachTo, LlmConfig, LlmPrompt, LlmRecognizerConfig, LlmSource, NerBackend, NerConfig,
    NerRecognizerConfig,
};

/// Aggregate cap on total dictionary terms across every shipped
/// dictionary, compiled into one shared Aho-Corasick automaton.
const MAX_DICTIONARY_TERM_COUNT: usize = 100_000;

/// Aggregate byte budget across every shipped dictionary's terms.
const MAX_DICTIONARY_TERM_BYTES: usize = 8 * 1024 * 1024;

/// Attach the built-in [`PatternRecognizer`] wrapped in the
/// `Enhanced` context layer.
pub(super) fn attach_pattern<M>(analyzer: Analyzer<M>) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
    Enhanced<PatternRecognizer>: Recognizer<M> + 'static,
{
    let builder = pattern_with_limits(PatternRecognizer::builder())
        .with_builtin_patterns()
        .with_builtin_dictionaries();
    Ok(analyzer.with_recognizer(builder.build_context_enhanced()?))
}

fn pattern_with_limits(builder: PatternRecognizerBuilder) -> PatternRecognizerBuilder {
    builder
        .with_term_count_limit(MAX_DICTIONARY_TERM_COUNT)
        .with_term_bytes_limit(MAX_DICTIONARY_TERM_BYTES)
}

/// Attach every recognizer in `ner` to `analyzer`.
///
/// Every configured recognizer attaches to every request; the
/// deployment picks the lineup at `Engine::with_ner` time.
pub(super) fn attach_ner_lineup<M>(
    mut analyzer: Analyzer<M>,
    ner: &NerConfig,
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    for recognizer in &ner.recognizers {
        analyzer = attach_ner_one(analyzer, recognizer)?;
    }
    Ok(analyzer)
}

fn attach_ner_one<M>(analyzer: Analyzer<M>, spec: &NerRecognizerConfig) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    let mut builder = NerRecognizer::builder().with_name(spec.name.clone());
    match &spec.backend {
        NerBackend::Bento { base_url, model } => {
            builder = builder.with_backend(BentoNer::new(base_url.clone(), model.clone())?);
        }
        #[cfg(feature = "test-utils")]
        NerBackend::Mock => {
            builder = builder.with_mock_backend();
        }
    }
    Ok(analyzer.with_recognizer(builder.build()?))
}

/// Attach every LLM recognizer whose `modalities` list contains
/// `modality`.
///
/// Errors on: any recognizer whose `modalities` list is empty
/// (bad config), Jinja2 prompt load/compile failure, provider
/// client construction failure.
///
/// Bound explanation:
///
/// - `RigBackend: LlmBackend<M>`: rig implements both `Text` and
///   `Image`.
/// - `DefaultPrompt: Prompt<M>`: elide ships text + image
///   default prompts.
/// - `Jinja2Prompt<M>: Prompt<M>`: same coverage.
pub(super) fn attach_llm_lineup<M>(
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
        analyzer = attach_llm_one(analyzer, recognizer)?;
    }
    Ok(analyzer)
}

fn attach_llm_one<M>(analyzer: Analyzer<M>, spec: &LlmRecognizerConfig) -> Result<Analyzer<M>>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
    Jinja2Prompt<M>: Prompt<M>,
{
    let mut builder = LlmRecognizer::<M>::builder().with_name(spec.name.clone());
    builder = attach_llm_source(builder, spec)?;
    builder = match &spec.prompt {
        None => builder.with_default_prompt(),
        Some(LlmPrompt::Inline { template }) => {
            builder.with_prompt(Jinja2Prompt::<M>::from_template(template.clone())?)
        }
        Some(LlmPrompt::File { path }) => builder.with_prompt(Jinja2Prompt::<M>::from_file(path)?),
    };
    Ok(analyzer.with_recognizer(builder.build()?))
}

fn attach_llm_source<M>(
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
