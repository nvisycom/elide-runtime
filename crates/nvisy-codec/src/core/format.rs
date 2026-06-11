//! Format identity: what kind of thing a registered codec is.
//!
//! - [`FormatId`] — stable string identifier (e.g.
//!   `"nvisy.text.txt"`). Open namespace, no central enum.
//! - [`Format`] — descriptor [`CodecRegistry`] indexes by id /
//!   extension / content type. Bundles a `FormatId`, its
//!   [`ModalityKind`], lookup keys, and an erased loader that
//!   decodes bytes into a typed handle.
//!
//! [`CodecRegistry`]: super::CodecRegistry
//! [`ModalityKind`]: nvisy_core::modality::ModalityKind

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use nvisy_core::modality::{Modality, ModalityKind};

use super::{ErasedLoader, Loader, erase};
use crate::document::{DocumentHandle, UntypedDocumentHandle};

/// Stable identifier for a registered codec format. Open string
/// namespace — downstream crates ship their own formats by
/// registering a [`Format`] with a unique [`FormatId`].
///
/// Convention: dot-separated namespace. Built-in formats use the
/// `nvisy.` prefix (e.g. `"nvisy.text.txt"`, `"nvisy.rich.pdf"`).
/// Third-party formats use their own (e.g. `"acme.parquet.v2"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FormatId(Cow<'static, str>);

impl FormatId {
    /// Construct from a static string literal — no allocation.
    pub const fn from_static(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// Construct from an owned [`String`].
    pub fn from_owned(id: String) -> Self {
        Self(Cow::Owned(id))
    }

    /// Borrow as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FormatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for FormatId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Descriptor for one registered codec format. Indexed by
/// [`CodecRegistry`] under its [`FormatId`], every extension in
/// `extensions`, and every MIME in `content_types`.
///
/// Construct via [`Format::new`]; read the parts via the accessor
/// methods. The fields are crate-private so the constructor stays
/// the only path that produces a `Format` — that way the
/// [`ModalityKind`] tag is always derived from the loader's
/// modality and never hand-set.
///
/// [`CodecRegistry`]: super::CodecRegistry
#[derive(Clone)]
pub struct Format {
    pub(crate) id: FormatId,
    pub(crate) modality: ModalityKind,
    pub(crate) extensions: Vec<Cow<'static, str>>,
    pub(crate) content_types: Vec<Cow<'static, str>>,
    pub(crate) loader: Arc<dyn ErasedLoader>,
}

impl Format {
    /// Build a [`Format`] for modality `M`. The runtime
    /// [`ModalityKind`] tag is taken from `M::KIND` and the loader
    /// is erased internally — neither needs to be named at the call
    /// site.
    ///
    /// Extensions and content types default to empty; chain
    /// [`with_extensions`] / [`with_content_types`] to declare the
    /// lookup keys the [`CodecRegistry`] indexes this format under.
    ///
    /// [`with_extensions`]: Self::with_extensions
    /// [`with_content_types`]: Self::with_content_types
    /// [`CodecRegistry`]: super::CodecRegistry
    pub fn new<M, L>(id: FormatId, loader: L) -> Self
    where
        M: Modality,
        L: Loader<M>,
        DocumentHandle<M>: Into<UntypedDocumentHandle>,
    {
        Self {
            id,
            modality: M::KIND,
            extensions: Vec::new(),
            content_types: Vec::new(),
            loader: erase(loader),
        }
    }

    /// Declare the file extensions (lowercased, no leading dot) that
    /// resolve to this format. Extends any previously-declared list.
    #[must_use]
    pub fn with_extensions<I, S>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Cow<'static, str>>,
    {
        self.extensions
            .extend(extensions.into_iter().map(Into::into));
        self
    }

    /// Declare the MIME content types (lowercased) that resolve to
    /// this format. Extends any previously-declared list.
    #[must_use]
    pub fn with_content_types<I, S>(mut self, content_types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Cow<'static, str>>,
    {
        self.content_types
            .extend(content_types.into_iter().map(Into::into));
        self
    }

    /// Stable identifier of this format.
    pub fn id(&self) -> &FormatId {
        &self.id
    }

    /// Modality this format produces.
    pub fn modality(&self) -> ModalityKind {
        self.modality
    }

    /// File extensions (lowercased, no leading dot) that resolve to
    /// this format.
    pub fn extensions(&self) -> &[Cow<'static, str>] {
        &self.extensions
    }

    /// MIME content types (lowercased) that resolve to this format.
    pub fn content_types(&self) -> &[Cow<'static, str>] {
        &self.content_types
    }

    /// Decode raw content through this format's loader, returning
    /// the runtime-tagged handle. Equivalent to calling
    /// [`CodecRegistry::decode`] after resolving the
    /// format yourself.
    ///
    /// [`CodecRegistry::decode`]: super::CodecRegistry::decode
    pub async fn decode(
        &self,
        content: crate::content::ContentData,
    ) -> Result<crate::document::UntypedDocumentHandle, nvisy_core::Error> {
        self.loader.decode(content).await
    }
}

impl fmt::Debug for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Format")
            .field("id", &self.id)
            .field("modality", &self.modality)
            .field("extensions", &self.extensions)
            .field("content_types", &self.content_types)
            .finish_non_exhaustive()
    }
}
