//! Response compression middleware.
//!
//! Applies gzip, brotli, and zstd compression based on the client's
//! `Accept-Encoding` header.

use axum::Router;
use tower_http::compression::CompressionLayer;

/// Extension trait for [`Router`] to add response compression.
pub trait RouterCompressionExt<S> {
    /// Layers gzip, brotli, and zstd response compression.
    fn with_compression(self) -> Self;
}

impl<S> RouterCompressionExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_compression(self) -> Self {
        self.layer(CompressionLayer::new())
    }
}
