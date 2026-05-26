//! Built-in [`Backend`] implementations plus the [`NerBackend`]
//! config enum that dispatches to a concrete one.
//!
//! Two backends ship today:
//! - [`NoopBackend`] — returns no entities. The default; used by
//!   tests and by deployments that detect via patterns / LLM only.
//! - [`BentoBackend`] (feature `bento`) — calls the externalised
//!   `inference-gliner` Bento in [`nvisycom/inference`] over HTTP.
//!   The service owns the model and the label-map translation; the
//!   runtime just forwards text + requested kinds and propagates a
//!   `correlation_id` as the `x-request-id` header.
//!
//! In-process backends (BERT-NER over `ort`, GLiNER via `gline-rs`)
//! lived here previously and have been removed in favour of the
//! externalised inference service.
//!
//! [`Backend`]: crate::core::Backend
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference

#[cfg(feature = "bento")]
mod bento_backend;
#[cfg(feature = "bento")]
mod bento_types;
mod noop_backend;

use nvisy_core::Result;
use serde::{Deserialize, Serialize};

#[cfg(feature = "bento")]
#[cfg_attr(docsrs, doc(cfg(feature = "bento")))]
pub use self::bento_backend::{BentoBackend, BentoParams};
pub use self::noop_backend::NoopBackend;
use crate::RecognizerBuilder;

/// Config-side selection of which [`Backend`] to construct.
///
/// The enum is always parseable regardless of compiled features —
/// selecting [`NerBackend::Bento`] on a build without the `bento`
/// feature surfaces as a clear runtime error at construction
/// instead of a deserialisation failure, so config files stay
/// portable across deployments.
///
/// [`Backend`]: crate::core::Backend
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NerBackend {
    /// No-op backend — produces zero entities. The default; used
    /// by tests and by deployments that detect via patterns/LLM
    /// only.
    #[default]
    Noop,

    /// Externalised [`BentoBackend`] — calls the `inference-gliner`
    /// Bento over HTTP. Requires the runtime to be built with the
    /// `bento` feature.
    Bento {
        /// Base URL of the `inference-gliner` Bento (for example
        /// `http://localhost:3000` or
        /// `http://inference-gliner:3000` inside a docker-compose
        /// network).
        base_url: String,
    },
}

impl NerBackend {
    /// Attach the [`Backend`] this selection identifies to
    /// `builder`.
    ///
    /// Unlike OCR's [`OcrBackend::into_extractor`], the NER
    /// recognizer requires both a backend *and* a language policy
    /// at the same time. Rather than thread the policy through
    /// here, this helper takes the partially-configured builder
    /// and attaches just the NER backend, leaving policy
    /// attachment to the caller.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected backend cannot be
    /// constructed, or if the config selects a backend whose
    /// feature wasn't compiled in.
    ///
    /// [`Backend`]: crate::core::Backend
    /// [`OcrBackend::into_extractor`]: https://docs.rs/nvisy-ocr/latest/nvisy_ocr/backend/enum.OcrBackend.html#method.into_extractor
    pub fn attach_ner_backend(&self, builder: RecognizerBuilder) -> Result<RecognizerBuilder> {
        match self {
            Self::Noop => Ok(builder.with_ner_backend(NoopBackend)),

            #[cfg(feature = "bento")]
            Self::Bento { base_url } => {
                let backend = BentoBackend::new(BentoParams::new(base_url.clone()))?;
                Ok(builder.with_ner_backend(backend))
            }

            #[cfg(not(feature = "bento"))]
            Self::Bento { .. } => Err(nvisy_core::Error::validation(
                "NerBackend::Bento requires nvisy-ner to be built with the `bento` feature",
                "ner",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Backend, Context};

    #[tokio::test]
    async fn noop_returns_empty() {
        let backend = NoopBackend::new();
        let out = backend
            .recognize("anything", &Context::default())
            .await
            .unwrap();
        assert!(out.is_empty());
    }

    #[cfg(not(feature = "bento"))]
    #[test]
    fn ner_backend_bento_without_feature_errors_clearly() {
        let builder = RecognizerBuilder::default();
        let backend = NerBackend::Bento {
            base_url: "http://localhost:3000".into(),
        };
        match backend.attach_ner_backend(builder) {
            Ok(_) => panic!("Bento should not attach without `bento` feature"),
            Err(e) => assert!(
                e.to_string().contains("`bento` feature"),
                "error should mention the bento feature: {e}",
            ),
        }
    }
}
