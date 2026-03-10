//! Loader and handler traits.
//!
//! A [`Loader`] validates and parses raw content, producing the
//! corresponding [`Handler`]. The handler holds the loaded data and
//! provides methods to read and manipulate it.
//!
//! Each handler implements the base [`Handler`] trait (identity + encode)
//! and one or more capability traits: [`TextHandler`], [`ImageHandler`],
//! [`AudioHandler`].

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

mod audio;
mod image;
mod rich;
mod text;

pub use audio::*;
pub use image::*;
use nvisy_core::path::ContentSource;
pub use rich::*;
pub use text::*;

/// Base trait implemented by all format handlers.
///
/// A handler holds loaded, validated content and provides methods to
/// identify and serialize it. Handlers are produced by their
/// corresponding [`Loader`].
///
/// Capability-specific span access is provided by the opt-in traits
/// [`TextHandler`], [`ImageHandler`], and [`AudioHandler`].
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
