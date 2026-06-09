//! Backend layer: the [`OcrBackend`] trait and its shipped impls.
//!
//! Built-in [`NoopBackend`] (returns no blocks; test stub) and
//! feature-gated [`BentoBackend`] (HTTP call into the externalised
//! `inference-ocr` service, scaffolded against the future wire
//! contract).
//!
//! Selecting which backend to use at deployment time is the
//! consumer's concern — `nvisy-ocr` itself doesn't enumerate "all
//! backends" in a closed enum.

mod noop_backend;
mod ocr_backend;

#[cfg(feature = "bento")]
mod bento_backend;
#[cfg(feature = "bento")]
mod bento_types;

#[cfg(feature = "bento")]
#[cfg_attr(docsrs, doc(cfg(feature = "bento")))]
pub use self::bento_backend::{BentoBackend, BentoParams};
pub use self::noop_backend::NoopBackend;
pub use self::ocr_backend::{OcrBackend, OcrRequest, OcrResponse};

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[tokio::test]
    async fn noop_returns_empty() {
        let backend = NoopBackend::new();
        let image = Bytes::from(vec![0u8; 8]);
        let response = backend.extract(OcrRequest::new(&image)).await.unwrap();
        assert_eq!(response.blocks.len(), 0);
    }
}
