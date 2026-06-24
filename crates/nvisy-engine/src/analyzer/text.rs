//! Compile the text-applicable parts of [`AnalyzerSpec`] into an
//! [`elide::Analyzer<Text>`].
//!
//! Text supports the full recognizer set: Pattern, NER, and LLM.
//! Real LLM providers + custom prompts and the NER bento backend
//! return a `Validation` error today (their infrastructure is not
//! exposed through the compile surface yet).
//!
//! [`AnalyzerSpec`]: nvisy_core::plan::AnalyzerSpec

use elide::Analyzer;
use elide::recognition::llm::LlmRecognizer;
use elide_core::modality::text::Text;
use elide_core::recognition::Scope;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::{
    AnalyzerSpec, LlmBackendSpec, LlmRecognizerSpec, RecognizerSpec,
};

use super::common::{attach_dedup, attach_enricher, attach_ner, attach_pattern, build_catalog};
use super::scope::compile_scope;

/// Compile `spec` into a text-modality analyzer + its compiled
/// [`Scope`].
///
/// The returned scope is **not** attached to the analyzer
/// (elide's `Analyzer::analyze` takes `&Scope` per-call, not at
/// build time); the caller pairs them: `analyzer.analyze(data,
/// &scope)`.
pub fn compile_text(spec: &AnalyzerSpec) -> Result<(Analyzer<Text>, Scope<Text>), Error> {
    let scope = compile_scope::<Text>(&spec.scope)?;
    let catalog = build_catalog(spec);
    let mut analyzer = Analyzer::<Text>::new();

    for enricher in &spec.enrichers {
        analyzer = attach_enricher(analyzer, enricher)?;
    }

    for recognizer in &spec.recognizers {
        analyzer = match recognizer {
            RecognizerSpec::Pattern(p) => attach_pattern(analyzer, p)?,
            RecognizerSpec::Ner(n) => attach_ner(analyzer, n)?,
            RecognizerSpec::Llm(l) => attach_llm(analyzer, l)?,
        };
    }

    analyzer = attach_dedup(analyzer, &spec.deduplication);
    let _ = catalog; // catalog wiring lands when selectors learn to tag-match upstream
    Ok((analyzer, scope))
}

fn attach_llm(
    analyzer: Analyzer<Text>,
    spec: &LlmRecognizerSpec,
) -> Result<Analyzer<Text>, Error> {
    let mut builder = LlmRecognizer::<Text>::builder().with_name(spec.name.clone());
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
