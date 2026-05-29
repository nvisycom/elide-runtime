//! Built-in [`Backend`] implementations plus the [`OcrBackend`]
//! config enum that dispatches to a concrete one.
//!
//! Two backends ship today:
//! - [`NoopBackend`] — returns no OCR results. The default; used
//!   in tests and in deployments that don't need OCR.
//! - [`BentoBackend`] (feature `bento`) — scaffolding for the
//!   externalised `inference-ocr` Bento in
//!   [`nvisycom/inference`]. Not yet functional; tracked under
//!   [#128].
//!
//! [`Backend`]: crate::core::Backend
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference
//! [#128]: https://github.com/nvisycom/runtime/issues/128

#[cfg(feature = "bento")]
mod bento_backend;
#[cfg(feature = "bento")]
mod bento_types;
mod noop_backend;

use nvisy_core::Error;
use serde::{Deserialize, Serialize};

#[cfg(feature = "bento")]
#[cfg_attr(docsrs, doc(cfg(feature = "bento")))]
pub use self::bento_backend::{BentoBackend, BentoParams};
pub use self::noop_backend::NoopBackend;
use crate::engine::Extractor;

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

    /// Externalised [`BentoBackend`] — calls the
    /// `inference-ocr` Bento over HTTP. Requires the runtime to
    /// be built with the `bento` feature.
    ///
    /// **Scaffolding only** until the wire contract is finalised
    /// upstream — see [#128].
    ///
    /// [`BentoBackend`]: BentoBackend
    /// [#128]: https://github.com/nvisycom/runtime/issues/128
    Bento {
        /// Base URL of the `inference-ocr` Bento (for example
        /// `http://localhost:3001` or `http://inference-ocr:3001`
        /// inside a docker-compose network).
        base_url: String,
    },
}

impl OcrBackend {
    /// Build an [`Extractor`] from this config selection.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected backend cannot be
    /// constructed, or if the config selects a backend whose
    /// feature wasn't compiled in.
    pub fn into_extractor(self) -> Result<Extractor, Error> {
        match self {
            Self::Noop => Ok(Extractor::new(NoopBackend)),

            #[cfg(feature = "bento")]
            Self::Bento { base_url } => {
                let backend = BentoBackend::new(BentoParams::new(base_url))?;
                Ok(Extractor::new(backend))
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
    use crate::core::{Backend, Context, ImageFormat, ImageInput};

    #[tokio::test]
    async fn noop_returns_empty() {
        let backend = NoopBackend::new();
        let image = ImageInput::new(vec![0u8; 8], ImageFormat::Png);
        let out = backend.run(&image, Context::default()).await.unwrap();
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn ocr_backend_into_extractor_noop() {
        assert!(OcrBackend::Noop.into_extractor().is_ok());
    }

    #[cfg(not(feature = "bento"))]
    #[test]
    fn ocr_backend_bento_without_feature_errors_clearly() {
        let err = OcrBackend::Bento {
            base_url: "http://localhost:3001".into(),
        }
        .into_extractor()
        .unwrap_err();
        assert!(
            err.to_string().contains("`bento` feature"),
            "error should mention the bento feature: {err}",
        );
    }
}
