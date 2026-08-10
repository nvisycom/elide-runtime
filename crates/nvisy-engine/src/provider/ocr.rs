//! Deployment-owned OCR enricher configuration.

/// Deployment configuration for the OCR enricher.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum OcrBackend {
    /// BentoML-hosted OCR service.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// No-op backend; recognises no blocks. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}
