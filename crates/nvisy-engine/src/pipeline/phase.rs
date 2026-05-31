//! Per-document pipeline phase abstraction.
//!
//! [`Phase<M>`] unifies the operations the orchestrator runs against a
//! single [`DocumentEnvelope<M>`]. Each phase wraps its slice of the
//! shared [`RunContext`] and the per-request [`EngineInput`] at
//! construction time (via [`Phase::Params`] + [`PhaseContext`]) and
//! exposes a uniform `run` entry point.
//!
//! The orchestrator builds a `Vec<Box<dyn Phase<M>>>` once per
//! envelope in fixed order, then walks it. Phase ordering lives in
//! the `Vec` construction site, not in the loop body — misordering a
//! phase shows up as a reviewable edit in one place.
//!
//! Ingestion (`Importer`) and export (`Exporter`) deliberately stay
//! outside this trait: ingestion runs *before* any envelope exists
//! (its output is the set of envelopes), and export operates over a
//! list of [`ExportFile`] configs read-only on the finalised
//! envelope. Both have well-defined orchestrator-level boundaries
//! today; folding them into `Phase<M>` would require a wider
//! contract.
//!
//! [`DocumentEnvelope<M>`]: crate::envelope::DocumentEnvelope
//! [`RunContext`]: super::orchestrator::RunContext
//! [`EngineInput`]: super::default::EngineInput
//! [`ExportFile`]: crate::ingestion::ExportFile

use std::marker::PhantomData;

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ontology::modality::{Audio, Image, Modality, Tabular, Text};

use super::orchestrator::RunContext;
use super::plan::Plan;
use crate::envelope::DocumentEnvelope;

/// Per-run state every phase reads from when running.
///
/// Held as a short-lived borrow inside the orchestrator's phase
/// loop. Phases pull their per-call config from `plan` and shared
/// run state from `run`; persistent handles (registries, engines)
/// the phase needs across calls live on the phase struct itself, not
/// in this context.
pub struct PhaseContext<'a, M: Modality> {
    /// Per-run shared infrastructure (extractors registry, detection
    /// engine, redaction defaults, cancellation token, run id).
    pub(crate) run: &'a RunContext,
    /// Per-request plan bundling each phase's behaviour knobs.
    pub(crate) plan: &'a Plan,
    _marker: PhantomData<fn() -> M>,
}

impl<'a, M: Modality> PhaseContext<'a, M> {
    pub(crate) fn new(run: &'a RunContext, plan: &'a Plan) -> Self {
        Self {
            run,
            plan,
            _marker: PhantomData,
        }
    }
}

/// Modality tag used in [`PhaseInfo`] for introspection (telemetry,
/// dry-run analyzers). Mirrors the closed set of [`Modality`]
/// implementors; modality-agnostic phases use [`Agnostic`].
///
/// [`Agnostic`]: ModalityKind::Agnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalityKind {
    /// Text-typed phase: `M = Text`.
    Text,
    /// Image-typed phase: `M = Image`.
    Image,
    /// Audio-typed phase: `M = Audio`.
    Audio,
    /// Tabular-typed phase: `M = Tabular`.
    Tabular,
    /// Phase body is the same for every modality.
    Agnostic,
}

impl ModalityKind {
    /// Resolve the [`ModalityKind`] for a concrete `M`. `Agnostic`
    /// when `M` is not one of the four built-in modalities (used by
    /// blanket phase impls).
    #[must_use]
    pub fn of<M: Modality>() -> Self {
        let id = std::any::TypeId::of::<M>();
        if id == std::any::TypeId::of::<Text>() {
            Self::Text
        } else if id == std::any::TypeId::of::<Image>() {
            Self::Image
        } else if id == std::any::TypeId::of::<Audio>() {
            Self::Audio
        } else if id == std::any::TypeId::of::<Tabular>() {
            Self::Tabular
        } else {
            Self::Agnostic
        }
    }

    /// Stable lowercase name suitable for tracing fields.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Tabular => "tabular",
            Self::Agnostic => "agnostic",
        }
    }
}

/// Introspection record describing a phase. Returned from
/// [`Phase::inspect`] for telemetry spans and (future) pre-run
/// analyzers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseInfo {
    /// Stable name used in tracing spans (e.g. `"extraction"`).
    /// Don't rename without auditing log queries.
    pub name: &'static str,
    /// Which modality this phase is specialised for, or
    /// [`ModalityKind::Agnostic`].
    pub modality: ModalityKind,
    /// `true` when the phase mutates [`DocumentEnvelope<M>`]. Today
    /// every `Phase<M>` mutates (the trait method takes
    /// `&mut DocumentEnvelope<M>`), but the field is kept for
    /// future read-only phases (dry-run analyzers, audit-only
    /// rescans) and to document intent at the impl site.
    pub mutating: bool,
}

/// A single per-document pipeline operation.
///
/// Phases are the **public interface** for everything that runs
/// against a [`DocumentEnvelope<M>`]: extraction, detection,
/// deduplication, redaction, validation. There is no separate
/// `Redactor` / `Validator` / `Deduplicator` surface — the phase
/// struct *is* the operation.
///
/// # Shape
///
/// The phase struct holds only the long-lived handles (registries,
/// arcs) it needs across runs; per-call configuration flows in
/// through the [`PhaseContext`] handed to [`Phase::run`]. Adding a
/// new phase is:
///
/// 1. Define `FooPhase` carrying its persistent handles
/// 2. `impl Phase<M> for FooPhase` reading its config from
///    `ctx.plan` and any shared state from `ctx.run` inside `run`
/// 3. Push it in `Orchestrator::build_phases`
///
/// No per-phase Params type, no `From` impl, no per-phase
/// constructor on the trait — the lighter shape is intentional, and
/// keeps `Phase<M>` object-safe so the orchestrator can iterate
/// `Vec<Box<dyn Phase<M>>>` directly.
#[async_trait]
pub trait Phase<M: Modality>: Send + Sync {
    /// Stable introspection record. Called by the orchestrator to
    /// emit a uniform tracing span around [`Self::run`] and by
    /// future pre-run analyzers.
    fn inspect(&self) -> PhaseInfo;

    /// Run the phase against the envelope. Phases pull per-call
    /// config from `ctx.plan` (and any shared run state from
    /// `ctx.run`) and mutate the envelope in place. Cancellation is
    /// checked by the orchestrator *between* phases, not inside them.
    async fn run(
        &self,
        ctx: &PhaseContext<'_, M>,
        envelope: &mut DocumentEnvelope<M>,
    ) -> Result<()>;
}
