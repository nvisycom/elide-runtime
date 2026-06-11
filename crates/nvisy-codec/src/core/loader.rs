//! Decoding raw bytes into a typed handle.
//!
//! - [`Loader<M>`] — per-modality decoder format implementations
//!   write. Returns a concrete handler that implements
//!   [`Handle<M>`].
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
//! [`Handle<M>`]: super::Handle
//! [`Handler::format`]: super::Handler::format
//! [`CodecRegistry`]: super::CodecRegistry
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
//! [`Format::loader`]: super::Format::loader
//! [`FormatId`]: super::FormatId

use std::sync::Arc;

use nvisy_core::Error;

use super::{Codable, Handle, Handler, ModalityKind};
use crate::content::ContentData;
use crate::document::{DocumentHandle, UntypedDocumentHandle};

/// Per-modality format loader.
///
/// A loader validates and parses raw content for modality `M`,
/// producing a handler that implements [`Handle<M>`]. Loaders are
/// the leaves the [`CodecRegistry`] composes — registering a
/// format means registering its loader.
///
/// [`Handle<M>`]: super::Handle
/// [`CodecRegistry`]: super::CodecRegistry
#[async_trait::async_trait]
pub trait Loader<M: Codable>: Send + Sync + 'static {
    /// The handler type this loader produces.
    type Handler: Handle<M>;

    /// Validate and parse the content, returning the loaded handler.
    async fn decode(&self, content: ContentData) -> Result<Self::Handler, Error>;
}

/// Object-safe loader the [`CodecRegistry`] holds behind `Arc`.
/// Adapts a per-modality [`Loader<M>`] into a uniform `decode`
/// signature returning an [`UntypedDocumentHandle`].
///
/// [`CodecRegistry`]: super::CodecRegistry
#[async_trait::async_trait]
pub trait ErasedLoader: Send + Sync + 'static {
    /// Modality this loader produces.
    fn modality(&self) -> ModalityKind;

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
    M: Codable,
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
struct LoaderAdapter<M: Codable, L: Loader<M>> {
    loader: L,
    _phantom: std::marker::PhantomData<fn() -> M>,
}

#[async_trait::async_trait]
impl<M, L> ErasedLoader for LoaderAdapter<M, L>
where
    M: Codable,
    L: Loader<M>,
    DocumentHandle<M>: Into<UntypedDocumentHandle>,
{
    fn modality(&self) -> ModalityKind {
        M::KIND
    }

    async fn decode(&self, content: ContentData) -> Result<UntypedDocumentHandle, Error> {
        let handler = self.loader.decode(content).await?;
        let format = handler.format();
        let handle: Box<dyn Handle<M>> = Box::new(handler);
        Ok(DocumentHandle::new(format, handle).into())
    }
}
