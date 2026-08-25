//! Recognition: which entities to find, and how.
//!
//! Split by what a component does. A **recognizer** produces
//! entities; an **enricher** produces the context recognizers read,
//! running before them to stamp a language hint, OCR'd text layout,
//! or audio transcript segments onto the request.
//!
//! Each group holds one module per backend, carrying the
//! deployment's own configuration — which NER model, which LLM
//! provider, which OCR and STT engines — beside the `compile` step
//! that turns those lineups into an [`elide::detection::Analyzer`]
//! per modality. Config and compile live together because they
//! change together: adding a backend means a config type *and* the
//! code that reads it.
//!
//! Mirrors the crate's redaction side, which does the same for the
//! other direction: where recognition finds entities, redaction
//! hides them.
//!
//! Scope is **not** per-modality: [`Scope`] is modality-free and is
//! built once in the orchestrator builder, then attached to the
//! [`Orchestrator`] via [`Orchestrator::with_scope`].
//!
//! [`Scope`]: elide::recognition::Scope
//!
//! ## Per-modality coverage
//!
//! | Modality | Pattern | NER | LLM |
//! |----------|---------|-----|-----|
//! | Text     | yes     | yes | yes |
//! | Tabular  | yes     | yes | (no upstream `LlmModality` impl) |
//! | Image    | yes     | yes | yes |
//! | Audio    | yes     | yes | (no upstream `LlmModality` impl) |
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`Orchestrator`]: elide::Orchestrator
//! [`Orchestrator::with_scope`]: elide::Orchestrator::with_scope

mod component;
mod enrichers;
mod layer;
mod modality;
mod recognizers;

/// What a deployment declares about recognition, as data.
///
/// Every type here is `Deserialize` + `JsonSchema`: a config file
/// parses straight into [`Recognizers`] and [`Enrichers`], and a
/// schema can be generated for whatever writes them. Nothing in
/// this module compiles anything — turning a lineup into an
/// analyzer is [`analyzers`](super::analyzers), which
/// needs the elide runtime and stays out of the config vocabulary.
pub mod config {
    pub use super::component::{Backend, Component};
    pub use super::enrichers::{Enrichers, OcrBackend, SttBackend};
    pub use super::recognizers::{
        AttachTo, AuthenticatedProvider, LlmBackend, LlmSource, NerBackend, Recognizers,
        UnauthenticatedProvider,
    };
}

use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::{Error, ErrorKind, Orchestrator, Result};

pub use self::config::*;
use self::modality::{compile_audio, compile_image, compile_tabular, compile_text};

/// An [`Orchestrator`] that can analyze any of the four modalities,
/// built from the deployment's `recognizers` and `enrichers`.
///
/// The whole recognition side in one call: the caller supplies the
/// lineups and gets back something ready to analyze. How a lineup
/// becomes an [`Analyzer`](elide::detection::Analyzer) — which
/// backends attach in which order, and what each modality supports
/// — stays in this module.
///
/// Only the analyzer half is set. Redaction is the other direction
/// and arrives per request as policies, so `with_analyzer` leaves
/// each anonymizer at its default rather than building four that go
/// unused.
///
/// The caller adds what is not recognition's business: a
/// [`FormatRegistry`](elide::codec::FormatRegistry) for decoding
/// container parts, and the per-request
/// [`Scope`](elide::recognition::Scope). Both belong to the request
/// or the codec layer, where these lineups are per deployment.
///
/// # Errors
///
/// Returns [`Configuration`](ErrorKind::Configuration) if a backend
/// cannot be built from its config, or if an enricher lineup names
/// more than one entry — elide attaches at most one per analyzer.
pub fn analyzers(recognizers: &Recognizers, enrichers: &Enrichers) -> Result<Orchestrator> {
    let ner = &recognizers.ner;
    let llm = &recognizers.llm;
    let ocr = pick_one(&enrichers.ocr, "OCR")?;
    let stt = pick_one(&enrichers.stt, "STT")?;

    Ok(Orchestrator::new()
        .with_analyzer::<Text>(compile_text(ner, llm)?)
        .with_analyzer::<Tabular>(compile_tabular(ner)?)
        .with_analyzer::<Image>(compile_image(ner, llm, ocr)?)
        .with_analyzer::<Audio>(compile_audio(ner, stt)?))
}

/// The single enricher a lineup may wire, or `None` for an empty
/// one.
///
/// elide attaches at most one enricher per analyzer, so a lineup
/// naming two is a misconfiguration worth rejecting at request
/// compile rather than silently running the first. `kind` names the
/// lineup in that error.
fn pick_one<'a, B>(lineup: &'a [Component<B>], kind: &str) -> Result<Option<&'a Component<B>>> {
    match lineup {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "{kind} enricher lineup carries {} entries; elide attaches at most \
                 one per analyzer. Wire exactly one enricher.",
                many.len(),
            ),
        )),
    }
}
