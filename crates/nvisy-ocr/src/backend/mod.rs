//! Built-in [`Backend`] implementations plus the [`OcrBackend`]
//! config enum that dispatches to a concrete one.
//!
//! Two backends ship today:
//! - [`NoopOcrBackend`] — returns no OCR results. The default; used
//!   in tests and in deployments that don't need OCR.
//! - [`BentoOcrBackend`] (feature `bento`) — scaffolding for the
//!   externalised `inference-ocr` Bento in
//!   [`nvisycom/inference`]. Not yet functional; tracked under
//!   [#128].
//!
//! Cloud OCR backends (AWS Textract, Google Cloud Vision, Azure
//! Document Intelligence) lived here previously and have been
//! removed to clear the deck for the externalised inference
//! architecture. Reintroduction is tracked under [#201] / [#202] /
//! [#203].
//!
//! [`Backend`]: crate::core::Backend
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference
//! [#128]: https://github.com/nvisycom/runtime/issues/128
//! [#201]: https://github.com/nvisycom/runtime/issues/201
//! [#202]: https://github.com/nvisycom/runtime/issues/202
//! [#203]: https://github.com/nvisycom/runtime/issues/203

#[cfg(feature = "bento")]
mod bento_backend;
#[cfg(feature = "bento")]
mod bento_types;
mod noop_backend;

use nvisy_core::Error;
use serde::{Deserialize, Serialize};

#[cfg(feature = "bento")]
#[cfg_attr(docsrs, doc(cfg(feature = "bento")))]
pub use self::bento_backend::{BentoOcrBackend, BentoOcrParams};
pub use self::noop_backend::NoopOcrBackend;
use crate::engine::OcrEngine;

/// Config-side selection of which [`Backend`] to construct.
///
/// The enum is always parseable regardless of compiled features —
/// selecting [`OcrBackend::Bento`] on a build without the `bento`
/// feature surfaces as a clear runtime error at construction
/// instead of a deserialisation failure, so config files stay
/// portable across deployments.
///
/// [`Backend`]: crate::core::Backend
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OcrBackend {
    /// No-op backend — produces zero OCR results. The default;
    /// used in tests and in deployments that don't need OCR.
    #[default]
    Noop,

    /// Externalised [`BentoOcrBackend`] — calls the
    /// `inference-ocr` Bento over HTTP. Requires the runtime to
    /// be built with the `bento` feature.
    ///
    /// **Scaffolding only** until the wire contract is finalised
    /// upstream — see [#128].
    ///
    /// [`BentoOcrBackend`]: BentoOcrBackend
    /// [#128]: https://github.com/nvisycom/runtime/issues/128
    Bento {
        /// Base URL of the `inference-ocr` Bento (for example
        /// `http://localhost:3001` or `http://inference-ocr:3001`
        /// inside a docker-compose network).
        base_url: String,
    },
}

impl OcrBackend {
    /// Build an [`OcrEngine`] from this config selection.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected backend cannot be
    /// constructed, or if the config selects a backend whose
    /// feature wasn't compiled in.
    pub fn into_engine(self) -> Result<OcrEngine, Error> {
        match self {
            Self::Noop => Ok(OcrEngine::new(NoopOcrBackend)),

            #[cfg(feature = "bento")]
            Self::Bento { base_url } => {
                let backend = BentoOcrBackend::new(BentoOcrParams::new(base_url))?;
                Ok(OcrEngine::new(backend))
            }

            #[cfg(not(feature = "bento"))]
            Self::Bento { .. } => Err(Error::validation(
                "OcrBackend::Bento requires nvisy-ocr to be built with the `bento` feature",
                "ocr",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ImageFormat, ImageInput, OcrParams};
    use crate::core::Backend;

    #[tokio::test]
    async fn noop_returns_empty() {
        let backend = NoopOcrBackend::new();
        let image = ImageInput::new(vec![0u8; 8], ImageFormat::Png);
        let out = backend.run(&image, OcrParams::default()).await.unwrap();
        assert_eq!(out.pages.len(), 0);
    }

    #[test]
    fn ocr_backend_into_engine_noop() {
        assert!(OcrBackend::Noop.into_engine().is_ok());
    }

    #[cfg(not(feature = "bento"))]
    #[test]
    fn ocr_backend_bento_without_feature_errors_clearly() {
        let err = OcrBackend::Bento {
            base_url: "http://localhost:3001".into(),
        }
        .into_engine()
        .unwrap_err();
        assert!(
            err.to_string().contains("`bento` feature"),
            "error should mention the bento feature: {err}",
        );
    }
}
