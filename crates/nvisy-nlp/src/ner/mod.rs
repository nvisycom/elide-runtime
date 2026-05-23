//! [`NerBackend`] trait and built-in implementations.
//!
//! The trait is the abstraction every NER source plugs into. Backends
//! consume `&str` plus an optional language hint and return
//! [`Entities`] with `confidence` populated. Filtering by confidence
//! threshold and entity-kind allowlist is the caller's responsibility
//! — backends return everything they detect.
//!
//! [`Entities`]: nvisy_ontology::entity::Entities

#[cfg(any(test, feature = "test-utils"))]
mod noop;
mod ort;

use async_trait::async_trait;
use nvisy_ontology::entity::{Entities, EntityKind};
use nvisy_ontology::primitive::LanguageTag;

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use self::noop::NoopNerBackend;
pub use self::ort::{OrtNerBackend, OrtNerConfig, id_to_label_from_config_json};
use crate::error::Result;

/// Recognize entities in text.
///
/// Async because the realistic implementation paths — ONNX inference
/// on a blocking pool, future LLM calls — need to yield. Pure-CPU
/// backends wrap the body in `async {}` cheaply.
///
/// `language` is **advisory**. Multilingual models may ignore it;
/// monolingual models should validate it against
/// [`supported_languages`] and may return
/// [`Error::UnsupportedLanguage`] if the hint disagrees.
///
/// [`supported_languages`]: Self::supported_languages
/// [`Error::UnsupportedLanguage`]: crate::Error::UnsupportedLanguage
#[async_trait]
pub trait NerBackend: Send + Sync {
    /// Recognize entities in `text`.
    async fn recognize(&self, text: &str, language: Option<&LanguageTag>) -> Result<Entities>;

    /// Languages this backend was trained or configured for.
    ///
    /// An empty slice means the backend accepts any language (treat
    /// as universal).
    fn supported_languages(&self) -> &[LanguageTag] {
        &[]
    }

    /// Entity kinds this backend can produce.
    ///
    /// An empty slice means the backend may produce any kind it can
    /// detect — used when the label space is open or large enough to
    /// make enumeration unhelpful.
    fn supported_kinds(&self) -> &[EntityKind] {
        &[]
    }
}
