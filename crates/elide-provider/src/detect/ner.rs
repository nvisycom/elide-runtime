//! NER recognizers, one per configured component.

use elide::Result;
use elide::detection::Analyzer;
use elide::modality::TextRecognizable;
use elide::recognition::Recognizer;
use elide::recognition::ner::NerRecognizer;
#[cfg(feature = "mocks")]
use elide::recognition::ner::backend::MockBackend as MockNerBackend;
use elide_bentoml::ner::BentoNer;
use elide_deployment::component::{Component, NerBackend as NerBackendConfig};
#[cfg(feature = "pioneer")]
use elide_pioneer_gliner::PioneerNer;

use super::decorate::{Decorated, decorate};
use crate::detect::NerBackend;
use crate::request::Resolved;

pub(super) fn attach_ner_lineup<M>(
    mut analyzer: Analyzer<M>,
    ner: &[Component<NerBackendConfig>],
    resolved: &Resolved<'_>,
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    NerRecognizer<Decorated<BentoNer>>: Recognizer<M> + 'static,
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
    NerRecognizer<Decorated<BentoNer>>: Recognizer<M> + 'static,
{
    // The recognizer carries its backend as a type parameter, so each
    // arm attaches a differently-typed recognizer rather than folding
    // into one builder.
    Ok(match &spec.backend {
        NerBackend::Bento { base_url, model } => analyzer.with_recognizer(
            NerRecognizer::new(decorate(
                BentoNer::new(base_url.clone(), model.clone())?,
                spec.rate_limit.as_ref(),
                resolved.sink(),
            ))
            .with_name(spec.name.clone()),
        ),
        #[cfg(feature = "pioneer")]
        NerBackend::Pioneer {
            api_key,
            base_url,
            threshold,
            zero_retention,
        } => {
            let mut backend = match base_url {
                Some(url) => PioneerNer::with_base_url(api_key.clone(), url.clone())?,
                None => PioneerNer::new(api_key.clone())?,
            };
            if let Some(threshold) = threshold {
                backend = backend.with_threshold(*threshold);
            }
            if *zero_retention {
                backend = backend.with_zero_retention();
            }
            analyzer.with_recognizer(
                NerRecognizer::new(decorate(backend, spec.rate_limit.as_ref(), resolved.sink()))
                    .with_name(spec.name.clone()),
            )
        }
        #[cfg(feature = "mocks")]
        NerBackend::Mock => analyzer.with_recognizer(
            NerRecognizer::new(decorate(
                MockNerBackend::default(),
                spec.rate_limit.as_ref(),
                resolved.sink(),
            ))
            .with_name(spec.name.clone()),
        ),
    })
}
