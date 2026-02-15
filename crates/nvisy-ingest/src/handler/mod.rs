//! Loader and handler traits.
//!
//! A [`Loader`] validates and parses raw content, producing a
//! [`Document`] containing the corresponding [`Handler`]. The handler
//! holds the loaded data and provides methods to read and manipulate it.
//!
//! Each handler defines its own span types and exposes them as async
//! streams via [`Handler::view_spans`] and [`Handler::edit_spans`].

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;
use nvisy_ontology::entity::DocumentType;

use crate::document::edit_stream::SpanEditStream;
use crate::document::view_stream::SpanStream;
use crate::document::Document;

pub mod encoding;
pub mod span;

pub mod text;
pub mod document;
pub mod image;
pub mod tabular;
pub mod audio;

pub use encoding::TextEncoding;
pub use span::{Span, SpanEdit};

pub use text::txt_handler::{TxtData, TxtHandler, TxtSpan};
pub use text::txt_loader::{TxtLoader, TxtParams};
pub use text::csv_handler::{CsvData, CsvHandler, CsvSpan};
pub use text::csv_loader::{CsvLoader, CsvParams};
pub use text::json_handler::{
    JsonData, JsonHandler, JsonIndent, JsonPath,
};
pub use text::json_loader::{JsonParams, JsonLoader};

#[cfg(feature = "png")]
pub use image::png::PngHandler;

#[cfg(feature = "wav")]
pub use audio::wav::WavHandler;
#[cfg(feature = "mp3")]
pub use audio::mp3::Mp3Handler;

/// Trait implemented by all format handlers.
///
/// A handler holds loaded, validated content and provides methods to
/// read and manipulate it. Handlers are produced by their corresponding
/// [`Loader`].
///
/// Each handler defines its own span addressing scheme ([`SpanId`](Self::SpanId))
/// and data type ([`SpanData`](Self::SpanData)). Pipeline actions
/// constrain `SpanData` to express what they need (e.g. `AsRef<str>`
/// for text scanning).
#[async_trait::async_trait]
pub trait Handler: Send + Sync + 'static {
    /// The document type this handler represents.
    fn document_type(&self) -> DocumentType;

    /// Strongly-typed identifier for a span within this handler.
    type SpanId: Send + Sync + Clone + 'static;
    /// The data type carried by each span.
    type SpanData: Send + 'static;

    /// Return the loaded content as an async stream of spans.
    async fn view_spans(&self) -> SpanStream<'_, Self::SpanId, Self::SpanData>;

    /// Apply edits from an async stream back to the source structure.
    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, Self::SpanId, Self::SpanData>,
    ) -> Result<(), Error>;
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
    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<Self::Handler>>, Error>;
}
