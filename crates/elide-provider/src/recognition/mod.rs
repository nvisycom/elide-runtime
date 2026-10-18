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

mod availability;
mod compile;
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
    pub use super::availability::{Availability, ComponentSelection, ResolvedComponents};
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
use elide::{Orchestrator, Result};
use elide_governance::recognition::Recognition;

pub(crate) use self::availability::Resolver;
use self::compile::Compile;
/// What compiling a request's analyzers produced: the orchestrator
/// to analyze with, and the tally its model calls record into.
pub use self::compile::UsageHandle;
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
pub fn analyzers(
    recognizers: &Recognizers,
    enrichers: &Enrichers,
    recognition: &[Recognition],
    availability: &Availability,
    selection: &ComponentSelection,
) -> Result<Analyzers> {
    let cx = Compile::new(recognizers, enrichers, recognition, availability, selection);
    let resolved = cx.resolve()?;

    let orchestrator = Orchestrator::new()
        .with_analyzer::<Text>(compile_text(&resolved)?)
        .with_analyzer::<Tabular>(compile_tabular(&resolved)?)
        .with_analyzer::<Image>(compile_image(&resolved)?)
        .with_analyzer::<Audio>(compile_audio(&resolved)?);

    // The usage sinks are already wired into the backends, so what
    // the orchestrator records lands in this request's tally. Handing
    // the tally back beside it keeps the caller from needing to know
    // that accounting happened at all.
    Ok(Analyzers {
        orchestrator,
        usage: cx.usage_handle(),
    })
}

/// A request's analyzers, and where its model calls are recorded.
///
/// Returned together because they are made together: the sinks are
/// wired into the backends the orchestrator owns, so the tally only
/// means anything beside the orchestrator that fills it.
pub struct Analyzers {
    /// The orchestrator to analyze with.
    pub orchestrator: Orchestrator,
    /// This request's usage tally, readable once the analysis has
    /// run.
    pub usage: UsageHandle,
}
