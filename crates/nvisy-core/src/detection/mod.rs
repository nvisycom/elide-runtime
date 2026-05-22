//! [`Recognizer`] trait and shared per-call detection config.
//!
//! A recognizer is the unit of entity-detection plug-in: it consumes
//! a typed per-call context (chosen by the impl as its associated
//! [`Recognizer::Context`]) and produces [`Entities`].
//!
//! The trait carries the *bare minimum* needed to dispatch:
//! associated context type, async `run`, optional `reset` for
//! cross-call state. Orchestration, parallel dispatch, and the
//! wider per-call context type live in downstream crates.
//!
//! Each backend defines its own context shape next to its
//! recognizer adapter. A bridge trait downstream converts a
//! universal "fat" context into each adapter's typed slice via
//! `From<&FatContext>`.
//!
//! [`Entities`]: nvisy_ontology::entity::Entities

mod params;

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;

pub use self::params::DetectionParams;
use crate::Result;

/// Recognize entities given a per-recognizer context.
///
/// `Context` is associated rather than a trait parameter so each
/// impl declares exactly what it consumes — pattern recognizers
/// need allow/deny lists, NER recognizers need language hints, LLM
/// recognizers need their own per-call config. No common context
/// type appears in this trait.
///
/// Implementations are independent — orchestrators run each on its
/// own and merge results.
///
/// Async because realistic impls dispatch to ONNX inference on a
/// blocking pool or call remote services.
#[async_trait]
pub trait Recognizer: Send + Sync {
    /// The per-call context this recognizer consumes.
    type Context;

    /// Detect entities in `ctx`. Offsets in returned entities are
    /// relative to whatever text `ctx` carries — callers rebase
    /// when integrating into a larger document.
    async fn run(&self, ctx: &Self::Context) -> Result<Entities>;

    /// Reset per-document state, called by the orchestrator at
    /// document boundaries. The default is a no-op — stateless
    /// recognizers don't need to override it.
    ///
    /// LLM-backed recognizers override this to clear coreference
    /// state between documents so per-document entity references
    /// don't bleed across runs.
    async fn reset(&self) {}
}
