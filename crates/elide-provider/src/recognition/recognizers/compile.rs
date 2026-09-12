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

use std::sync::{Arc, LazyLock};

use elide::detection::Analyzer;
use elide::entity::LabelCatalog;
use elide::modality::TextRecognizable;
use elide::recognition::Recognizer;
use elide::recognition::context::Enhanced;
#[cfg(feature = "test-utils")]
use elide::recognition::llm::backend::MockBackend as MockLlmBackend;
use elide::recognition::llm::backend::{LlmBackend, LlmModality, RigBackend};
use elide::recognition::llm::prompt::{DefaultPrompt, Prompt};
use elide::recognition::llm::provider::Provider;
use elide::recognition::llm::{LlmRecognizer, LlmRecognizerBuilder};
use elide::recognition::ner::NerRecognizer;
use elide::recognition::pattern::{
    Dictionary, PatternRecognizer, PatternRecognizerBuilder, Regex, Scoring, Term, Variant,
};
use elide::{Error, ErrorKind, Result};
use elide_bentoml::ner::BentoNer;
use elide_governance::recognition::{CustomMatcher, MatchOn, Recognition};

use super::super::{Component, LlmBackend as LlmBackendConfig, NerBackend as NerBackendConfig};
use crate::recognition::{AttachTo, LlmSource, NerBackend};

/// Aggregate cap on total dictionary terms across every shipped
/// dictionary, compiled into one shared Aho-Corasick automaton.
const MAX_DICTIONARY_TERM_COUNT: usize = 100_000;

/// Aggregate byte budget across every shipped dictionary's terms.
const MAX_DICTIONARY_TERM_BYTES: usize = 8 * 1024 * 1024;

/// Cap on matchers one request may declare, across every policy.
///
/// Each compiles a regex or a term list on the request path, so
/// this bounds what a caller can make the engine do before
/// detection starts.
const MAX_CUSTOM_MATCHERS: usize = 64;

/// Cap on terms in one matcher's list.
const MAX_CUSTOM_TERMS: usize = 4096;

/// Compiled-size budget for one caller-supplied regex.
///
/// The `regex` crate does not backtrack, so a pathological pattern
/// costs compile time and memory rather than match time. This
/// bounds both.
const MAX_CUSTOM_REGEX_BYTES: usize = 256 * 1024;

/// The built-in pattern recognizer, compiled once for the process.
///
/// Building one compiles the whole shipped regex set — around 50 ms
/// — and the result depends on nothing per-request: the same
/// patterns, dictionaries and limits every time. It used to be
/// rebuilt inside [`attach_pattern`], which runs once per modality
/// per request, so a four-modality deployment paid ~200 ms of regex
/// compilation before reading a byte of the document.
///
/// Shared rather than cloned: `PatternRecognizer` owns compiled
/// automata and is not `Clone`, but an `Arc` is itself a
/// `Recognizer`, so every analyzer attaches the same instance.
///
/// The build takes no caller input — the patterns, dictionaries
/// and limits are all compiled-in constants — so a failure means
/// the shipped set is broken, which no deployment can act on and
/// every request would hit. It panics rather than threading an
/// error no caller can handle.
static BUILTIN_PATTERNS: LazyLock<Arc<Enhanced<PatternRecognizer>>> = LazyLock::new(|| {
    let builder = pattern_with_limits(PatternRecognizer::builder())
        .with_builtin_patterns()
        .with_builtin_dictionaries();
    Arc::new(
        builder
            .build_context_enhanced()
            .expect("the shipped builtin patterns compile"),
    )
});

/// Attach the built-in [`PatternRecognizer`] wrapped in the
/// `Enhanced` context layer.
pub(in crate::recognition) fn attach_pattern<M>(analyzer: Analyzer<M>) -> Analyzer<M>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
    Enhanced<PatternRecognizer>: Recognizer<M> + 'static,
{
    analyzer.with_recognizer(Arc::clone(&BUILTIN_PATTERNS))
}

/// Attach the caller's own matchers, compiled fresh for this
/// request.
///
/// The built-in recognizer is shared and never varies; these do,
/// so they are their own recognizer built per call. A request
/// declaring no matchers builds nothing and pays nothing.
///
/// # Errors
///
/// Returns [`Configuration`](ErrorKind::Configuration) when a
/// matcher names a label its policy does not declare, names a
/// shipped built-in, exceeds a limit, or carries a regex that does
/// not compile.
pub(in crate::recognition) fn attach_custom<M>(
    analyzer: Analyzer<M>,
    recognition: &[Recognition],
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
{
    let total: usize = recognition.iter().map(|r| r.matchers.len()).sum();
    if total == 0 {
        return Ok(analyzer);
    }
    if total > MAX_CUSTOM_MATCHERS {
        return Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "this request declares {total} custom matchers, over the \
                 limit of {MAX_CUSTOM_MATCHERS}: each compiles on the request \
                 path before detection starts.",
            ),
        ));
    }

    let mut builder = pattern_with_limits(PatternRecognizer::builder())
        .with_size_limit(MAX_CUSTOM_REGEX_BYTES)
        .with_dfa_size_limit(MAX_CUSTOM_REGEX_BYTES);
    for vocabulary in recognition {
        for matcher in &vocabulary.matchers {
            builder = extend(builder, recognition, matcher)?;
        }
    }
    // Not context-enhanced: boosting reads keyword lists a matcher
    // does not carry, so there is nothing for it to work from.
    Ok(analyzer.with_recognizer(builder.build()?))
}

/// Validate one matcher against its policy and fold it into
/// `builder`.
fn extend(
    builder: PatternRecognizerBuilder,
    recognition: &[Recognition],
    matcher: &CustomMatcher,
) -> Result<PatternRecognizerBuilder> {
    check_label(recognition, matcher)?;
    match &matcher.match_on {
        MatchOn::Pattern { pattern } => {
            let variant = Variant::new(pattern.clone())
                .map_err(|err| invalid(matcher, format!("its regex is invalid: {err}")))?
                .with_score(matcher.confidence);
            let regex = Regex::builder()
                .with_name(matcher.name.clone())
                .with_labels(vec![matcher.label.clone()])
                .with_variants(vec![variant])
                .build()
                .map_err(|err| invalid(matcher, format!("it does not build: {err}")))?;
            Ok(builder.with_pattern(regex))
        }
        MatchOn::Terms { terms } => {
            if terms.is_empty() {
                return Err(invalid(matcher, "its term list is empty".to_owned()));
            }
            if terms.len() > MAX_CUSTOM_TERMS {
                return Err(invalid(
                    matcher,
                    format!(
                        "it lists {} terms, over the limit of {MAX_CUSTOM_TERMS}",
                        terms.len(),
                    ),
                ));
            }
            let dictionary = Dictionary::builder()
                .with_name(matcher.name.clone())
                .with_labels(vec![matcher.label.clone()])
                .with_terms(terms.iter().map(Term::new).collect::<Vec<_>>())
                .with_scoring(Scoring::Uniform(matcher.confidence))
                .build()
                .map_err(|err| invalid(matcher, format!("it does not build: {err}")))?;
            Ok(builder.with_dictionary(dictionary))
        }
    }
}

/// A matcher may only detect a label the request introduces.
///
/// Naming a label the request does not declare would detect into a
/// vocabulary the catalog never held; naming a shipped built-in
/// would race elide's own definition, and reconciliation would
/// pick a winner by confidence rather than by intent.
fn check_label(recognition: &[Recognition], matcher: &CustomMatcher) -> Result<()> {
    if BUILTIN_LABELS.contains(&matcher.label) {
        return Err(invalid(
            matcher,
            format!(
                "`{}` is a label elide already detects: a matcher for it would \
                 race the shipped definition. Introduce a label of your own.",
                matcher.label.as_str(),
            ),
        ));
    }
    // Any of the request's vocabularies may declare it: they merge
    // into one catalog, so which one carries the label is moot.
    if recognition
        .iter()
        .flat_map(|r| &r.custom)
        .any(|l| l.to_ref() == matcher.label)
    {
        return Ok(());
    }
    Err(invalid(
        matcher,
        format!(
            "`{}` is not among the request's own `custom` labels: a matcher \
             detects a label the request introduces, so declare it there first.",
            matcher.label.as_str(),
        ),
    ))
}

/// Every label elide ships, resolved once.
static BUILTIN_LABELS: LazyLock<LabelCatalog> = LazyLock::new(LabelCatalog::with_builtins);

/// A rejection naming the matcher, so a caller knows which one.
fn invalid(matcher: &CustomMatcher, why: String) -> Error {
    Error::new(
        ErrorKind::Configuration,
        format!("custom matcher `{}` is rejected: {why}", matcher.name),
    )
}

fn pattern_with_limits(builder: PatternRecognizerBuilder) -> PatternRecognizerBuilder {
    builder
        .with_term_count_limit(MAX_DICTIONARY_TERM_COUNT)
        .with_term_bytes_limit(MAX_DICTIONARY_TERM_BYTES)
}

/// Attach every recognizer in `ner` to `analyzer`.
///
/// Every configured recognizer attaches to every request; the
/// deployment picks the lineup in its `ProviderConfig`.
pub(in crate::recognition) fn attach_ner_lineup<M>(
    mut analyzer: Analyzer<M>,
    ner: &[Component<NerBackendConfig>],
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    for recognizer in ner {
        analyzer = attach_ner_one(analyzer, recognizer)?;
    }
    Ok(analyzer)
}

fn attach_ner_one<M>(analyzer: Analyzer<M>, spec: &Component<NerBackend>) -> Result<Analyzer<M>>
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
pub(in crate::recognition) fn attach_llm_lineup<M>(
    mut analyzer: Analyzer<M>,
    llm: &[Component<LlmBackendConfig>],
    modality: AttachTo,
) -> Result<Analyzer<M>>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
{
    for recognizer in llm {
        if recognizer.backend.modalities.is_empty() {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "LLM recognizer `{}` declares empty `modalities`; \
                     add at least one modality or remove the recognizer",
                    recognizer.name,
                ),
            ));
        }
        if !recognizer.backend.modalities.contains(&modality) {
            continue;
        }
        analyzer = attach_llm_one(analyzer, recognizer)?;
    }
    Ok(analyzer)
}

fn attach_llm_one<M>(
    analyzer: Analyzer<M>,
    spec: &Component<LlmBackendConfig>,
) -> Result<Analyzer<M>>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
    DefaultPrompt: Prompt<M>,
{
    let mut builder = LlmRecognizer::<M>::builder().with_name(spec.name.clone());
    builder = attach_llm_source(builder, spec)?;
    builder = builder.with_default_prompt();
    Ok(analyzer.with_recognizer(builder.build()?))
}

fn attach_llm_source<M>(
    builder: LlmRecognizerBuilder<M>,
    spec: &Component<LlmBackendConfig>,
) -> Result<LlmRecognizerBuilder<M>>
where
    M: LlmModality,
    RigBackend: LlmBackend<M>,
{
    let provider = match &spec.backend.source {
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
