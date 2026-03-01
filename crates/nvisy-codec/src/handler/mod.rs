//! Loader and handler traits.
//!
//! A [`Loader`] validates and parses raw content, producing a
//! [`Document`] containing the corresponding [`Handler`]. The handler
//! holds the loaded data and provides methods to read and manipulate it.
//!
//! Each handler implements the base [`Handler`] trait (identity + encode)
//! and one or more capability traits: [`TextHandler`], [`ImageHandler`],
//! [`AudioHandler`].

use std::hash::Hash;

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

/// Capability trait for handlers that expose text content.
///
/// Handlers implementing this trait can yield text spans and accept
/// text edits. Each handler defines its own text span addressing
/// scheme via [`TextId`](Self::TextId).
#[async_trait::async_trait]
pub trait TextHandler: Handler {
    /// Strongly-typed identifier for a text span within this handler.
    type TextId: Send + Sync + Clone + Eq + Hash + 'static;

    /// Return text content as an async stream of spans.
    async fn text_spans(&self) -> SpanStream<'_, Self::TextId, text::TextData>;

    /// Apply text edits from an async stream back to the source structure.
    async fn edit_text(
        &mut self,
        edits: SpanEditStream<'_, Self::TextId, text::TextData>,
    ) -> Result<(), Error>;
}

/// Capability trait for handlers that expose image content.
///
/// Handlers implementing this trait can yield image spans and accept
/// image edits.
#[async_trait::async_trait]
pub trait ImageHandler: Handler {
    /// Strongly-typed identifier for an image span within this handler.
    type ImageId: Send + Sync + Clone + 'static;

    /// Return image content as an async stream of spans.
    async fn image_spans(&self) -> SpanStream<'_, Self::ImageId, image::ImageData>;

    /// Apply image edits from an async stream back to the source structure.
    async fn edit_images(
        &mut self,
        edits: SpanEditStream<'_, Self::ImageId, image::ImageData>,
    ) -> Result<(), Error>;
}

/// Capability trait for handlers that expose audio content.
///
/// Handlers implementing this trait can yield audio spans and accept
/// audio edits.
#[async_trait::async_trait]
pub trait AudioHandler: Handler {
    /// Strongly-typed identifier for an audio span within this handler.
    type AudioId: Send + Sync + Clone + 'static;

    /// Return audio content as an async stream of spans.
    async fn audio_spans(&self) -> SpanStream<'_, Self::AudioId, audio::AudioData>;

    /// Apply audio edits from an async stream back to the source structure.
    async fn edit_audio(
        &mut self,
        edits: SpanEditStream<'_, Self::AudioId, audio::AudioData>,
    ) -> Result<(), Error>;
}
