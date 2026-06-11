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
//! `Box<dyn Handler<M>>` + the [`FormatId`].
//!
//! Consumers downstream of decode commit to a modality via the
//! consuming accessors ([`into_text`], [`into_image`], …),
//! which yield a [`DocumentHandle<M>`] — a typed wrapper that owns
//! the underlying [`Handler<M>`] and exposes the per-modality
//! capability surface without further dispatch.
//!
//! [`Handler<M>`]: crate::core::Handler
//! [`into_text`]: UntypedDocumentHandle::into_text
//! [`into_image`]: UntypedDocumentHandle::into_image

#[cfg(feature = "internal_audio")]
mod audio;
#[cfg(feature = "internal_image")]
mod image;
#[cfg(feature = "internal_tabular")]
mod tabular;
#[cfg(feature = "internal_text")]
mod text;

use derive_more::From;
#[cfg(feature = "internal_audio")]
use nvisy_core::modality::Audio;
#[cfg(feature = "internal_image")]
use nvisy_core::modality::Image;
use nvisy_core::modality::Modality;
#[cfg(feature = "internal_tabular")]
use nvisy_core::modality::Tabular;
#[cfg(feature = "internal_text")]
use nvisy_core::modality::Text;

use crate::core::{FormatId, Handler};

/// Runtime-tagged handle returned by the codec registry, carrying the
/// underlying [`Handler<M>`] for some `M` along with the
/// [`FormatId`] of the producing loader.
///
/// Commit to a modality via [`into_text`] / [`into_image`] /
/// [`into_audio`] / [`into_tabular`] to obtain a typed
/// [`DocumentHandle<M>`]. The accessors are consuming — once you
/// commit to a modality, the untyped form is gone.
///
/// [`Handler<M>`]: crate::core::Handler
/// [`into_text`]: Self::into_text
/// [`into_image`]: Self::into_image
/// [`into_audio`]: Self::into_audio
/// [`into_tabular`]: Self::into_tabular
#[derive(Debug, From)]
pub enum UntypedDocumentHandle {
    #[cfg(feature = "internal_text")]
    /// Text-modality handle.
    Text(DocumentHandle<Text>),
    #[cfg(feature = "internal_tabular")]
    /// Tabular-modality handle.
    Tabular(DocumentHandle<Tabular>),
    #[cfg(feature = "internal_image")]
    /// Image-modality handle.
    Image(DocumentHandle<Image>),
    #[cfg(feature = "internal_audio")]
    /// Audio-modality handle.
    Audio(DocumentHandle<Audio>),
}

impl UntypedDocumentHandle {
    /// The [`FormatId`] of the loader that produced this handle.
    pub fn format_id(&self) -> &FormatId {
        match self {
            #[cfg(feature = "internal_text")]
            Self::Text(h) => h.format_id(),
            #[cfg(feature = "internal_tabular")]
            Self::Tabular(h) => h.format_id(),
            #[cfg(feature = "internal_image")]
            Self::Image(h) => h.format_id(),
            #[cfg(feature = "internal_audio")]
            Self::Audio(h) => h.format_id(),
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Text>`] if
    /// this handle carries text modality.
    #[cfg(feature = "internal_text")]
    pub fn into_text(self) -> Option<DocumentHandle<Text>> {
        match self {
            Self::Text(h) => Some(h),
            #[cfg(feature = "internal_tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "internal_image")]
            Self::Image(_) => None,
            #[cfg(feature = "internal_audio")]
            Self::Audio(_) => None,
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Tabular>`]
    /// if this handle carries tabular modality.
    #[cfg(feature = "internal_tabular")]
    pub fn into_tabular(self) -> Option<DocumentHandle<Tabular>> {
        match self {
            Self::Tabular(h) => Some(h),
            #[cfg(feature = "internal_text")]
            Self::Text(_) => None,
            #[cfg(feature = "internal_image")]
            Self::Image(_) => None,
            #[cfg(feature = "internal_audio")]
            Self::Audio(_) => None,
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Image>`] if
    /// this handle carries image modality.
    #[cfg(feature = "internal_image")]
    pub fn into_image(self) -> Option<DocumentHandle<Image>> {
        match self {
            Self::Image(h) => Some(h),
            #[cfg(feature = "internal_text")]
            Self::Text(_) => None,
            #[cfg(feature = "internal_tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "internal_audio")]
            Self::Audio(_) => None,
        }
    }

    /// Consume self, returning the inner [`DocumentHandle<Audio>`] if
    /// this handle carries audio modality.
    #[cfg(feature = "internal_audio")]
    pub fn into_audio(self) -> Option<DocumentHandle<Audio>> {
        match self {
            Self::Audio(h) => Some(h),
            #[cfg(feature = "internal_text")]
            Self::Text(_) => None,
            #[cfg(feature = "internal_tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "internal_image")]
            Self::Image(_) => None,
        }
    }
}

/// Typed view of a single-modality handle. Carries the [`FormatId`]
/// alongside the handler so audit and provenance can always answer
/// "what format is this?" without re-decoding.
///
/// Constructed via the consuming accessors on
/// [`UntypedDocumentHandle`]. The inner handler is held in a `Box`
/// because the pipeline runs phases sequentially per document — there
/// is no concurrent access to the handle within a single document's
/// run, so reference counting buys nothing.
///
/// Implements the core `*At` trait surface
/// ([`TextAt`] / [`DataAt`] / [`RedactAt`]) directly so any
/// pipeline component can read from / write to a codec-backed
/// source through the same traits the engine bounds on. The
/// per-modality impls live in the `text` / `tabular` / `image` /
/// `audio` sibling modules.
///
/// Modality coverage:
///
/// | Modality   | TextAt | DataAt | RedactAt |
/// |------------|--------|--------|----------|
/// | Text       |   ✓    |   ✓    |    ✓     |
/// | Tabular    |   ✓    |   ✓    |    ✓     |
/// | Image      |        |   ✓    |    ✓     |
/// | Audio      |        |   ✓    |    ✓     |
///
/// Image and audio don't implement [`TextAt`] — "text at this
/// location" for image means OCR text, for audio means transcript
/// text, and both come from the extraction phase in
/// `nvisy-toolkit::extraction`. The codec layer has no visibility
/// into either.
///
/// [`TextAt`]: nvisy_core::extraction::TextAt
/// [`DataAt`]: nvisy_core::extraction::DataAt
/// [`RedactAt`]: nvisy_core::redaction::RedactAt
pub struct DocumentHandle<M: Modality> {
    format_id: FormatId,
    handler: Box<dyn Handler<M>>,
}

impl<M: Modality> DocumentHandle<M> {
    /// Wrap a handler and a format id into a typed handle. Used by
    /// codec loaders to produce the typed handle, which is then
    /// erased into an [`UntypedDocumentHandle`] variant for registry
    /// return.
    pub fn new(format_id: FormatId, handler: Box<dyn Handler<M>>) -> Self {
        Self { format_id, handler }
    }

    /// The [`FormatId`] of the producing loader.
    pub fn format_id(&self) -> &FormatId {
        &self.format_id
    }

    /// Borrow the inner handler. Use this for read-only capability
    /// methods ([`Handler::read`]).
    ///
    /// [`Handler::read`]: crate::core::Handler::read
    pub fn handler(&self) -> &dyn Handler<M> {
        &*self.handler
    }

    /// Mutably borrow the inner handler. Use this for cursor-advancing
    /// methods ([`Handler::next_chunk`]) and the redaction batch
    /// applicator ([`Handler::redact`]).
    ///
    /// [`Handler::next_chunk`]: crate::core::Handler::next_chunk
    /// [`Handler::redact`]: crate::core::Handler::redact
    pub fn handler_mut(&mut self) -> &mut dyn Handler<M> {
        &mut *self.handler
    }

    /// Consume self, returning the inner handler.
    pub fn into_handler(self) -> Box<dyn Handler<M>> {
        self.handler
    }
}

impl<M: Modality> std::fmt::Debug for DocumentHandle<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentHandle")
            .field("format_id", &self.format_id)
            .field("modality", &std::any::type_name::<M>())
            .finish()
    }
}
