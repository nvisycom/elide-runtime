//! [`NerBackend`] trait and built-in implementations.
//!
//! The trait is the abstraction every NER source plugs into. Backends
//! consume `&str` plus a [`NerParams`] hint and return [`Entities`]
//! with `confidence` populated. Filtering by confidence threshold and
//! entity-kind allowlist is the caller's responsibility — backends
//! return everything they detect.
//!
//! Two backends ship today:
//! - [`NoopBackend`] — returns no entities. Used by tests and by
//!   deployments that detect via patterns / LLM only.
//! - [`BentoNerBackend`] (feature `bento`) — calls the externalized
//!   `inference-gliner` Bento in [`nvisycom/inference`] over HTTP.
//!   The service owns the model and the label-map translation; the
//!   runtime just forwards text + requested kinds and propagates a
//!   `correlation_id` as the `x-request-id` header.
//!
//! In-process backends (BERT-NER over `ort`, GLiNER via `gline-rs`)
//! lived here previously and have been removed in favour of the
//! externalized inference service.
//!
//! [`Entities`]: nvisy_ontology::entity::Entities
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference

#[cfg(feature = "bento")]
mod bento_backend;
#[cfg(feature = "bento")]
mod bento_types;
mod noop_backend;

use async_trait::async_trait;
use nvisy_ontology::entity::{Entities, EntityKind};
use nvisy_ontology::primitive::LanguageTag;

#[cfg(feature = "bento")]
#[cfg_attr(docsrs, doc(cfg(feature = "bento")))]
pub use self::bento_backend::{BentoNerBackend, BentoNerParams};
pub use self::noop_backend::NoopBackend;
use crate::error::Result;

/// Per-call hints passed alongside the input text to a [`NerBackend`].
///
/// Both fields are advisory — backends are free to ignore either when
/// their model doesn't expose the corresponding knob. Packed into a
/// struct so the trait stays stable as new hint kinds are added.
#[derive(Debug, Default, Clone, Copy)]
pub struct NerParams<'a> {
    /// Caller-resolved language for the input. Multilingual models
    /// may ignore it; monolingual models may validate it against a
    /// configured allowlist and return [`Error::UnsupportedLanguage`]
    /// when the hint disagrees.
    ///
    /// [`Error::UnsupportedLanguage`]: crate::Error::UnsupportedLanguage
    pub language: Option<&'a LanguageTag>,

    /// Entity kinds the caller is interested in. Backends with a
    /// zero-shot label vector (e.g. GLiNER) shape inference around
    /// this; backends with a fixed label set ignore it. The
    /// [`NlpEngine`] post-filters output against the same allowlist
    /// either way.
    ///
    /// [`NlpEngine`]: crate::NlpEngine
    pub requested_kinds: Option<&'a [EntityKind]>,
}

impl<'a> NerParams<'a> {
    /// Construct an empty params (no language hint, no kind filter).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style setter for the language hint.
    pub fn with_language(mut self, language: &'a LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Builder-style setter for the requested-kinds hint.
    pub fn with_requested_kinds(mut self, kinds: &'a [EntityKind]) -> Self {
        self.requested_kinds = Some(kinds);
        self
    }
}

/// Recognize entities in text.
///
/// Async because the realistic implementation paths — HTTP calls to
/// an externalized inference service, future LLM calls — need to
/// yield. Pure-CPU backends wrap the body in `async {}` cheaply.
///
/// Implementors **must** provide [`recognize`]. The default
/// [`recognize_batch`] impl serialises over the inputs by calling
/// [`recognize`] in a loop; backends that can natively batch a
/// single network round-trip (such as [`BentoNerBackend`]) should
/// override it.
///
/// Batch entries share a single [`NerParams`] — the typical caller
/// is post-tokenisation chunking from one document, so the language
/// hint and requested-kinds list apply uniformly. Mixed-context
/// inputs should be issued as separate batches.
///
/// [`recognize`]: Self::recognize
/// [`recognize_batch`]: Self::recognize_batch
#[async_trait]
pub trait NerBackend: Send + Sync {
    /// Recognize entities in `text` under `params`.
    async fn recognize(&self, text: &str, params: NerParams<'_>) -> Result<Entities>;

    /// Recognize entities in each of `texts` under one shared
    /// [`NerParams`].
    ///
    /// The returned vec is in the same order as `texts`. The
    /// default impl loops over [`recognize`]; backends with native
    /// batching override it to issue one round-trip.
    ///
    /// [`recognize`]: Self::recognize
    async fn recognize_batch(
        &self,
        texts: &[&str],
        params: NerParams<'_>,
    ) -> Result<Vec<Entities>> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.recognize(text, params).await?);
        }
        Ok(out)
    }
}
