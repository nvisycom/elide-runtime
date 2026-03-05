//! Loader and handler traits.
//!
//! A [`Loader`] validates and parses raw content, producing a
//! [`Document`] containing the corresponding [`Handler`]. The handler
//! holds the loaded data and provides methods to read and manipulate it.
//!
//! Each handler implements the base [`Handler`] trait (identity + encode)
//! and one or more capability traits: [`TextHandler`], [`ImageHandler`],
//! [`AudioHandler`].

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use crate::document::Document;

mod audio;
mod edit_span;
mod edit_stream;
mod image;
mod rich;
mod text;
mod view_span;
mod view_stream;

pub use audio::*;
pub use edit_span::SpanEdit;
pub use edit_stream::SpanEditStream;
pub use image::*;
pub use rich::*;
pub use text::*;
pub use view_span::Span;
pub use view_stream::SpanStream;

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

    /// Serialize the current handler content back to [`ContentData`].
    fn encode(&self) -> Result<ContentData, Error>;
}

/// Trait implemented by format loaders.
///
/// A loader validates and parses raw content, producing a
/// [`Document`] with the corresponding handler.
#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
    /// The handler type this loader produces.
    type Handler: Handler;
    /// Strongly-typed parameters for loading.
    type Params: Send;

    /// Validate and parse the content, returning a document with
    /// the loaded handler.
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Document<Self::Handler>, Error>;
}
