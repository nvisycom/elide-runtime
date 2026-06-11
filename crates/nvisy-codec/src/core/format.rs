//! Format identity: what kind of thing a registered codec is.
//!
//! - [`FormatId`] — stable string identifier (e.g.
//!   `"nvisy.text.txt"`). Open namespace, no central enum.
//! - [`ModalityKind`] — runtime tag for the four codec modalities
//!   (Text, Tabular, Image, Audio). Used wherever the typed `M`
//!   marker has been erased.
//! - [`Format`] — descriptor [`CodecRegistry`] indexes by id /
//!   extension / content type. Bundles a `FormatId`, its
//!   `ModalityKind`, lookup keys, and the [`ErasedLoader`] that
//!   decodes bytes into a typed handle.
//!
//! [`CodecRegistry`]: super::CodecRegistry
//! [`ErasedLoader`]: super::ErasedLoader

use std::any::TypeId;
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use nvisy_core::modality::{Audio, Image, Modality, Tabular, Text};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Codable, ErasedLoader, Loader, erase};
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

/// Runtime tag identifying which [`Modality`] a generic container
/// carries. Used for runtime dispatch where the marker type is
/// erased — at the codec / pipeline boundary where a `Document<M>`
/// or `Handle<M>` has been pushed through an
/// [`UntypedDocumentHandle`] or `AnyLocation` arm and the typed
/// `M` is no longer in scope.
///
/// [`Modality`]: nvisy_core::modality::Modality
/// [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModalityKind {
    /// [`Text`] modality.
    ///
    /// [`Text`]: nvisy_core::modality::Text
    Text,
    /// [`Tabular`] modality.
    ///
    /// [`Tabular`]: nvisy_core::modality::Tabular
    Tabular,
    /// [`Image`] modality.
    ///
    /// [`Image`]: nvisy_core::modality::Image
    Image,
    /// [`Audio`] modality.
    ///
    /// [`Audio`]: nvisy_core::modality::Audio
    Audio,
}

impl ModalityKind {
    /// Return the [`ModalityKind`] for a typed `M: Modality` at the
    /// call site.
    ///
    /// Resolved at runtime via [`TypeId`] rather than a
    /// `Modality`-level associated const so adding a new modality
    /// type doesn't force every implementor to advertise a tag.
    /// The match is exhaustive over the four built-in marker types;
    /// an unknown `M` panics.
    #[must_use]
    pub fn of<M: Modality>() -> Self {
        let id = TypeId::of::<M>();
        if id == TypeId::of::<Text>() {
            Self::Text
        } else if id == TypeId::of::<Tabular>() {
            Self::Tabular
        } else if id == TypeId::of::<Image>() {
            Self::Image
        } else if id == TypeId::of::<Audio>() {
            Self::Audio
        } else {
            unreachable!("Modality must be one of Text/Tabular/Image/Audio");
        }
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
    pub fn new<M, L>(
        id: FormatId,
        extensions: Vec<Cow<'static, str>>,
        content_types: Vec<Cow<'static, str>>,
        loader: L,
    ) -> Self
    where
        M: Codable,
        L: Loader<M>,
        DocumentHandle<M>: Into<UntypedDocumentHandle>,
    {
        Self {
            id,
            modality: M::KIND,
            extensions,
            content_types,
            loader: erase(loader),
        }
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

    /// Loader that decodes raw content into the typed handle.
    pub fn loader(&self) -> &Arc<dyn ErasedLoader> {
        &self.loader
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
