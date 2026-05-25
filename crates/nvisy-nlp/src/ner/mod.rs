//! [`NerBackend`] trait and built-in implementations.
//!
//! The trait is the abstraction every NER source plugs into. Backends
//! consume `&str` plus an optional language hint and return
//! [`Entities`] with `confidence` populated. Filtering by confidence
//! threshold and entity-kind allowlist is the caller's responsibility
//! — backends return everything they detect.
//!
//! Two backends ship today:
//! - [`NoopBackend`] — returns no entities. Used by tests and by
//!   deployments that detect via patterns / LLM only.
//! - **HTTP backend** (planned, in a follow-up PR) — calls an
//!   externalized inference service that hosts the actual model.
//!
//! In-process backends (BERT-NER over `ort`, GLiNER via `gline-rs`)
//! lived here previously and have been removed. They are slated to
//! return as opt-in features once the upstream `ort` 2.0 line
//! stabilises — see issues #192 and #193.
//!
//! [`Entities`]: nvisy_ontology::entity::Entities

mod noop_backend;

use async_trait::async_trait;
use nvisy_ontology::entity::{Entities, EntityKind};
use nvisy_ontology::primitive::LanguageTag;

pub use self::noop_backend::NoopBackend;
use crate::error::Result;

/// Recognize entities in text.
///
/// Async because the realistic implementation paths — HTTP calls to
/// an externalized inference service, future LLM calls — need to
/// yield. Pure-CPU backends wrap the body in `async {}` cheaply.
///
/// `language` is **advisory**. Multilingual models may ignore it;
/// monolingual models may validate it against a configured allowlist
/// and return [`Error::UnsupportedLanguage`] if the hint disagrees.
///
/// `requested_kinds` is **also advisory** — backends that can shape
/// their inference around it (zero-shot models) may use it; backends
/// with a fixed label vector ignore it. The [`NlpEngine`] still
/// post-filters output against the same allowlist.
///
/// [`Error::UnsupportedLanguage`]: crate::Error::UnsupportedLanguage
/// [`NlpEngine`]: crate::NlpEngine
#[async_trait]
pub trait NerBackend: Send + Sync {
    /// Recognize entities in `text`.
    async fn recognize(
        &self,
        text: &str,
        language: Option<&LanguageTag>,
        requested_kinds: Option<&[EntityKind]>,
    ) -> Result<Entities>;
}
