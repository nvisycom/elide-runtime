//! Built-in [`Backend`] implementations.
//!
//! Two backends ship today:
//!
//! - [`NoopBackend`] — returns no OCR results. Used in tests and in
//!   deployments that don't need OCR.
//! - [`BentoBackend`] (feature `bento`) — scaffolding for the
//!   externalised `inference-ocr` Bento in
//!   [`nvisycom/inference`]. Not yet functional; tracked under
//!   [#128].
//!
//! Selecting which backend to use at deployment time is the consumer's
//! concern — `nvisy-ocr` itself doesn't enumerate "all backends" in a
//! closed enum. The pipeline layer (`nvisy-document`) hosts the
//! TOML-deserialisable selector and hands the chosen [`Backend`] to
//! [`crate::Extractor::new`].
//!
//! [`Backend`]: crate::core::Backend
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference
//! [#128]: https://github.com/nvisycom/runtime/issues/128

#[cfg(feature = "bento")]
mod bento_backend;
#[cfg(feature = "bento")]
mod bento_types;
mod noop_backend;

#[cfg(feature = "bento")]
#[cfg_attr(docsrs, doc(cfg(feature = "bento")))]
pub use self::bento_backend::{BentoBackend, BentoParams};
pub use self::noop_backend::NoopBackend;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Backend, Context, ImageInput};

    #[tokio::test]
    async fn noop_returns_empty() {
        let backend = NoopBackend::new();
        let image = ImageInput::new(vec![0u8; 8]);
        let out = backend.run(&image, Context::default()).await.unwrap();
        assert_eq!(out.len(), 0);
    }
}
