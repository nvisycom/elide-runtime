//! Type-erased content handle for all supported modalities.

use std::fmt;

use derive_more::From;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, DocumentType};
#[cfg(feature = "audio")]
use nvisy_core::modality::Audio;
#[cfg(feature = "image")]
use nvisy_core::modality::Image;
#[cfg(feature = "tabular")]
use nvisy_core::modality::Tabular;
#[cfg(feature = "text")]
use nvisy_core::modality::Text;

use crate::core::{Handle, LocationStream, Redactions};
use crate::handler::Handler;
#[cfg(feature = "rich")]
use crate::handler::RichHandle;
#[cfg(feature = "audio")]
use crate::handler::{AudioData, AudioRedaction};
#[cfg(feature = "image")]
use crate::handler::{ImageData, ImageRedaction};
#[cfg(feature = "tabular")]
use crate::handler::{TabularHandle, TabularRedaction};
#[cfg(feature = "text")]
use crate::handler::{TextData, TextRedaction};

/// Cheap, non-generic tag for which modality a [`DocumentHandle`]
/// carries. Lets callers dispatch on the modality without holding
/// the handle's lock or matching against the trait-object variants
/// directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleModality {
    #[cfg(feature = "text")]
    Text,
    #[cfg(feature = "tabular")]
    Tabular,
    #[cfg(feature = "image")]
    Image,
    #[cfg(feature = "audio")]
    Audio,
    #[cfg(feature = "rich")]
    Rich,
}

/// A fully type-erased document that can hold any supported modality.
///
/// Variants are feature-gated by modality (`text`, `tabular`,
/// `image`, `audio`, `rich`); a `DocumentHandle` built with only the
/// default features has the `Text` and `Tabular` arms only.
#[derive(From)]
pub enum DocumentHandle {
    #[cfg(feature = "text")]
    Text(Box<dyn Handle<Text>>),
    #[cfg(feature = "tabular")]
    Tabular(Box<dyn TabularHandle>),
    #[cfg(feature = "image")]
    Image(Box<dyn Handle<Image>>),
    #[cfg(feature = "audio")]
    Audio(Box<dyn Handle<Audio>>),
    #[cfg(feature = "rich")]
    Rich(Box<dyn RichHandle>),
}

impl fmt::Debug for DocumentHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DocumentHandle")
            .field(&self.document_type())
            .finish()
    }
}

impl DocumentHandle {
    /// The modality tag for this handle. Cheaper than matching on
    /// the variants directly and avoids holding the trait object
    /// across borrows.
    pub fn modality(&self) -> HandleModality {
        match self {
            #[cfg(feature = "text")]
            Self::Text(_) => HandleModality::Text,
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => HandleModality::Tabular,
            #[cfg(feature = "image")]
            Self::Image(_) => HandleModality::Image,
            #[cfg(feature = "audio")]
            Self::Audio(_) => HandleModality::Audio,
            #[cfg(feature = "rich")]
            Self::Rich(_) => HandleModality::Rich,
        }
    }

    /// The document type of the underlying content.
    pub fn document_type(&self) -> DocumentType {
        match self {
            #[cfg(feature = "text")]
            Self::Text(h) => h.document_type(),
            #[cfg(feature = "tabular")]
            Self::Tabular(h) => h.document_type(),
            #[cfg(feature = "image")]
            Self::Image(h) => h.document_type(),
            #[cfg(feature = "audio")]
            Self::Audio(h) => h.document_type(),
            #[cfg(feature = "rich")]
            Self::Rich(h) => Handler::document_type(h.as_ref()),
        }
    }

    /// Content source identity and lineage.
    pub fn source(&self) -> ContentSource {
        match self {
            #[cfg(feature = "text")]
            Self::Text(h) => h.source(),
            #[cfg(feature = "tabular")]
            Self::Tabular(h) => h.source(),
            #[cfg(feature = "image")]
            Self::Image(h) => h.source(),
            #[cfg(feature = "audio")]
            Self::Audio(h) => h.source(),
            #[cfg(feature = "rich")]
            Self::Rich(h) => Handler::source(h.as_ref()),
        }
    }

    /// Encode the document back to raw bytes.
    pub fn encode(&self) -> Result<ContentData, Error> {
        match self {
            #[cfg(feature = "text")]
            Self::Text(h) => h.encode(),
            #[cfg(feature = "tabular")]
            Self::Tabular(h) => h.encode(),
            #[cfg(feature = "image")]
            Self::Image(h) => h.encode(),
            #[cfg(feature = "audio")]
            Self::Audio(h) => h.encode(),
            #[cfg(feature = "rich")]
            Self::Rich(h) => Handler::encode(h.as_ref()),
        }
    }

    /// Stream text locations from text or rich documents.
    #[cfg(feature = "text")]
    pub fn text_locations(&self) -> LocationStream<'_, Text> {
        match self {
            Self::Text(h) => h.locations(),
            #[cfg(feature = "rich")]
            Self::Rich(h) => <dyn Handle<Text>>::locations(h.as_ref()),
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => LocationStream::empty(),
            #[cfg(feature = "image")]
            Self::Image(_) => LocationStream::empty(),
            #[cfg(feature = "audio")]
            Self::Audio(_) => LocationStream::empty(),
        }
    }

    /// Stream tabular (cell) locations from spreadsheet documents.
    #[cfg(feature = "tabular")]
    pub fn tabular_locations(&self) -> LocationStream<'_, Tabular> {
        match self {
            Self::Tabular(h) => h.locations(),
            _ => LocationStream::empty(),
        }
    }

    /// `true` when the underlying tabular source carries explicit
    /// column headers / typed schema; `false` when column semantics
    /// have to be inferred. `None` for non-tabular handles.
    #[cfg(feature = "tabular")]
    pub fn tabular_has_header(&self) -> Option<bool> {
        match self {
            Self::Tabular(h) => Some(h.has_header()),
            _ => None,
        }
    }

    /// Stream image locations from image or rich documents.
    #[cfg(feature = "image")]
    pub fn image_locations(&self) -> LocationStream<'_, Image> {
        match self {
            Self::Image(h) => h.locations(),
            #[cfg(feature = "rich")]
            Self::Rich(h) => <dyn Handle<Image>>::locations(h.as_ref()),
            #[cfg(feature = "text")]
            Self::Text(_) => LocationStream::empty(),
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => LocationStream::empty(),
            #[cfg(feature = "audio")]
            Self::Audio(_) => LocationStream::empty(),
        }
    }

    /// Stream audio locations from audio documents.
    #[cfg(feature = "audio")]
    pub fn audio_locations(&self) -> LocationStream<'_, Audio> {
        match self {
            Self::Audio(h) => h.locations(),
            _ => LocationStream::empty(),
        }
    }

    /// Read text data at the given location.
    ///
    /// Returns `None` if the location is out of bounds or the handle
    /// does not expose text content.
    #[cfg(feature = "text")]
    pub async fn read_text(&self, location: &Text) -> Option<TextData> {
        match self {
            Self::Text(h) => h.read(location).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => <dyn Handle<Text>>::read(h.as_ref(), location).await,
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "image")]
            Self::Image(_) => None,
            #[cfg(feature = "audio")]
            Self::Audio(_) => None,
        }
    }

    /// Read the cell value at the given tabular location.
    #[cfg(feature = "tabular")]
    pub async fn read_tabular(&self, location: &Tabular) -> Option<TextData> {
        match self {
            Self::Tabular(h) => h.read(location).await,
            _ => None,
        }
    }

    /// Read image data at the given location.
    #[cfg(feature = "image")]
    pub async fn read_image(&self, location: &Image) -> Option<ImageData> {
        match self {
            Self::Image(h) => h.read(location).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => <dyn Handle<Image>>::read(h.as_ref(), location).await,
            #[cfg(feature = "text")]
            Self::Text(_) => None,
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => None,
            #[cfg(feature = "audio")]
            Self::Audio(_) => None,
        }
    }

    /// Read audio data at the given location.
    #[cfg(feature = "audio")]
    pub async fn read_audio(&self, location: &Audio) -> Option<AudioData> {
        match self {
            Self::Audio(h) => h.read(location).await,
            _ => None,
        }
    }

    /// Apply a batch of text redactions to the document.
    #[cfg(feature = "text")]
    pub async fn apply_text_redactions(
        &mut self,
        redactions: Redactions<Text, TextRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Text(h) => h.redact(redactions).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => <dyn Handle<Text>>::redact(h.as_mut(), redactions).await,
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => Ok(()),
            #[cfg(feature = "image")]
            Self::Image(_) => Ok(()),
            #[cfg(feature = "audio")]
            Self::Audio(_) => Ok(()),
        }
    }

    /// Apply a batch of tabular redactions to the document.
    #[cfg(feature = "tabular")]
    pub async fn apply_tabular_redactions(
        &mut self,
        redactions: Redactions<Tabular, TabularRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Tabular(h) => h.redact(redactions).await,
            _ => Ok(()),
        }
    }

    /// Apply a batch of image redactions to the document.
    #[cfg(feature = "image")]
    pub async fn apply_image_redactions(
        &mut self,
        redactions: Redactions<Image, ImageRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Image(h) => h.redact(redactions).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => <dyn Handle<Image>>::redact(h.as_mut(), redactions).await,
            #[cfg(feature = "text")]
            Self::Text(_) => Ok(()),
            #[cfg(feature = "tabular")]
            Self::Tabular(_) => Ok(()),
            #[cfg(feature = "audio")]
            Self::Audio(_) => Ok(()),
        }
    }

    /// Apply a batch of audio redactions to the document.
    #[cfg(feature = "audio")]
    pub async fn apply_audio_redactions(
        &mut self,
        redactions: Redactions<Audio, AudioRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Audio(h) => h.redact(redactions).await,
            _ => Ok(()),
        }
    }
}
