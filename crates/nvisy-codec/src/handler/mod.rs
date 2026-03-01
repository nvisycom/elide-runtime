//! Loader and handler traits.
//!
//! A [`Loader`] validates and parses raw content, producing a
//! [`Document`] containing the corresponding [`Handler`]. The handler
//! holds the loaded data and provides methods to read and manipulate it.
//!
//! Each handler implements the base [`Handler`] trait (identity + encode)
//! and one or more capability traits: [`TextHandler`], [`ImageHandler`],
//! [`AudioHandler`].

use bytes::Bytes;

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_core::fs::DocumentType;

use crate::document::Document;

mod view_span;
mod edit_span;
mod view_stream;
mod edit_stream;
mod text;
mod rich;
mod image;
mod audio;

pub use view_span::Span;
pub use edit_span::SpanEdit;
pub use view_stream::SpanStream;
pub use edit_stream::SpanEditStream;

pub use text::*;
pub use rich::*;
pub use image::*;
pub use audio::*;

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

    /// Serialize the current handler content back to raw bytes.
    fn encode(&self) -> Result<Bytes, Error>;
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
