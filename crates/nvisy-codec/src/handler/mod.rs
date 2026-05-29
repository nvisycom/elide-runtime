//! Per-modality wire types (`*Data`, `*Redaction`, `*Output`) and
//! the base [`Handler`] / [`Loader`] traits format handlers
//! implement.
//!
//! Per-modality trait surfaces ([`Codable`], [`Handle<M>`]) live in
//! [`crate::core`]; each module here adds its concrete [`Codable`]
//! impl plus the data/redaction shapes the [`Handle<M>`] methods
//! exchange.
//!
//! Modality features control which wire types compile. The default
//! set (`text`, `tabular`) covers the lightweight cases; opt into
//! `image`, `audio`, or `rich` for the heavier modalities that pull
//! additional dependencies.
//!
//! [`Codable`]: crate::core::Codable
//! [`Handle<M>`]: crate::core::Handle

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "image")]
mod image;
#[cfg(feature = "rich")]
mod rich;
#[cfg(feature = "tabular")]
mod tabular;
#[cfg(feature = "text")]
mod text;

#[cfg(feature = "audio")]
pub use self::audio::*;
#[cfg(feature = "image")]
pub use self::image::*;
#[cfg(feature = "rich")]
pub use self::rich::*;
#[cfg(feature = "tabular")]
pub use self::tabular::*;
#[cfg(feature = "text")]
pub use self::text::*;

/// Base trait implemented by all format handlers.
///
/// A handler holds loaded, validated content and provides methods to
/// identify and serialize it. Handlers are produced by their
/// corresponding [`Loader`].
///
/// Per-modality capability is provided by implementing
/// [`Handle<M>`] for each modality the handler exposes.
///
/// [`Handle<M>`]: crate::core::Handle
pub trait Handler: Send + Sync + 'static {
    /// The document type this handler represents.
    fn document_type(&self) -> DocumentType;

    /// Content source identity and lineage for this handler.
    fn source(&self) -> ContentSource;

    /// Serialize the current handler content back to [`ContentData`].
    fn encode(&self) -> Result<ContentData, Error>;
}

/// Trait implemented by format loaders.
///
/// A loader validates and parses raw content, producing the
/// corresponding handler.
#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
    /// The handler type this loader produces.
    type Handler: Handler;
    /// Strongly-typed parameters for loading.
    type Params: Send;

    /// Validate and parse the content, returning the loaded handler.
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Self::Handler, Error>;
}
