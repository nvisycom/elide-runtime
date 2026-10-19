//! NER recognizers, one per configured component.

use elide::Result;
use elide::backend::metrics::Metered;
use elide::detection::Analyzer;
use elide::modality::TextRecognizable;
use elide::recognition::Recognizer;
use elide::recognition::ner::NerRecognizer;
#[cfg(feature = "mocks")]
use elide::recognition::ner::backend::MockBackend as MockNerBackend;
use elide_bentoml::ner::BentoNer;

use crate::component::{Component, NerBackend as NerBackendConfig};
use crate::detect::NerBackend;
use crate::request::Resolved;

pub(super) fn attach_ner_lineup<M>(
    mut analyzer: Analyzer<M>,
    ner: &[Component<NerBackendConfig>],
    resolved: &Resolved<'_>,
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    NerRecognizer<Metered<BentoNer>>: Recognizer<M> + 'static,
{
    for recognizer in ner {
        analyzer = attach_ner_one(analyzer, recognizer, resolved)?;
    }
    Ok(analyzer)
}

fn attach_ner_one<M>(
    analyzer: Analyzer<M>,
    spec: &Component<NerBackend>,
    resolved: &Resolved<'_>,
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    // One bound per backend the config can name: the recognizer
    // carries its backend as a type parameter, so each is a distinct
    // type that has to implement `Recognizer<M>` in its own right.
    //
    // The `Mock` arm needs no bound of its own. `#[cfg]` on a
    // where-clause predicate is still unstable (rust#115590), so a
    // bound naming the mock would have to be either unconditional —
    // which fails to resolve when `mocks` is off, since the type is
    // gated upstream — or duplicated across a cfg'd pair of
    // signatures. Neither is necessary: every caller in `modality`
    // instantiates a concrete `Analyzer<Text>`/`Analyzer<Image>`, so
    // the mock recognizer's own impl is what satisfies it there.
    NerRecognizer<Metered<BentoNer>>: Recognizer<M> + 'static,
{
    // The recognizer carries its backend as a type parameter, so each
    // arm attaches a differently-typed recognizer rather than folding
    // into one builder.
    Ok(match &spec.backend {
        NerBackend::Bento { base_url, model } => analyzer.with_recognizer(
            NerRecognizer::new(Metered::new(
                BentoNer::new(base_url.clone(), model.clone())?,
                resolved.sink(),
            ))
            .with_name(spec.name.clone()),
        ),
        #[cfg(feature = "mocks")]
        NerBackend::Mock => analyzer.with_recognizer(
            NerRecognizer::new(Metered::new(MockNerBackend::default(), resolved.sink()))
                .with_name(spec.name.clone()),
        ),
    })
}
