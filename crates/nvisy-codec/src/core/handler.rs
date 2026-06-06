//! [`Handler`] + [`Loader`]: base traits every format handler /
//! loader implements.
//!
//! - [`Handler`] holds loaded, validated content and serializes it
//!   back. Per-modality capability comes from a separate
//!   [`Handle<M>`] impl on the same struct (one modality per
//!   handler; rich formats expose embedded children via
//!   [`EmbeddedHandles`] instead of stacking impls).
//! - [`Loader<M>`] decodes raw [`ContentData`] into a handler. The
//!   [`CodecRegistry`] stores erased loaders via
//!   [`crate::core::LoaderAdapter`].
//!
//! [`Handle<M>`]: super::Handle
//! [`EmbeddedHandles`]: super::EmbeddedHandles
//! [`CodecRegistry`]: super::CodecRegistry

use nvisy_core::Error;

use super::{Codable, EmbeddedHandles, FormatId, IndexedHandle};
use crate::content::{ContentData, ContentSource};

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
/// [`Handle<M>`]: super::Handle
/// [`EmbeddedHandles`]: super::EmbeddedHandles
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
/// [`Handle<M>`]: super::Handle
/// [`IndexedHandle<M>`]: super::IndexedHandle
/// [`CodecRegistry`]: super::CodecRegistry
#[async_trait::async_trait]
pub trait Loader<M: Codable>: Send + Sync + 'static {
    /// The handler type this loader produces.
    type Handler: IndexedHandle<M>;

    /// Validate and parse the content, returning the loaded handler.
    async fn decode(&self, content: ContentData) -> Result<Self::Handler, Error>;
}
