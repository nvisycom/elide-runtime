//! Attach the deployment's NER lineup to a per-modality
//! [`Analyzer`]. Walks [`NerConfig::recognizers`] and builds one
//! elide `NerRecognizer` per entry via `elide-bento`'s `BentoNer`
//! backend (or `MockBackend` under the `test-utils` feature).
//!
//! Every configured recognizer attaches to every request; the
//! deployment picks the lineup at `Engine::with_ner` time.
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`NerConfig::recognizers`]: crate::provider::ner::NerConfig::recognizers

use elide::detection::Analyzer;
use elide::recognition::ner::NerRecognizer;
use elide_bento::ner::BentoNer;
use elide_core::Result;
use elide_core::modality::TextRecognizable;
use elide_core::recognition::Recognizer;

use crate::provider::ner::{NerBackend, NerConfig, NerRecognizerConfig};

/// Attach every recognizer in `ner` to `analyzer`.
pub(in crate::analyzer) fn attach_ner_lineup<M>(
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
