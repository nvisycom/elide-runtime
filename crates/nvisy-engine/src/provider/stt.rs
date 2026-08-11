//! Deployment-owned STT enricher configuration.

/// Deployment configuration for the speech-to-text enricher.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SttBackend {
    /// BentoML-hosted STT service.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// No-op backend. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}
