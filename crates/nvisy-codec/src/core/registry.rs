//! [`CodecRegistry`]: resolves an extension or content type to a
//! registered [`Format`] and decodes content through its loader.
//!
//! Each [`Format`] bundles a [`FormatId`], its [`ModalityKind`], the
//! extensions and content types that resolve to it, and an
//! [`ErasedLoader`] that decodes bytes into an
//! [`UntypedDocumentHandle`].
//!
//! Downstream crates register their own formats by calling
//! [`CodecRegistry::register`] — there is no central enum to extend.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use nvisy_core::Error;
use nvisy_core::content::ContentData;
use nvisy_core::modality::ModalityKind;

use crate::core::{Codable, FormatId, Handler, IndexedHandle, Loader};
use crate::document::{DocumentHandle, UntypedDocumentHandle};

/// Descriptor for one registered codec format.
#[derive(Clone)]
pub struct Format {
    /// Stable identifier (e.g. `"nvisy.text.txt"`).
    pub id: FormatId,
    /// Modality this format produces.
    pub modality: ModalityKind,
    /// File extensions (lowercased, no leading dot) that resolve to
    /// this format.
    pub extensions: Vec<Cow<'static, str>>,
    /// MIME content types (lowercased) that resolve to this format.
    pub content_types: Vec<Cow<'static, str>>,
    /// Loader that decodes raw content into the typed handle.
    pub loader: Arc<dyn ErasedLoader>,
}

impl std::fmt::Debug for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Format")
            .field("id", &self.id)
            .field("modality", &self.modality)
            .field("extensions", &self.extensions)
            .field("content_types", &self.content_types)
            .finish_non_exhaustive()
    }
}

/// Object-safe loader the registry holds behind `Arc`. Adapts a
/// per-modality [`Loader<M>`] into a uniform `decode` signature that
/// returns an [`UntypedDocumentHandle`].
#[async_trait]
pub trait ErasedLoader: Send + Sync + 'static {
    /// Modality this loader produces.
    fn modality(&self) -> ModalityKind;

    /// Decode raw content into an [`UntypedDocumentHandle`].
    async fn decode(&self, content: ContentData) -> Result<UntypedDocumentHandle, Error>;
}

/// Adapter that wraps a per-modality [`Loader<M>`] into an
/// [`ErasedLoader`] the registry can store. The produced handler's
/// own [`Handler::format`] supplies the [`FormatId`] for the typed
/// [`DocumentHandle<M>`].
pub struct LoaderAdapter<M: Codable, L: Loader<M>> {
    loader: L,
    _phantom: std::marker::PhantomData<fn() -> M>,
}

impl<M: Codable, L: Loader<M>> LoaderAdapter<M, L> {
    /// Wrap a typed loader so the registry can dispatch into it
    /// uniformly.
    pub fn new(loader: L) -> Self {
        Self {
            loader,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<M, L> ErasedLoader for LoaderAdapter<M, L>
where
    M: Codable + WrapUntyped,
    L: Loader<M>,
{
    fn modality(&self) -> ModalityKind {
        M::KIND
    }

    async fn decode(&self, content: ContentData) -> Result<UntypedDocumentHandle, Error> {
        let handler = self.loader.decode(content).await?;
        let format = handler.format();
        let handle: Box<dyn IndexedHandle<M>> = Box::new(handler);
        let typed = DocumentHandle::new(format, handle);
        Ok(M::wrap(typed))
    }
}

/// Wrap a typed [`DocumentHandle<M>`] into the matching
/// [`UntypedDocumentHandle`] variant.
///
/// Implemented per modality so the [`ErasedLoader`] adapter can erase
/// `M` at the registry boundary while still constructing the right
/// variant on the way out.
pub trait WrapUntyped: Codable {
    /// Erase the typed handle into the runtime-tagged variant.
    fn wrap(handle: DocumentHandle<Self>) -> UntypedDocumentHandle;
}

#[cfg(feature = "internal_text")]
impl WrapUntyped for nvisy_core::modality::Text {
    fn wrap(handle: DocumentHandle<Self>) -> UntypedDocumentHandle {
        UntypedDocumentHandle::Text(handle)
    }
}

#[cfg(feature = "internal_tabular")]
impl WrapUntyped for nvisy_core::modality::Tabular {
    fn wrap(handle: DocumentHandle<Self>) -> UntypedDocumentHandle {
        UntypedDocumentHandle::Tabular(handle)
    }
}

#[cfg(feature = "internal_image")]
impl WrapUntyped for nvisy_core::modality::Image {
    fn wrap(handle: DocumentHandle<Self>) -> UntypedDocumentHandle {
        UntypedDocumentHandle::Image(handle)
    }
}

#[cfg(feature = "internal_audio")]
impl WrapUntyped for nvisy_core::modality::Audio {
    fn wrap(handle: DocumentHandle<Self>) -> UntypedDocumentHandle {
        UntypedDocumentHandle::Audio(handle)
    }
}

/// Codec registry — owns the set of registered [`Format`]s and
/// resolves them by extension, content type, or id.
#[derive(Debug, Default)]
pub struct CodecRegistry {
    formats: Vec<Format>,
    by_id: HashMap<FormatId, usize>,
    by_extension: HashMap<String, usize>,
    by_content_type: HashMap<String, usize>,
}

impl CodecRegistry {
    /// Empty registry. Use [`register`] to add custom formats, or
    /// [`with_builtin`] to start from a pre-populated set of every
    /// built-in format the active feature set enables.
    ///
    /// [`register`]: Self::register
    /// [`with_builtin`]: Self::with_builtin
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populated registry containing every built-in format the
    /// active feature set enables (TXT, JSON, HTML, CSV, PNG, JPEG,
    /// WAV, PDF, …). Equivalent to [`new`] followed by registering
    /// each built-in format.
    ///
    /// Add custom formats afterward with [`register`]; they take
    /// precedence on extension / content-type collisions
    /// (last registration wins).
    ///
    /// [`new`]: Self::new
    /// [`register`]: Self::register
    pub fn with_builtin() -> Self {
        let registry = Self::new();
        #[allow(unused_mut)]
        let mut registry = registry;
        #[cfg(feature = "txt")]
        {
            registry = registry.register(crate::handler::text::txt_format());
        }
        #[cfg(feature = "json")]
        {
            registry = registry.register(crate::handler::text::json_format());
        }
        #[cfg(feature = "markdown")]
        {
            registry = registry.register(crate::handler::text::markdown_format());
        }
        #[cfg(feature = "html")]
        {
            registry = registry.register(crate::handler::text::html_format());
        }
        #[cfg(feature = "csv")]
        {
            registry = registry.register(crate::handler::tabular::csv_format());
        }
        #[cfg(feature = "xlsx")]
        {
            registry = registry.register(crate::handler::tabular::xlsx_format());
        }
        #[cfg(feature = "png")]
        {
            registry = registry.register(crate::handler::image::png_format());
        }
        #[cfg(feature = "jpeg")]
        {
            registry = registry.register(crate::handler::image::jpeg_format());
        }
        #[cfg(feature = "tiff")]
        {
            registry = registry.register(crate::handler::image::tiff_format());
        }
        #[cfg(feature = "wav")]
        {
            registry = registry.register(crate::handler::audio::wav_format());
        }
        #[cfg(feature = "mp3")]
        {
            registry = registry.register(crate::handler::audio::mp3_format());
        }
        #[cfg(feature = "pdf")]
        {
            registry = registry.register(crate::handler::rich::pdf_format());
        }
        #[cfg(feature = "docx")]
        {
            registry = registry.register(crate::handler::rich::docx_format());
        }
        registry
    }

    /// Register a [`Format`]. The format's id, extensions, and
    /// content types are indexed for lookup. Returns `self` so calls
    /// chain.
    ///
    /// # Panics
    ///
    /// Panics if the format's id is already registered. Extensions
    /// and content types that conflict with an existing format are
    /// overwritten (last registration wins) — register custom
    /// formats *after* [`with_builtin`] if you want them to take
    /// precedence.
    ///
    /// [`with_builtin`]: Self::with_builtin
    #[must_use]
    pub fn register(mut self, format: Format) -> Self {
        assert!(
            !self.by_id.contains_key(&format.id),
            "format id already registered: {}",
            format.id
        );
        let index = self.formats.len();
        for ext in &format.extensions {
            self.by_extension.insert(ext.to_ascii_lowercase(), index);
        }
        for ct in &format.content_types {
            self.by_content_type.insert(ct.to_ascii_lowercase(), index);
        }
        self.by_id.insert(format.id.clone(), index);
        self.formats.push(format);
        self
    }

    /// Look up a registered format by id.
    pub fn by_id(&self, id: &FormatId) -> Option<&Format> {
        self.by_id.get(id).map(|&i| &self.formats[i])
    }

    /// Look up a registered format by file extension (case-insensitive,
    /// no leading dot).
    pub fn by_extension(&self, ext: &str) -> Option<&Format> {
        self.by_extension
            .get(&ext.to_ascii_lowercase())
            .map(|&i| &self.formats[i])
    }

    /// Look up a registered format by MIME content type (case-insensitive).
    pub fn by_content_type(&self, mime: &str) -> Option<&Format> {
        self.by_content_type
            .get(&mime.to_ascii_lowercase())
            .map(|&i| &self.formats[i])
    }

    /// Iterate over every registered format in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &Format> {
        self.formats.iter()
    }

    /// Decode in-memory bytes using the format resolved from the
    /// extension hint.
    pub async fn decode_from_memory(
        &self,
        bytes: impl Into<ContentData>,
        extension: &str,
    ) -> Result<UntypedDocumentHandle, Error> {
        let format = self.by_extension(extension).ok_or_else(|| {
            Error::validation(
                format!("no codec registered for extension `{extension}`"),
                "nvisy_codec::registry::decode_from_memory",
            )
        })?;
        format.loader.decode(bytes.into()).await
    }
}
