//! Attach the deployment's NER lineup to a per-modality
//! [`Analyzer`]. Walks [`NerConfig::recognizers`] and builds one
//! `NerRecognizer` per entry via `elide-bento`'s `BentoNer`
//! backend (or `MockBackend` under the `test-utils` feature).
//!
//! Symmetric with [`super::llm`]; called by every per-modality
//! compile function when `AnalyzerParams.recognizers.ner == true`.
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`NerConfig::recognizers`]: nvisy_core::ner::NerConfig::recognizers

use elide::detection::Analyzer;
use elide::recognition::ner::NerRecognizer;
use elide_bento::BentoNer;
use elide_core::Error;
use elide_core::modality::TextRecognizable;
use elide_core::recognition::Recognizer;
use nvisy_core::ner::{NerBackendConfig, NerConfig, NerRecognizer as ConfigNerRecognizer};

/// Attach every recognizer from the deployment's NER lineup.
/// Errors when the lineup is empty (compile is only invoked when
/// the request toggled `ner = true`, so "no recognizers
/// configured" is user-visible). Modality-generic for any
/// `M: TextRecognizable`.
pub(in crate::analyzer) fn attach_ner_lineup<M>(
    mut analyzer: Analyzer<M>,
    ner: &NerConfig,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    if ner.recognizers.is_empty() {
        return Err(Error::new(
            elide_core::ErrorKind::Validation,
            "AnalyzerParams.recognizers.ner = true but the deployment has no NER \
             recognizer configured; add one to `[[ner.recognizers]]` in the \
             deployment config or leave `ner = false`",
        ));
    }
    for recognizer in &ner.recognizers {
        analyzer = attach_ner_one(analyzer, recognizer)?;
    }
    Ok(analyzer)
}

fn attach_ner_one<M>(
    analyzer: Analyzer<M>,
    spec: &ConfigNerRecognizer,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    let mut builder = NerRecognizer::builder().with_name(spec.name.clone());
    match &spec.backend {
        NerBackendConfig::Bento { base_url, model } => {
            builder = builder.with_backend(BentoNer::new(base_url.clone(), model.clone())?);
        }
        #[cfg(feature = "test-utils")]
        NerBackendConfig::Mock => {
            builder = builder.with_mock_backend();
        }
        // `NerBackendConfig` is `#[non_exhaustive]`. A future
        // variant reaching this arm should surface as a
        // Validation error rather than silently dropping the
        // recognizer.
        _ => {
            return Err(Error::new(
                elide_core::ErrorKind::Validation,
                format!(
                    "NER recognizer `{}` uses a backend kind this engine binary \
                     doesn't understand; upgrade the engine or downgrade the config",
                    spec.name,
                ),
            ));
        }
    }
    Ok(analyzer.with_recognizer(builder.build()?))
}
