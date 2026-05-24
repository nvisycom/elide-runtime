//! Type-erased content handle for all supported modalities.

mod located;
mod span;
mod stream;

use std::fmt;

use derive_more::{From, IsVariant, TryInto};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
#[cfg(feature = "audio")]
use nvisy_ontology::entity::AudioLocation;
#[cfg(feature = "image")]
use nvisy_ontology::entity::ImageLocation;
#[cfg(feature = "tabular")]
use nvisy_ontology::entity::TabularLocation;
#[cfg(feature = "text")]
use nvisy_ontology::entity::TextLocation;

pub use self::located::Located;
pub use self::span::Span;
pub use self::stream::LocationStream;
#[cfg(feature = "audio")]
use crate::handler::{AudioData, AudioHandler, AudioRedaction, BoxedAudioHandler};
#[cfg(feature = "image")]
use crate::handler::{BoxedImageHandler, ImageData, ImageHandler, ImageRedaction};
#[cfg(feature = "rich")]
use crate::handler::BoxedRichHandler;
#[cfg(feature = "tabular")]
use crate::handler::{BoxedTabularHandler, TabularHandler, TabularRedaction};
#[cfg(feature = "text")]
use crate::handler::{BoxedTextHandler, TextData, TextHandler, TextRedaction};
use crate::handler::{Handler, Redactions};

/// A fully type-erased document that can hold any supported modality.
///
/// Variants are feature-gated by modality (`text`, `tabular`,
/// `image`, `audio`, `rich`); a `ContentHandle` built with only the
/// default features has the `Text` and `Tabular` arms only.
#[derive(From, IsVariant, TryInto)]
pub enum ContentHandle {
    #[cfg(feature = "text")]
    Text(BoxedTextHandler),
    #[cfg(feature = "tabular")]
    Tabular(BoxedTabularHandler),
    #[cfg(feature = "image")]
    Image(BoxedImageHandler),
    #[cfg(feature = "audio")]
    Audio(BoxedAudioHandler),
    #[cfg(feature = "rich")]
    Rich(BoxedRichHandler),
}

impl fmt::Debug for ContentHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentHandle")
            .field(&self.document_type())
            .finish()
    }
}

impl ContentHandle {
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
            Self::Rich(h) => h.document_type(),
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
            Self::Rich(h) => h.source(),
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
            Self::Rich(h) => h.encode(),
        }
    }

    /// Stream text locations from text or rich documents.
    #[cfg(feature = "text")]
    pub fn text_locations(&self) -> LocationStream<'_, TextLocation> {
        match self {
            Self::Text(h) => h.locations(),
            #[cfg(feature = "rich")]
            Self::Rich(h) => TextHandler::locations(h),
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
    pub fn tabular_locations(&self) -> LocationStream<'_, TabularLocation> {
        match self {
            Self::Tabular(h) => h.locations(),
            _ => LocationStream::empty(),
        }
    }

    /// Stream image locations from image or rich documents.
    #[cfg(feature = "image")]
    pub fn image_locations(&self) -> LocationStream<'_, ImageLocation> {
        match self {
            Self::Image(h) => h.locations(),
            #[cfg(feature = "rich")]
            Self::Rich(h) => ImageHandler::locations(h),
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
    pub fn audio_locations(&self) -> LocationStream<'_, AudioLocation> {
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
    pub async fn read_text(&self, location: &TextLocation) -> Option<TextData> {
        match self {
            Self::Text(h) => h.read(location).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => TextHandler::read(h, location).await,
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
    pub async fn read_tabular(&self, location: &TabularLocation) -> Option<TextData> {
        match self {
            Self::Tabular(h) => h.read(location).await,
            _ => None,
        }
    }

    /// Read image data at the given location.
    #[cfg(feature = "image")]
    pub async fn read_image(&self, location: &ImageLocation) -> Option<ImageData> {
        match self {
            Self::Image(h) => h.read(location).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => ImageHandler::read(h, location).await,
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
    pub async fn read_audio(&self, location: &AudioLocation) -> Option<AudioData> {
        match self {
            Self::Audio(h) => h.read(location).await,
            _ => None,
        }
    }

    /// Apply a batch of text redactions to the document.
    #[cfg(feature = "text")]
    pub async fn apply_text_redactions(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Text(h) => TextHandler::redact(h, redactions).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => TextHandler::redact(h, redactions).await,
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
        redactions: Redactions<TabularLocation, TabularRedaction>,
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
        redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Image(h) => ImageHandler::redact(h, redactions).await,
            #[cfg(feature = "rich")]
            Self::Rich(h) => ImageHandler::redact(h, redactions).await,
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
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Audio(h) => h.redact(redactions).await,
            _ => Ok(()),
        }
    }
}
