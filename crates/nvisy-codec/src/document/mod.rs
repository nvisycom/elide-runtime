//! [`UntypedDocumentHandle`] (runtime-tagged handle returned by the
//! codec registry) and [`DocumentHandle<M>`] (typed view used
//! downstream of decode).
//!
//! # Two-tier handle shape
//!
//! The registry can't know up front which modality a decoded format
//! produces — that's a property of the format descriptor, resolved
//! at decode time. [`UntypedDocumentHandle`] is the registry-level
//! return: an enum with one variant per modality, each carrying an
//! `Arc<dyn Handle<M>>` + the [`FormatId`].
//!
//! Consumers downstream of decode commit to a modality via the
//! consuming accessors ([`into_text`][it], [`into_image`][ii], …),
//! which yield a [`DocumentHandle<M>`] — a typed wrapper that owns
//! the underlying [`Handle<M>`] and exposes the per-modality
//! capability surface without further dispatch.
//!
//! [it]: UntypedDocumentHandle::into_text
//! [ii]: UntypedDocumentHandle::into_image

use std::sync::Arc;

#[cfg(feature = "audio")]
use nvisy_core::modality::Audio;
#[cfg(feature = "image")]
use nvisy_core::modality::Image;
use nvisy_core::modality::ModalityKind;
#[cfg(feature = "tabular")]
use nvisy_core::modality::Tabular;
#[cfg(feature = "text")]
use nvisy_core::modality::Text;

use crate::core::{Codable, FormatId, IndexedHandle};

/// Runtime-tagged handle returned by the codec registry, carrying the
/// underlying [`Handle<M>`] for some `M` along with the
/// [`FormatId`] of the producing loader.
///
/// Commit to a modality via [`into_text`][it] / [`into_image`][ii] /
/// [`into_audio`][ia] / [`into_tabular`][itb] to obtain a typed
/// [`DocumentHandle<M>`]. The accessors are consuming — once you
/// commit to a modality, the untyped form is gone.
///
/// [it]: Self::into_text
/// [ii]: Self::into_image
/// [ia]: Self::into_audio
/// [itb]: Self::into_tabular
#[derive(Debug)]
pub enum UntypedDocumentHandle {
    #[cfg(feature = "text")]
    /// Text-modality handle.
    Text(DocumentHandle<Text>),
    #[cfg(feature = "tabular")]
    /// Tabular-modality handle.
    Tabular(DocumentHandle<Tabular>),
    #[cfg(feature = "image")]
    /// Image-modality handle.
    Image(DocumentHandle<Image>),
    #[cfg(feature = "audio")]
    /// Audio-modality handle.
    Audio(DocumentHandle<Audio>),
}

impl UntypedDocumentHandle {
    /// The [`FormatId`] of the loader that produced this handle.
    pub fn format(&self) -> &FormatId {
        match self {
            #[cfg(feature = "text")]
            Self::Text(h) => h.format(),
            #[cfg(feature = "tabular")]
            Self::Tabular(h) => h.format(),
            #[cfg(feature = "image")]
            Self::Image(h) => h.format(),
            #[cfg(feature = "audio")]
            Self::Audio(h) => h.format(),
        }
    }

    /// Runtime modality tag — cheaper than matching variants directly
    /// when the caller only needs the modality, not the handle.
    pub fn modality(&self) -> ModalityKind {
        match self {
            #[cfg(feature = "text")]
            Self::Text(_) => ModalityKind::Text,
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => ModalityKind::Tabular,
            #[cfg(feature = "image")]
            Self::Image(_) => ModalityKind::Image,
            #[cfg(feature = "audio")]
            Self::Audio(_) => ModalityKind::Audio,
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Text>`] if
    /// this handle carries text modality.
    #[cfg(feature = "text")]
    pub fn into_text(self) -> Option<DocumentHandle<Text>> {
        match self {
            Self::Text(h) => Some(h),
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "image")]
            Self::Image(_) => None,
            #[cfg(feature = "audio")]
            Self::Audio(_) => None,
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Tabular>`]
    /// if this handle carries tabular modality.
    #[cfg(feature = "tabular")]
    pub fn into_tabular(self) -> Option<DocumentHandle<Tabular>> {
        match self {
            Self::Tabular(h) => Some(h),
            #[cfg(feature = "text")]
            Self::Text(_) => None,
            #[cfg(feature = "image")]
            Self::Image(_) => None,
            #[cfg(feature = "audio")]
            Self::Audio(_) => None,
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Image>`] if
    /// this handle carries image modality.
    #[cfg(feature = "image")]
    pub fn into_image(self) -> Option<DocumentHandle<Image>> {
        match self {
            Self::Image(h) => Some(h),
            #[cfg(feature = "text")]
            Self::Text(_) => None,
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "audio")]
            Self::Audio(_) => None,
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Audio>`] if
    /// this handle carries audio modality.
    #[cfg(feature = "audio")]
    pub fn into_audio(self) -> Option<DocumentHandle<Audio>> {
        match self {
            Self::Audio(h) => Some(h),
            #[cfg(feature = "text")]
            Self::Text(_) => None,
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "image")]
            Self::Image(_) => None,
        }
    }
}

/// Typed view of a single-modality handle. Carries the [`FormatId`]
/// alongside the handler so audit and provenance can always answer
/// "what format is this?" without re-decoding.
///
/// Constructed via the consuming accessors on
/// [`UntypedDocumentHandle`]. The inner handler is held in an `Arc`
/// so the handle can be cloned cheaply and shared across phases.
pub struct DocumentHandle<M: Codable> {
    format: FormatId,
    handler: Arc<dyn IndexedHandle<M>>,
}

impl<M: Codable> DocumentHandle<M> {
    /// Wrap a handler and a format id into a typed handle. Used by
    /// codec loaders to produce the typed handle, which is then
    /// boxed into an [`UntypedDocumentHandle`] variant for registry
    /// return.
    pub fn new(format: FormatId, handler: Arc<dyn IndexedHandle<M>>) -> Self {
        Self { format, handler }
    }

    /// The [`FormatId`] of the producing loader.
    pub fn format(&self) -> &FormatId {
        &self.format
    }

    /// Borrow the inner handler. Use this to invoke per-modality
    /// capability methods ([`IndexedHandle::read`][ir],
    /// [`Handle::next_chunk`][nc], etc.).
    ///
    /// [ir]: crate::core::IndexedHandle::read
    /// [nc]: crate::core::Handle::next_chunk
    pub fn handler(&self) -> &Arc<dyn IndexedHandle<M>> {
        &self.handler
    }

    /// Consume self, returning the inner handler.
    pub fn into_handler(self) -> Arc<dyn IndexedHandle<M>> {
        self.handler
    }
}

impl<M: Codable> Clone for DocumentHandle<M> {
    fn clone(&self) -> Self {
        Self {
            format: self.format.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl<M: Codable> std::fmt::Debug for DocumentHandle<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentHandle")
            .field("format", &self.format)
            .field("modality", &std::any::type_name::<M>())
            .finish()
    }
}
