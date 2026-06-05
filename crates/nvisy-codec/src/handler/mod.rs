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

use crate::core::{Codable, EmbeddedHandles, FormatId, IndexedHandle};

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "image")]
mod image;
#[cfg(feature = "tabular")]
mod tabular;
#[cfg(feature = "text")]
mod text;

#[cfg(feature = "audio")]
pub use self::audio::*;
#[cfg(feature = "image")]
pub use self::image::*;
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
/// [`Handle<M>`] for the single modality the handler exposes.
/// Multi-modality is **not** done via multiple `Handle<M>` impls on
/// the same struct — rich formats implement [`EmbeddedHandles`] and
/// expose child handles instead.
///
/// [`Handle<M>`]: crate::core::Handle
/// [`EmbeddedHandles`]: crate::core::EmbeddedHandles
pub trait Handler: Send + Sync + 'static {
    /// Stable id of the format this handler represents (e.g.
    /// `"nvisy.text.txt"`). Cheap to clone: built-in formats use a
    /// statically-borrowed `Cow`.
    fn format(&self) -> FormatId;

    /// Content source identity and lineage for this handler.
    fn source(&self) -> &ContentSource;

    /// Serialize the current handler content back to [`ContentData`].
    fn encode(&self) -> Result<ContentData, Error>;

    /// Embedded-child accessor for rich formats (PDF, DOCX) whose
    /// chunks reference inner [`UntypedDocumentHandle`] handles.
    /// Returns `None` for leaf formats — the default — so only rich
    /// handlers need to override.
    ///
    /// The engine importer walks this to build the embed tree under
    /// the root document, without an `Any` downcast at the call site.
    ///
    /// [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
    fn embedded(&self) -> Option<&dyn EmbeddedHandles> {
        None
    }
}

/// Per-modality format loader.
///
/// A loader validates and parses raw content for modality `M`,
/// producing a handler that implements [`IndexedHandle<M>`] (which
/// in turn implies [`Handle<M>`]). Loaders are the leaves the
/// [`CodecRegistry`] composes — registering a format means
/// registering its loader.
///
/// [`Handle<M>`]: crate::core::Handle
/// [`IndexedHandle<M>`]: crate::core::IndexedHandle
/// [`CodecRegistry`]: crate::CodecRegistry
#[async_trait::async_trait]
pub trait Loader<M: Codable>: Send + Sync + 'static {
    /// The handler type this loader produces.
    type Handler: IndexedHandle<M>;

    /// Validate and parse the content, returning the loaded handler.
    async fn decode(&self, content: ContentData) -> Result<Self::Handler, Error>;
}
