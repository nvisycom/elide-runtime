//! Decoding raw bytes into a typed handle.
//!
//! - [`Loader<M>`] — per-modality decoder format implementations
//!   write. Returns a concrete handler that implements
//!   [`Handler<M>`].
//! - [`ErasedLoader`] — object-safe loader surface the
//!   [`CodecRegistry`] stores behind `Arc`. Adapts a per-modality
//!   `Loader<M>` into a uniform `decode` signature that returns
//!   [`UntypedDocumentHandle`].
//! - [`erase`] — bridge from typed `Loader<M>` to
//!   `Arc<dyn ErasedLoader>` every shipped format uses when
//!   populating [`Format::loader`].
//!
//! The handler's own [`Handler::format`] supplies the
//! [`FormatId`] inside [`ErasedLoader::decode`]; [`erase`] only
//! erases `M`.
//!
//! [`Handler<M>`]: super::Handler
//! [`Handler::format`]: super::Handler::format
//! [`CodecRegistry`]: super::CodecRegistry
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
//! [`Format::loader`]: super::Format::loader
//! [`FormatId`]: super::FormatId

use std::sync::Arc;

use nvisy_core::Error;
use nvisy_core::modality::Modality;

use super::Handler;
use crate::content::ContentData;
use crate::document::{DocumentHandle, UntypedDocumentHandle};

/// Per-modality format loader.
///
/// A loader validates and parses raw content for modality `M`,
/// producing a handler that implements [`Handler<M>`]. Loaders are
/// the leaves the [`CodecRegistry`] composes — registering a
/// format means registering its loader.
///
/// # Implementing a third-party format
///
/// 1. Implement [`Handler<M>`] for the per-format handler type that
///    owns the parsed in-memory representation.
/// 2. Implement `Loader<M>` for a stateless type whose [`decode`]
///    validates raw [`ContentData`] and returns the handler.
/// 3. Build a [`Format`] with [`Format::new`], chain
///    [`with_extensions`] / [`with_content_types`] as needed, and
///    register it on a [`CodecRegistry`] via
///    [`CodecRegistry::add_format`].
///
/// The registry erases `M` internally; third-party callers never
/// touch the object-safe loader surface.
///
/// [`Handler<M>`]: super::Handler
/// [`CodecRegistry`]: super::CodecRegistry
/// [`CodecRegistry::add_format`]: super::CodecRegistry::add_format
/// [`Format`]: super::Format
/// [`Format::new`]: super::Format::new
/// [`with_extensions`]: super::Format::with_extensions
/// [`with_content_types`]: super::Format::with_content_types
/// [`decode`]: Loader::decode
#[async_trait::async_trait]
pub trait Loader<M: Modality>: Send + Sync + 'static {
    /// The handler type this loader produces.
    type Handler: Handler<M>;

    /// Validate and parse the content, returning the loaded handler.
    async fn decode(&self, content: ContentData) -> Result<Self::Handler, Error>;
}

/// Object-safe loader the [`CodecRegistry`] holds behind `Arc`.
/// Adapts a per-modality [`Loader<M>`] into a uniform `decode`
/// signature returning an [`UntypedDocumentHandle`].
///
/// Crate-internal: every consumer goes through [`Format::decode`]
/// or [`CodecRegistry::decode_from_memory`] instead of touching
/// this trait directly.
///
/// [`CodecRegistry`]: super::CodecRegistry
/// [`Format::decode`]: super::Format::decode
/// [`CodecRegistry::decode_from_memory`]: super::CodecRegistry::decode_from_memory
#[async_trait::async_trait]
pub(crate) trait ErasedLoader: Send + Sync + 'static {
    /// Decode raw content into an [`UntypedDocumentHandle`].
    async fn decode(&self, content: ContentData) -> Result<UntypedDocumentHandle, Error>;
}

/// Erase a typed [`Loader<M>`] into an `Arc<dyn ErasedLoader>` the
/// [`CodecRegistry`] can store. Called only by [`Format::new`] —
/// not part of the public API.
///
/// [`CodecRegistry`]: super::CodecRegistry
/// [`Format::new`]: super::Format::new
pub(crate) fn erase<M, L>(loader: L) -> Arc<dyn ErasedLoader>
where
    M: Modality,
    L: Loader<M>,
    DocumentHandle<M>: Into<UntypedDocumentHandle>,
{
    Arc::new(LoaderAdapter {
        loader,
        _phantom: std::marker::PhantomData,
    })
}

/// Private wrapper that holds a typed [`Loader<M>`] and implements
/// the object-safe [`ErasedLoader`] surface. Constructed only via
/// [`erase`]; not part of the public API.
struct LoaderAdapter<M: Modality, L: Loader<M>> {
    loader: L,
    _phantom: std::marker::PhantomData<fn() -> M>,
}

#[async_trait::async_trait]
impl<M, L> ErasedLoader for LoaderAdapter<M, L>
where
    M: Modality,
    L: Loader<M>,
    DocumentHandle<M>: Into<UntypedDocumentHandle>,
{
    async fn decode(&self, content: ContentData) -> Result<UntypedDocumentHandle, Error> {
        let handler = self.loader.decode(content).await?;
        let format = handler.format();
        let handle: Box<dyn Handler<M>> = Box::new(handler);
        Ok(DocumentHandle::new(format, handle).into())
    }
}
