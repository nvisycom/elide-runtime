//! [`BentoBackend`] — externalised OCR over an `inference-ocr`
//! Bento in [`nvisycom/inference`].
//!
//! **Scaffolding only.** The wire contract has not been finalised
//! upstream (tracked under [#128]). The struct, parameters, and
//! [`Backend`] impl exist so config wiring and feature gates can
//! land now and the externalised path becomes a one-file change
//! when the contract is ready. Until then [`Backend::run`] returns
//! a clear runtime error rather than pretending to call the
//! service.
//!
//! Once the wire types are pinned, this file mirrors
//! [`crate::backend::bento_types`] the same way `nvisy-ner`'s
//! `BentoBackend` mirrors `nvisy_core.ner.v1`.
//!
//! [`Backend`]: crate::core::Backend
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference
//! [#128]: https://github.com/nvisycom/runtime/issues/128

use async_trait::async_trait;
use bentoml::prelude::*;
use nvisy_core::Error;
use nvisy_ontology::document::Block;
use nvisy_ontology::entity::{ModelKind, ModelProvenance};
use nvisy_ontology::modality::Image;

use crate::core::{Backend, Context, ImageInput};

/// Parameters for [`BentoBackend`].
#[derive(Debug, Clone)]
pub struct BentoParams {
    /// Base URL of the `inference-ocr` Bento (e.g. `http://localhost:3001`).
    pub base_url: String,
}

impl BentoParams {
    /// Construct with the given service URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

/// [`Backend`] that will call an externalised OCR Bento over HTTP.
///
/// **Not yet functional.** Both [`Backend::run`] and
/// [`Backend::run_batch`] return a clear runtime error until the
/// inference repo finalises the OCR wire contract — see [#128].
///
/// Constructing one still validates the configured `base_url`
/// (via the underlying [`Client`] builder) so config errors
/// surface at startup rather than at the first request, even
/// though the request path itself is stubbed out.
///
/// [`Backend`]: crate::core::Backend
#[derive(Debug)]
pub struct BentoBackend;

impl BentoBackend {
    /// Build a backend against the given parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client cannot be
    /// constructed (invalid `base_url`).
    pub fn new(params: BentoParams) -> Result<Self, Error> {
        // Validate the URL eagerly so misconfiguration surfaces at
        // startup. The constructed client is discarded today; once
        // the wire contract lands it moves to a field and the
        // request path picks it up.
        Client::builder()
            .with_base_url(&params.base_url)
            .build()
            .map_err(|e| Error::runtime(format!("bentoml client init: {e}"), "ocr-bento", false))?;
        Ok(Self)
    }
}

#[async_trait]
impl Backend for BentoBackend {
    fn provenance(&self) -> ModelProvenance {
        ModelProvenance::new("bento-ocr", ModelKind::SelfHosted)
    }

    async fn run(
        &self,
        _image: &ImageInput,
        _ctx: Context<'_>,
    ) -> Result<Vec<Block<Image>>, Error> {
        Err(Error::runtime(
            "BentoBackend is scaffolded; the inference-ocr wire \
             contract has not been finalised yet — see \
             https://github.com/nvisycom/runtime/issues/128",
            "ocr-bento",
            false,
        ))
    }
}
