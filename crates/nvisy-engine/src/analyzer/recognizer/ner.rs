//! Attach the deployment's NER lineup to a per-modality
//! [`Analyzer`]. Walks [`NerConfig::recognizers`] and builds one
//! `NerRecognizer` per entry via `elide-bento`'s `BentoNer`
//! backend (or `MockBackend` under the `test-utils` feature).
//!
//! Symmetric with [`super::llm`]; called by every per-modality
//! compile function with the request's
//! `AnalyzerParams.recognizers.ner` [`ProviderSelection`].
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`NerConfig::recognizers`]: crate::provider::ner::NerConfig::recognizers
//! [`ProviderSelection`]: nvisy_schema::plan::ProviderSelection

use elide::detection::Analyzer;
use elide::recognition::ner::NerRecognizer;
use elide_bento::ner::BentoNer;
use elide_core::modality::TextRecognizable;
use elide_core::recognition::Recognizer;
use elide_core::Error;
use nvisy_schema::plan::ProviderSelection;

use super::selection::select;
use crate::provider::ner::{NerBackend, NerConfig, NerRecognizer as ConfigNerRecognizer};

/// Attach recognizers from the deployment's NER lineup selected
/// by `selection`.
///
/// - `None`: softly-on default. Attaches every configured
///   recognizer if the lineup is non-empty; skips silently
///   otherwise.
/// - `Some(All(true))`: explicit opt-in. Attaches every
///   configured recognizer; errors if the lineup is empty.
/// - `Some(All(false))`: explicit opt-out.
/// - `Some(Only(names))`: attach only the named recognizers.
///   Empty list and unknown names error.
pub(in crate::analyzer) fn attach_ner_lineup<M>(
    mut analyzer: Analyzer<M>,
    ner: &NerConfig,
    selection: Option<&ProviderSelection>,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    NerRecognizer: Recognizer<M> + 'static,
{
    let selected = select(selection, &ner.recognizers, "ner")?;
    for recognizer in selected {
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
