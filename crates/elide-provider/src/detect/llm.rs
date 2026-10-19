//! LLM recognizers, one per configured component.

use elide::backend::metrics::Metered;
use elide::detection::Analyzer;
use elide::recognition::llm::LlmRecognizer;
use elide::recognition::llm::backend::{
    LlmBackend, LlmModality, MockBackend as MockLlmBackend, RigBackend,
};
use elide::recognition::llm::prompt::{DefaultPrompt, Prompt};
use elide::recognition::llm::provider::Provider;
use elide::{Error, ErrorKind, Result};

use crate::component::{Component, LlmBackend as LlmBackendConfig};
use crate::detect::{AttachTo, LlmSource};
use crate::request::Resolved;

pub(super) fn attach_llm_lineup<M>(
    mut analyzer: Analyzer<M>,
    llm: &[Component<LlmBackendConfig>],
    modality: AttachTo,
    resolved: &Resolved<'_>,
) -> Result<Analyzer<M>>
where
    M: LlmModality,
    Metered<RigBackend<M>>: LlmBackend<Modality = M>,
    Metered<MockLlmBackend<M>>: LlmBackend<Modality = M>,
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
        analyzer = attach_llm_one(analyzer, recognizer, resolved)?;
    }
    Ok(analyzer)
}

fn attach_llm_one<M>(
    analyzer: Analyzer<M>,
    spec: &Component<LlmBackendConfig>,
    resolved: &Resolved<'_>,
) -> Result<Analyzer<M>>
where
    M: LlmModality,
    Metered<RigBackend<M>>: LlmBackend<Modality = M>,
    Metered<MockLlmBackend<M>>: LlmBackend<Modality = M>,
    DefaultPrompt: Prompt<M>,
{
    // The recognizer carries its backend as a type parameter, so the
    // mock and the rig-backed arms are distinct types and each
    // attaches its own recognizer. `with_default` supplies the
    // default prompt that a separate builder step used to add.
    Ok(match &spec.backend.source {
        #[cfg(feature = "mocks")]
        LlmSource::Mock => analyzer.with_recognizer(
            LlmRecognizer::with_default(Metered::new(MockLlmBackend::default(), resolved.sink()))
                .with_name(spec.name.clone().to_string()),
        ),
        source => {
            let backend = Metered::new(RigBackend::new(llm_provider(source)?)?, resolved.sink());
            analyzer.with_recognizer(
                LlmRecognizer::with_default(backend).with_name(spec.name.clone().to_string()),
            )
        }
    })
}

/// Resolve a configured source into the rig provider it names.
///
/// The per-client `Provider` variants are gated upstream, and this
/// crate enables them, so a source a config can express always has
/// a variant to map onto.
fn llm_provider(source: &LlmSource) -> Result<Provider> {
    Ok(match source {
        LlmSource::OpenAi(p) => Provider::OpenAi(p.clone()),
        LlmSource::Anthropic(p) => Provider::Anthropic(p.clone()),
        LlmSource::Gemini(p) => Provider::Gemini(p.clone()),
        LlmSource::Ollama(p) => Provider::Ollama(p.clone()),
        #[cfg(feature = "mocks")]
        LlmSource::Mock => {
            return Err(Error::new(
                ErrorKind::Configuration,
                "the mock LLM source has no rig provider; it attaches directly",
            ));
        }
    })
}
