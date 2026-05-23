//! [`NerBackend`] trait and built-in implementations.
//!
//! The trait is the abstraction every NER source plugs into. Backends
//! consume `&str` plus an optional language hint and return
//! [`Entities`] with `confidence` populated. Filtering by confidence
//! threshold and entity-kind allowlist is the caller's responsibility
//! — backends return everything they detect.
//!
//! [`Entities`]: nvisy_ontology::entity::Entities

#[cfg(feature = "gliner")]
mod gliner;
mod noop;
#[cfg(feature = "onnx")]
mod ort;

use async_trait::async_trait;
use nvisy_ontology::entity::{Entities, EntityKind};
use nvisy_ontology::primitive::LanguageTag;

#[cfg(feature = "gliner")]
#[cfg_attr(docsrs, doc(cfg(feature = "gliner")))]
pub use self::gliner::{GlinerBackend, GlinerConfig, GlinerMode};
pub use self::noop::NoopNerBackend;
#[cfg(feature = "onnx")]
#[cfg_attr(docsrs, doc(cfg(feature = "onnx")))]
pub use self::ort::{OrtNerBackend, OrtNerConfig, id_to_label_from_config_json};
use crate::error::Result;

/// Recognize entities in text.
///
/// Async because the realistic implementation paths — ONNX inference
/// on a blocking pool, future LLM calls — need to yield. Pure-CPU
/// backends wrap the body in `async {}` cheaply.
///
/// `language` is **advisory**. Multilingual models may ignore it;
/// monolingual models may validate it against a configured allowlist
/// and return [`Error::UnsupportedLanguage`] if the hint disagrees.
///
/// `requested_kinds` is **also advisory** and exists to let zero-shot
/// backends (notably [`GlinerBackend`]) materialise the exact label
/// set the caller is asking about. Backends with a fixed label vector
/// ([`OrtNerBackend`], [`NoopNerBackend`]) ignore it — the
/// [`Engine`] still post-filters their output against the same
/// allowlist, so passing it here is purely an optimisation hint.
///
/// [`GlinerBackend`]: GlinerBackend
/// [`OrtNerBackend`]: OrtNerBackend
/// [`NoopNerBackend`]: NoopNerBackend
/// [`Error::UnsupportedLanguage`]: crate::Error::UnsupportedLanguage
/// [`Engine`]: crate::Engine
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
